use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};

use order_check::audio::{Segmenter, load_wav_16k_mono};
use order_check::matcher::Matcher;
use order_check::menu::Menu;
use order_check::mic::CpalMic;
use order_check::pipeline::{check_utterances, transcribe_segments};
use order_check::records::Records;
use order_check::server::{AppState, router};
use order_check::session::{SessionConfig, Sessions};
use order_check::stt::WhisperTranscriber;
use order_check::ticket::Ticket;

#[derive(Parser)]
#[command(
    name = "order-check",
    about = "Checks a spoken restaurant order against the POS ticket"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check one recorded order (WAV) against a ticket (JSON) and print the report.
    Check {
        #[arg(long, default_value = "menus/chick-fil-a.toml")]
        menu: PathBuf,
        #[arg(long, default_value = "models/ggml-large-v3-turbo-q5_0.bin")]
        model: PathBuf,
        #[arg(long)]
        audio: PathBuf,
        #[arg(long)]
        ticket: PathBuf,
        /// Also report the optional service-standards booleans.
        #[arg(long)]
        standards: bool,
    },
    /// Run the in-restaurant server: live mic, order checks, POS and dashboard pages.
    Serve {
        #[arg(long, default_value = "menus/chick-fil-a.toml")]
        menu: PathBuf,
        #[arg(long, default_value = "models/ggml-large-v3-turbo-q5_0.bin")]
        model: PathBuf,
        #[arg(long, default_value = "data/records.db")]
        db: PathBuf,
        #[arg(long, default_value_t = 7080)]
        port: u16,
        /// Turn on the optional service-standards check.
        #[arg(long)]
        standards: bool,
        /// Stream live transcript text to the POS demo panel (for demos only).
        #[arg(long)]
        demo_transcript: bool,
        /// Seconds after sending before the simulated "food ready" buzz.
        #[arg(long, default_value_t = 20)]
        ready_after: u64,
        /// Fill the dashboard with clearly labeled synthetic records first.
        #[arg(long)]
        seed_demo_data: bool,
        #[arg(long, default_value = "web")]
        web: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Check {
            menu,
            model,
            audio,
            ticket,
            standards,
        } => {
            let menu = Menu::load(&menu)?;
            let ticket: Ticket = serde_json::from_str(&std::fs::read_to_string(&ticket)?)?;
            ticket.validate(&menu)?;
            let stt = WhisperTranscriber::new(&model)?;
            let pcm = load_wav_16k_mono(&audio)?;
            let mut seg = Segmenter::new();
            let mut segments = seg.push(&pcm);
            segments.extend(seg.flush());
            let started = std::time::Instant::now();
            let utts = transcribe_segments(&stt, &menu, &segments)?;
            let report = check_utterances(&menu, &Matcher::new(&menu), utts, &ticket, standards);
            eprintln!(
                "transcribed {} segment(s) in {} ms; check took {} ms",
                segments.len(),
                started.elapsed().as_millis(),
                report.verify_ms
            );
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Cmd::Serve {
            menu,
            model,
            db,
            port,
            standards,
            demo_transcript,
            ready_after,
            seed_demo_data,
            web,
        } => {
            let menu = Arc::new(Menu::load(&menu)?);
            let records = Records::open(&db)?;
            if seed_demo_data {
                order_check::records::seed_demo(&records, &menu, 300)?;
            }
            eprintln!("loading {} ...", model.display());
            let stt = Arc::new(WhisperTranscriber::new(&model)?);
            let (events, _) = tokio::sync::broadcast::channel(256);
            let sessions = Arc::new(Sessions::new(
                menu,
                stt,
                records,
                Box::new(CpalMic::new()),
                SessionConfig {
                    standards_on: standards,
                    demo_transcript,
                },
                events.clone(),
            ));
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async move {
                let ticker = sessions.clone();
                tokio::spawn(async move {
                    let mut every = tokio::time::interval(Duration::from_secs(30));
                    loop {
                        every.tick().await;
                        ticker.expire(Instant::now());
                    }
                });
                let app = router(AppState { sessions, events, ready_after: Duration::from_secs(ready_after), web_dir: web });
                let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
                eprintln!("Order Check running: POS http://localhost:{port}/pos.html  dashboard http://localhost:{port}/dashboard.html");
                axum::serve(listener, app).await?;
                anyhow::Ok(())
            })?;
        }
    }
    Ok(())
}
