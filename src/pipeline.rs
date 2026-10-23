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
    let confidence = utts
        .iter()
        .map(|u| u.confidence)
        .fold(None, |m: Option<f32>, c| Some(m.map_or(c, |m| m.min(c))));
    let result = verify(menu, &spoken, ticket, confidence.unwrap_or(0.0));
    let standards = standards_on.then(|| check_standards(menu, &utts, &spoken));
    CheckReport {
        result,
        standards,
        transcript: utts,
        verify_ms: started.elapsed().as_millis() as u32,
    }
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
