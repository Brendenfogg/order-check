//! Evaluation: runs recorded or synthetic orders with known answers through
//! the real pipeline and reports catch rate, false alarms, and latency.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::audio::{Segmenter, load_wav_16k_mono};
use crate::extract::extract;
use crate::matcher::Matcher;
use crate::menu::Menu;
use crate::pipeline::{prompt_for, transcribe_segments, transcript_confidence};
use crate::stt::Transcriber;
use crate::ticket::Ticket;
use crate::verify::{MismatchKind, Outcome, verify};

#[derive(Debug, Clone, Deserialize)]
pub struct CaseVariant {
    pub name: String,
    pub ticket: Ticket,
    #[serde(default)]
    pub planted: Vec<MismatchKind>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Case {
    pub id: String,
    pub situation: String,
    pub noise: String,
    pub truth: Ticket,
    pub variants: Vec<CaseVariant>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct EvalReport {
    pub cases: u32,
    pub checks: u32,
    pub extraction_accuracy: f32,
    pub catch_rate: f32,
    pub false_alarm_rate: f32,
    pub unconfirmed_rate: f32,
    pub p50_finalize_ms: u32,
    pub p95_finalize_ms: u32,
    pub by_kind: BTreeMap<String, (u32, u32)>,
    pub by_situation: BTreeMap<String, (f32, f32)>,
    pub by_noise: BTreeMap<String, (f32, f32)>,
    pub failures: Vec<String>,
}

#[derive(Default)]
struct Tally {
    caught: u32,
    planted: u32,
    false_alarms: u32,
    correct: u32,
}

impl Tally {
    fn rates(&self) -> (f32, f32) {
        (
            ratio(self.caught, self.planted),
            ratio(self.false_alarms, self.correct),
        )
    }
}

fn ratio(a: u32, b: u32) -> f32 {
    if b == 0 { 0.0 } else { a as f32 / b as f32 }
}

fn percentile(sorted: &[u32], p: f32) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let i = ((sorted.len() - 1) as f32 * p).round() as usize;
    sorted[i]
}

/// Runs every `<cases_dir>/<id>/{case.json, audio.wav}` through the pipeline.
/// Finalize time counts only the last utterance plus the check, since earlier
/// utterances are transcribed while the order is still being taken.
/// What Whisper heard for one case, kept so logic changes can be re-scored
/// without re-transcribing. `last_ms` is the time the final utterance took.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heard {
    pub utts: Vec<crate::extract::Utterance>,
    pub last_ms: u32,
}

pub type TranscriptCache = BTreeMap<String, Heard>;

pub fn run_eval(
    menu: &Menu,
    stt: &dyn Transcriber,
    cases_dir: &Path,
) -> anyhow::Result<EvalReport> {
    run_eval_cached(menu, Some(stt), cases_dir, &mut TranscriptCache::new())
}

/// Like `run_eval`, but reuses transcripts already in `cache` and adds new ones.
/// With `stt = None`, every case must already be in the cache.
pub fn run_eval_cached(
    menu: &Menu,
    stt: Option<&dyn Transcriber>,
    cases_dir: &Path,
    cache: &mut TranscriptCache,
) -> anyhow::Result<EvalReport> {
    let matcher = Matcher::new(menu);
    let mut dirs: Vec<_> = std::fs::read_dir(cases_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("case.json").exists())
        .collect();
    dirs.sort();

    let mut report = EvalReport::default();
    let (mut all, mut extraction_ok, mut unconfirmed) = (Tally::default(), 0u32, 0u32);
    let mut by_kind: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut by_situation: BTreeMap<String, Tally> = BTreeMap::new();
    let mut by_noise: BTreeMap<String, Tally> = BTreeMap::new();
    let mut finalize_ms = Vec::new();

    for dir in dirs {
        let case: Case = serde_json::from_str(&std::fs::read_to_string(dir.join("case.json"))?)
            .with_context(|| format!("reading {}", dir.join("case.json").display()))?;
        let heard_case = match cache.get(&case.id) {
            Some(h) => h.clone(),
            None => {
                let stt = stt
                    .ok_or_else(|| anyhow::anyhow!("{} is not in the transcript cache", case.id))?;
                let h = transcribe_case(menu, stt, &dir.join("audio.wav"))?;
                cache.insert(case.id.clone(), h.clone());
                h
            }
        };
        let utts = heard_case.utts;
        let transcribe_last = std::time::Duration::from_millis(heard_case.last_ms as u64);
        report.cases += 1;

        let spoken = extract(menu, &matcher, &utts);
        let confidence = transcript_confidence(&utts);
        let heard: String = utts
            .iter()
            .map(|u| u.text.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        let truth = verify(menu, &spoken, &case.truth, confidence);
        if truth.outcome == Outcome::Match {
            extraction_ok += 1;
        } else {
            let why: Vec<_> = truth
                .mismatches
                .iter()
                .chain(&truth.unconfirmed)
                .map(|m| m.message.as_str())
                .collect();
            report.failures.push(format!(
                "{}: extraction differs from truth ({}) — heard \"{heard}\"",
                case.id,
                why.join("; ")
            ));
        }

        for v in &case.variants {
            let t0 = Instant::now();
            let r = verify(menu, &spoken, &v.ticket, confidence);
            finalize_ms.push((transcribe_last + t0.elapsed()).as_millis() as u32);
            report.checks += 1;
            if r.outcome == Outcome::Unconfirmed {
                unconfirmed += 1;
            }
            let sit = by_situation.entry(case.situation.clone()).or_default();
            let noise = by_noise.entry(case.noise.clone()).or_default();
            if v.planted.is_empty() {
                let alarm = r.outcome == Outcome::Mismatch;
                for t in [&mut all, sit, noise] {
                    t.correct += 1;
                    t.false_alarms += alarm as u32;
                }
                if alarm {
                    let msgs: Vec<_> = r.mismatches.iter().map(|m| m.message.as_str()).collect();
                    report.failures.push(format!(
                        "{}: false alarm on correct ticket ({}) — heard \"{heard}\"",
                        case.id,
                        msgs.join("; ")
                    ));
                }
            } else {
                let hit = r.outcome == Outcome::Mismatch
                    && r.mismatches.iter().any(|m| v.planted.contains(&m.kind));
                for t in [&mut all, sit, noise] {
                    t.planted += 1;
                    t.caught += hit as u32;
                }
                for k in &v.planted {
                    let e = by_kind.entry(kind_name(*k)).or_default();
                    e.1 += 1;
                    e.0 += hit as u32;
                }
                if !hit {
                    report.failures.push(format!(
                        "{}: missed planted {} ({:?}) — heard \"{heard}\"",
                        case.id, v.name, r.outcome
                    ));
                }
            }
        }
    }

    finalize_ms.sort_unstable();
    report.extraction_accuracy = ratio(extraction_ok, report.cases);
    (report.catch_rate, report.false_alarm_rate) = all.rates();
    report.unconfirmed_rate = ratio(unconfirmed, report.checks);
    report.p50_finalize_ms = percentile(&finalize_ms, 0.5);
    report.p95_finalize_ms = percentile(&finalize_ms, 0.95);
    report.by_kind = by_kind;
    report.by_situation = by_situation
        .into_iter()
        .map(|(k, t)| (k, t.rates()))
        .collect();
    report.by_noise = by_noise.into_iter().map(|(k, t)| (k, t.rates())).collect();
    Ok(report)
}

fn transcribe_case(menu: &Menu, stt: &dyn Transcriber, wav: &Path) -> anyhow::Result<Heard> {
    let pcm = load_wav_16k_mono(wav)?;
    let mut seg = Segmenter::new();
    let mut segments = seg.push(&pcm);
    segments.extend(seg.flush());
    let (earlier, last) = segments.split_at(segments.len().saturating_sub(1));
    let mut utts = transcribe_segments(stt, menu, earlier)?;
    let started = Instant::now();
    for s in last {
        let u = stt.transcribe(s, &prompt_for(menu, &utts))?;
        if !u.text.is_empty() {
            utts.push(u);
        }
    }
    Ok(Heard {
        utts,
        last_ms: started.elapsed().as_millis() as u32,
    })
}

fn kind_name(k: MismatchKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn pct(x: f32) -> String {
    format!("{:.1}%", x * 100.0)
}

/// A readable summary for the README and LinkedIn write-up.
pub fn render_markdown(r: &EvalReport, model: &str) -> String {
    let mut md = String::new();
    md.push_str(&format!("# Order Check evaluation ({model})\n\n"));
    md.push_str(&format!(
        "{} orders, {} ticket checks. The check caught **{}** of planted mistakes, raised a false alarm on **{}** of correct tickets, and returned a result in **{} ms** (median; p95 {} ms) after Finalize.\n\n",
        r.cases, r.checks, pct(r.catch_rate), pct(r.false_alarm_rate), r.p50_finalize_ms, r.p95_finalize_ms
    ));
    md.push_str("| Metric | Value | Target |\n|---|---|---|\n");
    md.push_str(&format!(
        "| Planted mistakes caught | {} | ≥ 90% |\n",
        pct(r.catch_rate)
    ));
    md.push_str(&format!(
        "| False alarms on correct tickets | {} | ≤ 5% |\n",
        pct(r.false_alarm_rate)
    ));
    md.push_str(&format!(
        "| Finalize → result, p95 | {} ms | ≤ 1000 ms |\n",
        r.p95_finalize_ms
    ));
    md.push_str(&format!(
        "| Spoken order read exactly right | {} | — |\n",
        pct(r.extraction_accuracy)
    ));
    md.push_str(&format!(
        "| Checks returned \"couldn't confirm\" | {} | — |\n\n",
        pct(r.unconfirmed_rate)
    ));
    md.push_str("## By mistake type\n\n| Type | Caught | Planted | Rate |\n|---|---|---|---|\n");
    for (k, (c, p)) in &r.by_kind {
        md.push_str(&format!("| {k} | {c} | {p} | {} |\n", pct(ratio(*c, *p))));
    }
    for (title, map) in [
        ("By audio situation", &r.by_situation),
        ("By background noise", &r.by_noise),
    ] {
        md.push_str(&format!(
            "\n## {title}\n\n| | Caught | False alarms |\n|---|---|---|\n"
        ));
        for (k, (c, f)) in map {
            md.push_str(&format!("| {k} | {} | {} |\n", pct(*c), pct(*f)));
        }
    }
    if !r.failures.is_empty() {
        md.push_str(&format!(
            "\n## Misses and false alarms ({})\n\n",
            r.failures.len()
        ));
        for f in &r.failures {
            md.push_str(&format!("- {f}\n"));
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::cfa;
    use crate::stt::FakeTranscriber;

    fn write_case(dir: &Path, id: &str, situation: &str, case: serde_json::Value) {
        let d = dir.join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("case.json"), case.to_string()).unwrap();
        // One second of tone so the segmenter hands the transcriber one utterance.
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(d.join("audio.wav"), spec).unwrap();
        for i in 0..16_000 {
            w.write_sample(((i as f32 * 0.17).sin() * 9000.0) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
        let _ = situation;
    }

    #[test]
    fn eval_on_fake_cases() {
        let dir = std::env::temp_dir().join(format!("oc-eval-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let spicy = serde_json::json!({"lines": [{"item": "spicy_sandwich", "qty": 1}, {"item": "polynesian", "qty": 3}]});
        write_case(
            &dir,
            "a-spicy",
            "face_to_face",
            serde_json::json!({
            "id": "a-spicy", "situation": "face_to_face", "noise": "clean", "truth": spicy,
            "variants": [
                {"name": "correct", "ticket": spicy, "planted": []},
                {"name": "variant", "ticket": {"lines": [{"item": "chicken_sandwich", "qty": 1}, {"item": "polynesian", "qty": 3}]}, "planted": ["variant"]},
                {"name": "count", "ticket": {"lines": [{"item": "spicy_sandwich", "qty": 1}, {"item": "polynesian", "qty": 1}]}, "planted": ["count"]}
            ]}),
        );
        let lem = serde_json::json!({"lines": [{"item": "lemonade", "qty": 1, "size": "lg"}]});
        write_case(
            &dir,
            "b-lemonade",
            "headset",
            serde_json::json!({
            "id": "b-lemonade", "situation": "headset", "noise": "snr5", "truth": lem,
            "variants": [
                {"name": "correct", "ticket": lem, "planted": []},
                {"name": "size", "ticket": {"lines": [{"item": "lemonade", "qty": 1, "size": "md"}]}, "planted": ["size"]}
            ]}),
        );

        let stt = FakeTranscriber::new(&[
            "a spicy chicken sandwich and three polynesian sauces",
            "a large lemonade",
        ]);
        let r = run_eval(&cfa(), &stt, &dir).unwrap();
        assert_eq!(r.cases, 2);
        assert_eq!(r.checks, 5);
        assert_eq!(r.catch_rate, 1.0, "{r:#?}");
        assert_eq!(r.false_alarm_rate, 0.0, "{r:#?}");
        assert_eq!(r.extraction_accuracy, 1.0);
        assert_eq!(r.by_kind["variant"], (1, 1));
        assert_eq!(r.by_situation["headset"], (1.0, 0.0));
        assert_eq!(r.by_noise["snr5"], (1.0, 0.0));
        assert!(r.failures.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missed_catch_is_listed() {
        let dir = std::env::temp_dir().join(format!("oc-eval-miss-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t = serde_json::json!({"lines": [{"item": "cookie", "qty": 1}]});
        write_case(
            &dir,
            "c-cookie",
            "face_to_face",
            serde_json::json!({
            "id": "c-cookie", "situation": "face_to_face", "noise": "clean", "truth": t,
            "variants": [{"name": "extra", "ticket": {"lines": [{"item": "cookie", "qty": 1}, {"item": "water", "qty": 1}]}, "planted": ["extra"]}]}),
        );
        // The transcriber "hears" the water too, so the planted extra is missed.
        let stt = FakeTranscriber::new(&["a cookie and a cup of water"]);
        let r = run_eval(&cfa(), &stt, &dir).unwrap();
        assert_eq!(r.catch_rate, 0.0);
        assert_eq!(r.failures.len(), 2, "{:?}", r.failures); // missed catch + wrong extraction
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
