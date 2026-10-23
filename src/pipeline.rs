//! The check, end to end: transcript → spoken order → verification.

use std::collections::BTreeMap;
use std::time::Instant;

use serde::Serialize;

use crate::extract::{Utterance, extract};
use crate::matcher::Matcher;
use crate::menu::Menu;
use crate::standards::check_standards;
use crate::stt::Transcriber;
use crate::ticket::Ticket;
use crate::verify::{CheckResult, verify};

#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub result: CheckResult,
    pub standards: Option<BTreeMap<String, bool>>,
    pub transcript: Vec<Utterance>,
    pub verify_ms: u32,
}

/// How much earlier transcript Whisper gets as context for the next segment.
const PROMPT_CONTEXT_CHARS: usize = 200;

pub fn check_utterances(
    menu: &Menu,
    matcher: &Matcher,
    utts: Vec<Utterance>,
    ticket: &Ticket,
    standards_on: bool,
) -> CheckReport {
    let started = Instant::now();
    let spoken = extract(menu, matcher, &utts);
    let result = verify(menu, &spoken, ticket, transcript_confidence(&utts));
    let standards = standards_on.then(|| check_standards(menu, &utts, &spoken));
    CheckReport {
        result,
        standards,
        transcript: utts,
        verify_ms: started.elapsed().as_millis() as u32,
    }
}

/// How sure Whisper was about the order overall: confidence weighted by text
/// length, so a mumbled "my pleasure" doesn't outweigh the order itself.
pub fn transcript_confidence(utts: &[Utterance]) -> f32 {
    let chars: usize = utts.iter().map(|u| u.text.len()).sum();
    if chars == 0 {
        return 0.0;
    }
    utts.iter()
        .map(|u| u.confidence * u.text.len() as f32)
        .sum::<f32>()
        / chars as f32
}

pub fn transcribe_segments(
    t: &dyn Transcriber,
    menu: &Menu,
    segments: &[Vec<f32>],
) -> anyhow::Result<Vec<Utterance>> {
    let mut out: Vec<Utterance> = Vec::new();
    for seg in segments {
        let u = t.transcribe(seg, &prompt_for(menu, &out))?;
        if !u.text.is_empty() {
            out.push(u);
        }
    }
    Ok(out)
}

/// Menu words first, then the tail of what was already said.
pub fn prompt_for(menu: &Menu, earlier: &[Utterance]) -> String {
    let said: String = earlier
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let tail_start = said
        .char_indices()
        .rev()
        .nth(PROMPT_CONTEXT_CHARS.saturating_sub(1))
        .map_or(0, |(i, _)| i);
    format!("{} {}", menu.vocabulary_prompt(), &said[tail_start..])
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::cfa;
    use crate::verify::Outcome;

    #[test]
    fn short_low_confidence_line_does_not_hide_extras() {
        let menu = cfa();
        let utts = vec![
            Utterance {
                text: "Hi, what can I get for you?".into(),
                confidence: 0.9,
            },
            Utterance {
                text: "A side salad and a small Sprite".into(),
                confidence: 0.86,
            },
            Utterance {
                text: "My pleasure".into(),
                confidence: 0.48,
            },
        ];
        let ticket: Ticket = serde_json::from_value(serde_json::json!({"lines": [
            {"item": "side_salad", "qty": 1}, {"item": "lemon_lime", "qty": 1, "size": "sm"}, {"item": "cookie", "qty": 1}]})).unwrap();
        let r = check_utterances(&menu, &Matcher::new(&menu), utts, &ticket, false);
        assert_eq!(r.result.outcome, Outcome::Mismatch, "{:#?}", r.result);
        assert_eq!(
            r.result.mismatches[0].message,
            "Rang 1 Cookie, guest didn't ask"
        );
    }
}
