use std::path::PathBuf;

use clap::{Parser, Subcommand};

use order_check::audio::{Segmenter, load_wav_16k_mono};
use order_check::matcher::Matcher;
use order_check::menu::Menu;
use order_check::pipeline::{check_utterances, transcribe_segments};
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
    }
    Ok(())
}
