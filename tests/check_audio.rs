use std::path::{Path, PathBuf};
use std::process::Command;

use order_check::audio::{Segmenter, load_wav_16k_mono};
use order_check::matcher::Matcher;
use order_check::menu::Menu;
use order_check::pipeline::{check_utterances, transcribe_segments};
use order_check::stt::WhisperTranscriber;
use order_check::ticket::Ticket;
use order_check::verify::Outcome;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Renders `text` with macOS `say` into a 16 kHz mono WAV. None if tools are missing.
fn speak(text: &str, name: &str) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("oc-say-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let aiff = dir.join(format!("{name}.aiff"));
    let wav = dir.join(format!("{name}.wav"));
    let ok = Command::new("say")
        .args(["-v", "Samantha", "-o"])
        .arg(&aiff)
        .arg(text)
        .status()
        .ok()?
        .success()
        && Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(&aiff)
            .args(["-ar", "16000", "-ac", "1"])
            .arg(&wav)
            .status()
            .ok()?
            .success();
    ok.then_some(wav)
}

#[test]
fn check_audio_spicy_meal() {
    let model = root().join("models/ggml-base.en.bin");
    if !model.exists() {
        eprintln!(
            "skipping: {} missing (run tools/download_models.sh)",
            model.display()
        );
        return;
    }
    let Some(wav) = speak(
        "Can I get a spicy chicken sandwich meal with a medium sweet tea, and three Polynesian sauces?",
        "spicy",
    ) else {
        eprintln!("skipping: say or ffmpeg unavailable");
        return;
    };

    let menu = Menu::load(&root().join("menus/chick-fil-a.toml")).unwrap();
    let pcm = load_wav_16k_mono(&wav).unwrap();
    let mut seg = Segmenter::new();
    let mut segments = seg.push(&pcm);
    segments.extend(seg.flush());
    assert!(!segments.is_empty());

    let stt = WhisperTranscriber::new(&model).unwrap();
    let utts = transcribe_segments(&stt, &menu, &segments).unwrap();
    let ticket: Ticket = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/fixtures/spicy_meal.ticket.json")).unwrap(),
    )
    .unwrap();
    let report = check_utterances(&menu, &Matcher::new(&menu), utts, &ticket, false);

    let messages: Vec<&str> = report
        .result
        .mismatches
        .iter()
        .map(|m| m.message.as_str())
        .collect();
    assert_eq!(report.result.outcome, Outcome::Mismatch, "{:#?}", report);
    assert!(
        messages.contains(&"Said SPICY CHICKEN SANDWICH MEAL, rang CHICKEN SANDWICH MEAL"),
        "{messages:?} / {:?}",
        report.transcript
    );
    assert!(
        messages.contains(&"Said 3 Polynesian, rang 1"),
        "{messages:?} / {:?}",
        report.transcript
    );
    let _ = Path::new(&wav);
}
