//! The microphone standing in for the clip. It only runs while an order is
//! open: `stop` drops the stream so the OS releases the mic.

use std::sync::mpsc::Sender;

pub trait AudioSource: Send {
    fn start(&mut self, sink: Sender<Vec<f32>>) -> anyhow::Result<()>;
    fn stop(&mut self);
}

/// No microphone at all. Every start fails, as with a denied permission.
pub struct NoMic;

impl AudioSource for NoMic {
    fn start(&mut self, _sink: Sender<Vec<f32>>) -> anyhow::Result<()> {
        anyhow::bail!("no microphone available")
    }

    fn stop(&mut self) {}
}

/// The default system microphone. The stream lives on its own thread
/// (macOS streams can't move between threads) and is dropped on `stop`.
#[derive(Default)]
pub struct CpalMic {
    stop: Option<Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CpalMic {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AudioSource for CpalMic {
    fn start(&mut self, sink: Sender<Vec<f32>>) -> anyhow::Result<()> {
        self.stop();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<anyhow::Result<()>>();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::spawn(move || match open_stream(sink) {
            Ok(stream) => {
                let _ = ready_tx.send(Ok(()));
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        });
        ready_rx
            .recv()
            .map_err(|_| anyhow::anyhow!("microphone thread died"))??;
        self.stop = Some(stop_tx);
        self.thread = Some(thread);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn open_stream(sink: Sender<Vec<f32>>) -> anyhow::Result<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("no input device"))?;
    let supported = device.default_input_config()?;
    let rate = supported.sample_rate();
    let channels = supported.channels().max(1) as usize;
    let config = supported.config();
    let on_error = |e| eprintln!("microphone error: {e}");
    let to_16k =
        move |mono: Vec<f32>| crate::audio::resample(&mono, rate, crate::audio::SAMPLE_RATE);

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &_| {
                let mono = data
                    .chunks(channels)
                    .map(|c| c.iter().sum::<f32>() / channels as f32)
                    .collect();
                let _ = sink.send(to_16k(mono));
            },
            on_error,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &_| {
                let mono = data
                    .chunks(channels)
                    .map(|c| {
                        c.iter().map(|&s| s as f32 / i16::MAX as f32).sum::<f32>() / channels as f32
                    })
                    .collect();
                let _ = sink.send(to_16k(mono));
            },
            on_error,
            None,
        )?,
        other => anyhow::bail!("unsupported microphone sample format {other:?}"),
    };
    stream.play()?;
    Ok(stream)
}
