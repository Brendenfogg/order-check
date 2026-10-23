use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use order_check::menu::Menu;
use order_check::mic::{AudioSource, NoMic};
use order_check::records::Records;
use order_check::session::{SessionConfig, Sessions};
use order_check::stt::FakeTranscriber;
use order_check::ticket::Ticket;
use order_check::verify::Outcome;

/// A mic that opens fine but hears nothing; tests feed audio with push_audio.
struct QuietMic;

impl AudioSource for QuietMic {
    fn start(&mut self, _sink: Sender<Vec<f32>>) -> anyhow::Result<()> {
        Ok(())
    }
    fn stop(&mut self) {}
}

fn menu() -> Arc<Menu> {
    Arc::new(
        Menu::load(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("menus/chick-fil-a.toml"),
        )
        .unwrap(),
    )
}

fn sessions(texts: &[&str], mic: Box<dyn AudioSource>) -> Sessions {
    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    Sessions::new(
        menu(),
        Arc::new(FakeTranscriber::new(texts)),
        Records::open_in_memory().unwrap(),
        mic,
        SessionConfig {
            standards_on: false,
            demo_transcript: false,
        },
        tx,
    )
}

/// One utterance worth of audio: half a second of tone, then a pause.
fn utterance() -> Vec<f32> {
    let mut v: Vec<f32> = (0..8000).map(|i| 0.3 * (i as f32 * 0.17).sin()).collect();
    v.extend(vec![0.0; 16000]);
    v
}

fn ticket(json: serde_json::Value) -> Ticket {
    serde_json::from_value(json).unwrap()
}

fn wrong() -> Ticket {
    ticket(
        serde_json::json!({"lines": [{"item": "chicken_sandwich", "qty": 1}, {"item": "polynesian", "qty": 1}]}),
    )
}

fn right() -> Ticket {
    ticket(
        serde_json::json!({"lines": [{"item": "spicy_sandwich", "qty": 1}, {"item": "polynesian", "qty": 3}]}),
    )
}

#[test]
fn finalize_then_send_deletes_transcript() {
    let s = sessions(
        &["a spicy chicken sandwich", "and three polynesian sauces"],
        Box::new(QuietMic),
    );
    let n = s.start("drive_thru", "Navy SUV").unwrap();
    assert_eq!(n, 1001);
    s.push_audio(n, &utterance());
    s.push_audio(n, &utterance());

    let first = s.finalize(n, &wrong()).unwrap();
    assert_eq!(first.result.outcome, Outcome::Mismatch, "{first:#?}");
    assert!(s.has_transcript(n));

    let second = s.finalize(n, &right()).unwrap();
    assert_eq!(second.result.outcome, Outcome::Match, "{second:#?}");

    let rec = s.send(n, false).unwrap();
    assert!(!s.has_transcript(n));
    assert!(rec.fixed);
    assert_eq!(rec.caught, 2);
    assert_eq!(rec.mode, "drive_thru");
    assert_eq!(s.records().dashboard(&menu(), 0).unwrap().orders, 1);
}

#[test]
fn sent_as_rung_is_not_fixed() {
    let s = sessions(&["a spicy chicken sandwich"], Box::new(QuietMic));
    let n = s.start("face2face", "Olivia").unwrap();
    s.push_audio(n, &utterance());
    s.finalize(
        n,
        &ticket(serde_json::json!({"lines": [{"item": "chicken_sandwich", "qty": 1}]})),
    )
    .unwrap();
    let rec = s.send(n, true).unwrap();
    assert!(!rec.fixed && rec.sent_as_rung);
    assert_eq!(rec.caught, 1);
}

#[test]
fn cancel_writes_no_record() {
    let s = sessions(&[], Box::new(QuietMic));
    let n = s.start("face2face", "Olivia").unwrap();
    s.cancel(n);
    assert!(!s.has_transcript(n));
    assert_eq!(s.records().dashboard(&menu(), 0).unwrap().orders, 0);
}

#[test]
fn abandoned_order_expires() {
    let s = sessions(&["a cookie"], Box::new(QuietMic));
    let n = s.start("drive_thru", "Red Truck").unwrap();
    s.push_audio(n, &utterance());
    assert!(s.expire(Instant::now()).is_empty());
    assert_eq!(s.expire(Instant::now() + Duration::from_secs(11 * 60)), [n]);
    assert!(!s.has_transcript(n));
    assert_eq!(s.records().dashboard(&menu(), 0).unwrap().orders, 0);
}

#[test]
fn mic_offline_still_works() {
    let s = sessions(&[], Box::new(NoMic));
    let n = s.start("drive_thru", "Silver Sedan").unwrap();
    let report = s.finalize(n, &right()).unwrap();
    assert_eq!(report.result.outcome, Outcome::Unconfirmed);
    assert_eq!(report.result.reason.as_deref(), Some("clip offline"));
    assert!(report.result.mismatches.is_empty());
    s.send(n, false).unwrap();
}

#[test]
fn unknown_order_errors() {
    let s = sessions(&[], Box::new(QuietMic));
    assert!(s.finalize(4242, &right()).is_err());
    assert!(s.send(4242, false).is_err());
}

#[test]
fn send_before_finalize_errors() {
    let s = sessions(&[], Box::new(QuietMic));
    let n = s.start("face2face", "Ann").unwrap();
    assert!(s.send(n, false).is_err());
}
