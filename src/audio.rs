//! Audio in: WAV files for tests and eval, and a simple energy-based
//! segmenter that cuts live audio into utterances at pauses.

use std::path::Path;

pub const SAMPLE_RATE: u32 = 16_000;

const FRAME: usize = 480; // 30 ms
const END_SILENCE_FRAMES: usize = 20; // 600 ms of quiet ends an utterance
const MIN_SPEECH_FRAMES: usize = 10; // under 300 ms of speech is a blip
const PAD_FRAMES: usize = 5; // 150 ms of context kept on each side
const MAX_FRAMES: usize = 833; // ~25 s; Whisper reads 30 s windows
const MIN_THRESHOLD: f32 = 0.01;

/// Reads any PCM WAV and returns 16 kHz mono samples in [-1, 1].
pub fn load_wav_16k_mono(path: &Path) -> anyhow::Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()?
        }
    };
    let channels = spec.channels.max(1) as usize;
    let mono: Vec<f32> = interleaved
        .chunks(channels)
        .map(|c| c.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok(resample(&mono, spec.sample_rate, SAMPLE_RATE))
}

/// Linear-interpolation resampling. Good enough for speech recognition.
pub fn resample(pcm: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || pcm.is_empty() {
        return pcm.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (pcm.len() as f64 / ratio).floor() as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos.floor() as usize;
            let frac = (pos - j as f64) as f32;
            let a = pcm[j];
            let b = *pcm.get(j + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}

/// Cuts a stream of 16 kHz audio into utterances at pauses, using frame
/// energy against a running noise floor.
pub struct Segmenter {
    leftover: Vec<f32>,
    floor: Option<f32>,
    pre_roll: std::collections::VecDeque<Vec<f32>>,
    current: Vec<f32>,
    in_speech: bool,
    speech_frames: usize,
    silent_frames: usize,
    frames: usize,
}

impl Segmenter {
    pub fn new() -> Self {
        Segmenter {
            leftover: Vec::new(),
            floor: None,
            pre_roll: Default::default(),
            current: Vec::new(),
            in_speech: false,
            speech_frames: 0,
            silent_frames: 0,
            frames: 0,
        }
    }

    pub fn push(&mut self, pcm: &[f32]) -> Vec<Vec<f32>> {
        self.leftover.extend_from_slice(pcm);
        let mut out = Vec::new();
        let whole = self.leftover.len() / FRAME * FRAME;
        let frames: Vec<Vec<f32>> = self.leftover[..whole]
            .chunks(FRAME)
            .map(|c| c.to_vec())
            .collect();
        self.leftover.drain(..whole);
        for frame in frames {
            if let Some(utt) = self.frame(frame) {
                out.push(utt);
            }
        }
        out
    }

    pub fn flush(&mut self) -> Option<Vec<f32>> {
        let rest = std::mem::take(&mut self.leftover);
        if self.in_speech {
            self.current.extend(rest);
        }
        self.finish()
    }

    fn frame(&mut self, frame: Vec<f32>) -> Option<Vec<f32>> {
        let rms = (frame.iter().map(|x| x * x).sum::<f32>() / frame.len() as f32).sqrt();
        let floor = self.floor.get_or_insert(rms.clamp(1e-4, MIN_THRESHOLD));
        let speech = rms > MIN_THRESHOLD.max(3.0 * *floor);
        // Track the quietest recent level, drifting up slowly so a noisy room raises it.
        *floor = if rms < *floor {
            rms.max(1e-4)
        } else {
            *floor * 1.005
        };

        if !self.in_speech {
            if speech {
                self.in_speech = true;
                self.current = self.pre_roll.drain(..).flatten().collect();
                self.current.extend(frame);
                self.speech_frames = 1;
                self.silent_frames = 0;
                self.frames = 1;
            } else {
                self.pre_roll.push_back(frame);
                if self.pre_roll.len() > PAD_FRAMES {
                    self.pre_roll.pop_front();
                }
            }
            return None;
        }

        self.current.extend(frame);
        self.frames += 1;
        if speech {
            self.speech_frames += 1;
            self.silent_frames = 0;
        } else {
            self.silent_frames += 1;
        }
        if self.silent_frames >= END_SILENCE_FRAMES || self.frames >= MAX_FRAMES {
            return self.finish();
        }
        None
    }

    fn finish(&mut self) -> Option<Vec<f32>> {
        if !self.in_speech {
            return None;
        }
        let extra_silence = self.silent_frames.saturating_sub(PAD_FRAMES) * FRAME;
        let keep = self.current.len().saturating_sub(extra_silence);
        self.current.truncate(keep);
        self.in_speech = false;
        self.silent_frames = 0;
        let utt = std::mem::take(&mut self.current);
        (self.speech_frames >= MIN_SPEECH_FRAMES).then_some(utt)
    }
}

impl Default for Segmenter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(secs: f32, amp: f32) -> Vec<f32> {
        let n = (secs * SAMPLE_RATE as f32) as usize;
        (0..n)
            .map(|i| amp * (i as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin())
            .collect()
    }

    fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (secs * SAMPLE_RATE as f32) as usize]
    }

    fn segments(parts: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let mut seg = Segmenter::new();
        let mut out = Vec::new();
        // Feed in 100 ms chunks, the way a microphone delivers audio.
        for chunk in parts.concat().chunks(1600) {
            out.extend(seg.push(chunk));
        }
        out.extend(seg.flush());
        out
    }

    #[test]
    fn segmenter_splits_on_silence() {
        let segs = segments(&[tone(0.5, 0.3), silence(1.0), tone(0.5, 0.3)]);
        assert_eq!(segs.len(), 2);
        for s in &segs {
            let secs = s.len() as f32 / SAMPLE_RATE as f32;
            assert!((0.5..=1.0).contains(&secs), "segment is {secs}s");
        }
    }

    #[test]
    fn segmenter_drops_blips() {
        assert!(segments(&[silence(0.5), tone(0.1, 0.3), silence(1.0)]).is_empty());
    }

    #[test]
    fn segmenter_keeps_trailing_speech_on_flush() {
        assert_eq!(segments(&[silence(0.3), tone(0.8, 0.3)]).len(), 1);
    }

    #[test]
    fn short_pause_does_not_split() {
        assert_eq!(
            segments(&[tone(0.5, 0.3), silence(0.3), tone(0.5, 0.3)]).len(),
            1
        );
    }

    #[test]
    fn loads_and_resamples_wav() {
        let dir = std::env::temp_dir().join(format!("oc-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stereo48k.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..48_000 {
            w.write_sample(i16::MAX / 2).unwrap();
            w.write_sample(i16::MAX / 2).unwrap();
        }
        w.finalize().unwrap();
        let pcm = load_wav_16k_mono(&path).unwrap();
        assert!(
            (15_900..=16_100).contains(&pcm.len()),
            "{} samples",
            pcm.len()
        );
        assert!((pcm[8000] - 0.5).abs() < 0.01);
    }
}
