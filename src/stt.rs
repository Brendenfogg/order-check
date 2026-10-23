//! Speech-to-text. Whisper runs locally through whisper.cpp; audio never leaves the machine.

use std::path::Path;
use std::sync::Mutex;

use crate::extract::Utterance;

pub trait Transcriber: Send + Sync {
    fn transcribe(&self, pcm: &[f32], prompt: &str) -> anyhow::Result<Utterance>;
}

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

/// whisper.cpp refuses clips under a second, so short ones are padded with silence.
const MIN_SAMPLES: usize = 17_600;
/// Segments Whisper itself thinks are probably not speech are dropped,
/// which keeps it from "hearing" menu words in background noise.
const NO_SPEECH_CUTOFF: f32 = 0.6;

pub struct WhisperTranscriber {
    state: Mutex<WhisperState>,
    eot: i32,
}

impl WhisperTranscriber {
    pub fn new(model: &Path) -> anyhow::Result<Self> {
        // OC_WHISPER_LOG=1 shows whisper.cpp's own logs (GPU backend, timings).
        if std::env::var("OC_WHISPER_LOG").is_err() {
            whisper_rs::install_logging_hooks();
        }
        let path = model
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("model path is not UTF-8"))?;
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("loading {}: {e}", model.display()))?;
        let eot = ctx.token_eot();
        let state = ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("creating whisper state: {e}"))?;
        Ok(WhisperTranscriber {
            state: Mutex::new(state),
            eot,
        })
    }
}

impl Transcriber for WhisperTranscriber {
    fn transcribe(&self, pcm: &[f32], prompt: &str) -> anyhow::Result<Utterance> {
        let mut audio = pcm.to_vec();
        if audio.len() < MIN_SAMPLES {
            audio.resize(MIN_SAMPLES, 0.0);
        }
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_n_threads(4);
        params.set_no_timestamps(true);
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_initial_prompt(prompt);

        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("whisper state poisoned"))?;
        state
            .full(params, &audio)
            .map_err(|e| anyhow::anyhow!("whisper: {e}"))?;

        let mut text = String::new();
        let mut probs = Vec::new();
        for seg in state.as_iter() {
            if seg.no_speech_probability() > NO_SPEECH_CUTOFF {
                continue;
            }
            text.push_str(
                &seg.to_str_lossy()
                    .map_err(|e| anyhow::anyhow!("whisper text: {e}"))?,
            );
            for t in 0..seg.n_tokens() {
                if let Some(tok) = seg.get_token(t)
                    && tok.token_id() < self.eot
                {
                    probs.push(tok.token_probability());
                }
            }
        }
        let confidence = if probs.is_empty() {
            0.0
        } else {
            probs.iter().sum::<f32>() / probs.len() as f32
        };
        Ok(Utterance {
            text: text.trim().to_string(),
            confidence,
        })
    }
}

/// Returns queued utterances in order, one per call. For tests.
pub struct FakeTranscriber(pub Mutex<std::collections::VecDeque<Utterance>>);

impl FakeTranscriber {
    pub fn new(texts: &[&str]) -> Self {
        FakeTranscriber(Mutex::new(
            texts
                .iter()
                .map(|t| Utterance {
                    text: t.to_string(),
                    confidence: 0.95,
                })
                .collect(),
        ))
    }
}

impl Transcriber for FakeTranscriber {
    fn transcribe(&self, _pcm: &[f32], _prompt: &str) -> anyhow::Result<Utterance> {
        Ok(self.0.lock().unwrap().pop_front().unwrap_or(Utterance {
            text: String::new(),
            confidence: 0.0,
        }))
    }
}
