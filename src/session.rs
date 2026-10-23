//! Order sessions: listening starts when an order starts, the transcript
//! lives only in memory, and it is dropped on send, cancel, or timeout.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail};

use serde::Serialize;
use tokio::sync::broadcast;

use crate::audio::Segmenter;
use crate::extract::Utterance;
use crate::matcher::Matcher;
use crate::menu::Menu;
use crate::mic::AudioSource;
use crate::pipeline::{CheckReport, check_utterances, prompt_for};
use crate::records::{OrderRecord, Records};
use crate::stt::Transcriber;
use crate::ticket::Ticket;
use crate::verify::{CheckResult, Outcome};

pub const ABANDON_AFTER: Duration = Duration::from_secs(10 * 60);
/// No order takes this long to say. If nobody has pressed Finalize by then
/// (a closed POS tab, a crash), the mic turns itself off.
pub const MAX_LISTEN: Duration = Duration::from_secs(5 * 60);
pub const FIRST_ORDER: u32 = 1001;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipState {
    Off,
    Listening,
    Checking,
    Mismatch,
    Unconfirmed,
    Ok,
    Ready,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Clip { order_num: u32, state: ClipState },
    Transcript { order_num: u32, text: String },
    Ready { order_num: u32 },
}

pub struct SessionConfig {
    pub standards_on: bool,
    pub demo_transcript: bool,
}

pub struct Sessions {
    menu: Arc<Menu>,
    matcher: Arc<Matcher>,
    stt: Arc<dyn Transcriber>,
    records: Records,
    mic: Mutex<Box<dyn AudioSource>>,
    config: SessionConfig,
    events: broadcast::Sender<Event>,
    inner: Mutex<Inner>,
}

struct Inner {
    next_num: u32,
    orders: HashMap<u32, Order>,
    /// The order the mic is currently feeding, if any.
    listening: Option<u32>,
}

struct Order {
    mode: String,
    started: Instant,
    offline: bool,
    audio: Option<Sender<Vec<f32>>>,
    pump: Option<JoinHandle<()>>,
    utts: Arc<Mutex<Vec<Utterance>>>,
    first: Option<(CheckResult, u32)>,
    last: Option<CheckReport>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Sessions {
    pub fn new(
        menu: Arc<Menu>,
        stt: Arc<dyn Transcriber>,
        records: Records,
        mic: Box<dyn AudioSource>,
        config: SessionConfig,
        events: broadcast::Sender<Event>,
    ) -> Self {
        let matcher = Arc::new(Matcher::new(&menu));
        Sessions {
            menu,
            matcher,
            stt,
            records,
            mic: Mutex::new(mic),
            config,
            events,
            inner: Mutex::new(Inner {
                next_num: FIRST_ORDER,
                orders: HashMap::new(),
                listening: None,
            }),
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    fn clip(&self, order_num: u32, state: ClipState) {
        self.emit(Event::Clip { order_num, state });
    }

    /// Starts an order and the listening that goes with it.
    pub fn start(&self, mode: &str, _label: &str) -> anyhow::Result<u32> {
        let mut inner = lock(&self.inner);
        if let Some(prev) = inner.listening.take() {
            self.stop_listening(&mut inner, prev);
        }
        let n = inner.next_num;
        inner.next_num += 1;

        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let utts = Arc::new(Mutex::new(Vec::new()));
        let pump = self.spawn_pump(n, rx, utts.clone());
        let offline = lock(&self.mic).start(tx.clone()).is_err();
        inner.orders.insert(
            n,
            Order {
                mode: mode.to_string(),
                started: Instant::now(),
                offline,
                audio: Some(tx),
                pump: Some(pump),
                utts,
                first: None,
                last: None,
            },
        );
        if !offline {
            inner.listening = Some(n);
        }
        drop(inner);
        self.clip(
            n,
            if offline {
                ClipState::Offline
            } else {
                ClipState::Listening
            },
        );
        Ok(n)
    }

    /// Cuts incoming audio into utterances and transcribes each one as soon
    /// as it ends, so only the last few seconds are left at Finalize.
    fn spawn_pump(
        &self,
        n: u32,
        rx: Receiver<Vec<f32>>,
        utts: Arc<Mutex<Vec<Utterance>>>,
    ) -> JoinHandle<()> {
        let stt = self.stt.clone();
        let menu = self.menu.clone();
        let events = self.events.clone();
        let demo = self.config.demo_transcript;
        std::thread::spawn(move || {
            let mut seg = Segmenter::new();
            let transcribe = |pcm: Vec<f32>| {
                let prompt = prompt_for(&menu, &lock(&utts));
                match stt.transcribe(&pcm, &prompt) {
                    Ok(u) if !u.text.is_empty() => {
                        if demo {
                            let _ = events.send(Event::Transcript {
                                order_num: n,
                                text: u.text.clone(),
                            });
                        }
                        lock(&utts).push(u);
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("order {n}: transcription failed: {e:#}"),
                }
            };
            for chunk in rx {
                for pcm in seg.push(&chunk) {
                    transcribe(pcm);
                }
            }
            if let Some(pcm) = seg.flush() {
                transcribe(pcm);
            }
        })
    }

    fn stop_listening(&self, inner: &mut Inner, n: u32) {
        if inner.listening == Some(n) {
            lock(&self.mic).stop();
            inner.listening = None;
        }
        if let Some(o) = inner.orders.get_mut(&n) {
            o.audio = None;
        }
    }

    pub fn push_audio(&self, order_num: u32, pcm: &[f32]) {
        let inner = lock(&self.inner);
        if let Some(tx) = inner.orders.get(&order_num).and_then(|o| o.audio.as_ref()) {
            let _ = tx.send(pcm.to_vec());
        }
    }

    pub fn finalize(&self, order_num: u32, ticket: &Ticket) -> anyhow::Result<CheckReport> {
        let started = Instant::now();
        let (pump, utts, offline) = {
            let mut inner = lock(&self.inner);
            if !inner.orders.contains_key(&order_num) {
                bail!("unknown order {order_num}");
            }
            self.stop_listening(&mut inner, order_num);
            let o = inner.orders.get_mut(&order_num).expect("checked above");
            (o.pump.take(), o.utts.clone(), o.offline)
        };
        self.clip(order_num, ClipState::Checking);
        // Wait for the last utterance to be transcribed.
        if let Some(p) = pump {
            let _ = p.join();
        }
        let transcript = lock(&utts).clone();
        let mut report = check_utterances(
            &self.menu,
            &self.matcher,
            transcript,
            ticket,
            self.config.standards_on,
        );
        if offline && report.transcript.is_empty() {
            report.result = CheckResult {
                outcome: Outcome::Unconfirmed,
                mismatches: Vec::new(),
                unconfirmed: Vec::new(),
                reason: Some("clip offline".into()),
            };
        }
        let finalize_ms = started.elapsed().as_millis() as u32;

        let mut inner = lock(&self.inner);
        let o = inner
            .orders
            .get_mut(&order_num)
            .ok_or_else(|| anyhow!("order {order_num} expired"))?;
        if o.first.is_none() {
            o.first = Some((report.result.clone(), finalize_ms));
        }
        o.last = Some(report.clone());
        drop(inner);
        self.clip(
            order_num,
            match report.result.outcome {
                Outcome::Match => ClipState::Ok,
                Outcome::Mismatch => ClipState::Mismatch,
                Outcome::Unconfirmed => ClipState::Unconfirmed,
            },
        );
        Ok(report)
    }

    /// The order goes to the kitchen: write the anonymous record and drop the transcript.
    pub fn send(&self, order_num: u32, as_rung: bool) -> anyhow::Result<OrderRecord> {
        let mut inner = lock(&self.inner);
        let Some(o) = inner.orders.get(&order_num) else {
            bail!("unknown order {order_num}")
        };
        let (Some((first, latency_ms)), Some(last)) = (o.first.as_ref(), o.last.as_ref()) else {
            bail!("order {order_num} has not been checked yet");
        };
        let caught = first.mismatches.len() as u32;
        let mut kinds: Vec<_> = first.mismatches.iter().map(|m| m.kind).collect();
        kinds.sort();
        kinds.dedup();
        let record = OrderRecord {
            order_num,
            mode: o.mode.clone(),
            ts: unix_now(),
            outcome: last.result.outcome,
            caught,
            kinds,
            pairs: first
                .mismatches
                .iter()
                .map(|m| (m.said.clone(), m.rang.clone()))
                .collect(),
            fixed: caught > 0 && last.result.outcome == Outcome::Match && !as_rung,
            sent_as_rung: as_rung,
            standards: last.standards.clone(),
            latency_ms: *latency_ms,
        };
        self.records.insert(&record)?;
        self.stop_listening(&mut inner, order_num);
        inner.orders.remove(&order_num);
        drop(inner);
        self.clip(order_num, ClipState::Ok);
        Ok(record)
    }

    pub fn cancel(&self, order_num: u32) {
        let mut inner = lock(&self.inner);
        self.stop_listening(&mut inner, order_num);
        let removed = inner.orders.remove(&order_num).is_some();
        drop(inner);
        if removed {
            self.clip(order_num, ClipState::Off);
        }
    }

    /// Drops orders left open too long. Their transcripts go with them; no record is kept.
    pub fn expire(&self, now: Instant) -> Vec<u32> {
        let mut inner = lock(&self.inner);
        let overlong = inner.listening.filter(|n| {
            inner
                .orders
                .get(n)
                .is_some_and(|o| now.saturating_duration_since(o.started) > MAX_LISTEN)
        });
        if let Some(n) = overlong {
            self.stop_listening(&mut inner, n);
            self.clip(n, ClipState::Off);
        }
        let stale: Vec<u32> = inner
            .orders
            .iter()
            .filter(|(_, o)| now.saturating_duration_since(o.started) > ABANDON_AFTER)
            .map(|(n, _)| *n)
            .collect();
        for n in &stale {
            self.stop_listening(&mut inner, *n);
            inner.orders.remove(n);
        }
        drop(inner);
        for n in &stale {
            self.clip(*n, ClipState::Off);
        }
        stale
    }

    pub fn has_transcript(&self, order_num: u32) -> bool {
        lock(&self.inner).orders.contains_key(&order_num)
    }

    pub fn records(&self) -> &Records {
        &self.records
    }

    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn ready(&self, order_num: u32) {
        self.clip(order_num, ClipState::Ready);
        self.emit(Event::Ready { order_num });
    }
}
