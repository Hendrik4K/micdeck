//! Folds adapter observations into one consistent picture.

use std::collections::{BTreeMap, VecDeque};

use dji_protocol::DeviceStatus;

use crate::level::Loudness;

use crate::model::{
    AudioEvent, AudioGraph, AudioService, AudioSource, LevelFrame, LinkState, MeterState, MicTestState,
    UsbEvent, UsbReceiver, WritePhase, WriteReport,
};

/// Status frames older than this mean the receiver stopped streaming.
pub(crate) const TELEMETRY_LIVE_MS: u64 = 1_500;
/// After opening the link, how long to wait for the first status frame.
pub(crate) const TELEMETRY_GRACE_MS: u64 = 3_000;
/// After plug-in, how long the kernel and PipeWire get to set up the input.
pub(crate) const AUDIO_SETUP_GRACE_MS: u64 = 5_000;
/// Continuous digital silence for this long means no sound arrives.
pub(crate) const SILENCE_AFTER_MS: u64 = 2_000;
/// A clip stays reported for this long.
pub(crate) const CLIP_HOLD_MS: u64 = 3_000;
/// Level frames older than this mean the meter stopped delivering.
pub(crate) const LEVEL_STALE_MS: u64 = 1_000;
/// A setting change without a final answer after this long counts as lost.
pub(crate) const WRITE_STALE_MS: u64 = 10_000;
/// How many setting changes the log keeps.
const WRITE_LOG_LEN: usize = 30;
/// Setting changes older than this drop out of the log.
pub(crate) const WRITE_KEEP_MS: u64 = 30 * 60 * 1_000;

/// One observation from an adapter or the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Usb(UsbEvent),
    Audio(AudioEvent),
    /// Whether a view currently shows a level meter. Samples are only
    /// measured while one does.
    MeterWanted(bool),
    /// Name of the demo scenario being shown (demo mode only).
    DemoLabel(String),
}

#[derive(Debug, Clone)]
pub(crate) struct ReceiverEntry {
    pub usb: UsbReceiver,
    pub attached_ms: u64,
    pub link_open_ms: Option<u64>,
    pub status: Option<DeviceStatus>,
    pub last_frame_ms: Option<u64>,
}

impl ReceiverEntry {
    /// The decoded status, if a frame arrived recently enough to trust it.
    pub fn live_status(&self, now_ms: u64) -> Option<&DeviceStatus> {
        let fresh = self
            .last_frame_ms
            .is_some_and(|t| now_ms.saturating_sub(t) <= TELEMETRY_LIVE_MS);
        if fresh { self.status.as_ref() } else { None }
    }
}

/// One setting change, as far as it got.
#[derive(Debug, Clone)]
pub(crate) struct WriteRecord {
    pub key: String,
    pub report: WriteReport,
    /// The value the receiver reported when the change was requested.
    pub from: Option<String>,
    /// When the first step arrived (state clock).
    pub started_ms: u64,
}

impl WriteRecord {
    /// The phase, with an unanswered change turned into `NoAnswer` once stale.
    pub fn phase(&self, now_ms: u64) -> WritePhase {
        let stale = now_ms.saturating_sub(self.started_ms) > WRITE_STALE_MS;
        if stale && !self.report.phase.is_final() { WritePhase::NoAnswer } else { self.report.phase.clone() }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SignalTracker {
    pub last_frame_ms: Option<u64>,
    pub last_sound_ms: Option<u64>,
    pub silent_since_ms: Option<u64>,
    pub last_clip_ms: Option<u64>,
    pub last_peak_dbfs: Option<f32>,
    pub loudness: Loudness,
}

impl SignalTracker {
    fn record(&mut self, frame: &LevelFrame, now_ms: u64) {
        self.last_frame_ms = Some(now_ms);
        self.last_peak_dbfs = frame.peak_dbfs.iter().copied().reduce(f32::max);
        if let Some(rms) = frame.rms_dbfs.iter().copied().reduce(f32::max) {
            self.loudness.update(rms, now_ms);
        }
        if frame.digital_silence {
            self.silent_since_ms.get_or_insert(now_ms);
        } else {
            self.silent_since_ms = None;
            self.last_sound_ms = Some(now_ms);
        }
        if frame.clipped {
            self.last_clip_ms = Some(now_ms);
        }
    }
}

/// Everything MicDeck currently knows.
#[derive(Debug, Clone)]
pub struct State {
    pub(crate) simulated: bool,
    pub(crate) demo_label: Option<String>,
    pub(crate) usb_ready: bool,
    pub(crate) usb_watch_error: Option<String>,
    pub(crate) receivers: BTreeMap<String, ReceiverEntry>,
    pub(crate) audio_service: AudioService,
    pub(crate) graph: AudioGraph,
    pub(crate) meter: MeterState,
    pub(crate) meter_wanted: bool,
    pub(crate) signal: SignalTracker,
    pub(crate) mic_test: MicTestState,
    /// Setting changes, oldest first.
    pub(crate) writes: VecDeque<WriteRecord>,
}

impl State {
    /// A fresh state. `simulated` marks every snapshot as demo data.
    pub fn new(simulated: bool) -> Self {
        Self {
            simulated,
            demo_label: None,
            usb_ready: false,
            usb_watch_error: None,
            receivers: BTreeMap::new(),
            audio_service: AudioService::Connecting,
            graph: AudioGraph::default(),
            meter: MeterState::Off,
            meter_wanted: false,
            signal: SignalTracker::default(),
            mic_test: MicTestState::Idle,
            writes: VecDeque::new(),
        }
    }

    pub fn is_simulated(&self) -> bool {
        self.simulated
    }

    /// Apply one observation made at `now_ms` (milliseconds, any monotonic origin).
    pub fn apply(&mut self, input: Input, now_ms: u64) {
        match input {
            Input::Usb(event) => self.apply_usb(event, now_ms),
            Input::Audio(event) => self.apply_audio(event, now_ms),
            Input::MeterWanted(wanted) => {
                self.meter_wanted = wanted;
                if !wanted {
                    self.signal = SignalTracker::default();
                }
            }
            // Only meaningful for demo data; real data never carries a label.
            Input::DemoLabel(label) => {
                if self.simulated {
                    self.demo_label = Some(label);
                }
            }
        }
    }

    fn apply_usb(&mut self, event: UsbEvent, now_ms: u64) {
        match event {
            UsbEvent::Ready => self.usb_ready = true,
            UsbEvent::Receiver(usb) => {
                let key = usb.key.clone();
                let entry = self.receivers.entry(key.clone()).or_insert_with(|| ReceiverEntry {
                    usb: usb.clone(),
                    attached_ms: now_ms,
                    link_open_ms: None,
                    status: None,
                    last_frame_ms: None,
                });
                let was_open = entry.link_open_ms.is_some();
                let is_open = usb.link == LinkState::Open;
                if is_open && !was_open {
                    entry.link_open_ms = Some(now_ms);
                } else if !is_open {
                    entry.link_open_ms = None;
                    entry.status = None;
                    entry.last_frame_ms = None;
                }
                entry.usb = usb;
                if !is_open {
                    self.abandon_writes(&key, "Verbindung zum Empfänger unterbrochen");
                }
            }
            UsbEvent::Removed { key } => {
                self.receivers.remove(&key);
                self.abandon_writes(&key, "Empfänger abgezogen");
            }
            UsbEvent::Telemetry { key, status } => {
                if let Some(entry) = self.receivers.get_mut(&key) {
                    entry.status = Some(status);
                    entry.last_frame_ms = Some(now_ms);
                }
            }
            UsbEvent::WatchFailed { message } => self.usb_watch_error = Some(message),
            UsbEvent::Write { key, report } => self.record_write(key, report, now_ms),
        }
    }

    fn record_write(&mut self, key: String, report: WriteReport, now_ms: u64) {
        if let Some(record) = self.writes.iter_mut().find(|r| r.report.request == report.request) {
            record.report = report;
            return;
        }
        let from = self
            .receivers
            .get(&key)
            .and_then(|r| r.status.as_ref())
            .and_then(|s| s.settings.get(&report.setting).cloned());
        self.writes.push_back(WriteRecord { key, report, from, started_ms: now_ms });
        while self.writes.len() > WRITE_LOG_LEN {
            self.writes.pop_front();
        }
    }

    /// Ends every unfinished change sent to `key`: no answer can come anymore.
    fn abandon_writes(&mut self, key: &str, why: &str) {
        for record in self.writes.iter_mut().filter(|r| r.key == key && !r.report.phase.is_final()) {
            record.report.phase = WritePhase::Failed { message: why.into() };
        }
    }

    /// The change still waiting for an answer, if any.
    pub(crate) fn pending_write(&self, now_ms: u64) -> Option<&WriteRecord> {
        self.writes.iter().rev().find(|r| !r.phase(now_ms).is_final())
    }

    /// The log entry for `request`, once a step of it was recorded.
    pub fn write_record(&self, request: u64) -> Option<(String, Option<String>, WriteReport)> {
        self.writes
            .iter()
            .find(|r| r.report.request == request)
            .map(|r| (r.key.clone(), r.from.clone(), r.report.clone()))
    }

    fn apply_audio(&mut self, event: AudioEvent, now_ms: u64) {
        match event {
            AudioEvent::Service(service) => {
                if service != AudioService::Available {
                    self.graph = AudioGraph::default();
                    self.meter = MeterState::Off;
                    self.signal = SignalTracker::default();
                    self.mic_test = MicTestState::Idle;
                }
                self.audio_service = service;
            }
            AudioEvent::Graph(graph) => {
                self.graph = graph;
                // A graph can only come from a working connection.
                self.audio_service = AudioService::Available;
            }
            AudioEvent::Meter(meter) => {
                if !matches!(meter, MeterState::Running { .. }) {
                    self.signal = SignalTracker::default();
                }
                self.meter = meter;
            }
            AudioEvent::MicTest(test) => self.mic_test = test,
            AudioEvent::Level(frame) => {
                let measuring = matches!(
                    &self.meter,
                    MeterState::Running { node_name, .. } if *node_name == frame.node_name
                );
                if measuring && self.meter_wanted {
                    self.signal.record(&frame, now_ms);
                }
            }
        }
    }

    /// The receiver MicDeck reports on: the first one by USB port.
    pub(crate) fn primary(&self) -> Option<&ReceiverEntry> {
        self.receivers.values().next()
    }

    /// The audio input belonging to the primary receiver, if PipeWire offers one.
    pub(crate) fn dji_source(&self) -> Option<&AudioSource> {
        // The receiver's ALSA card number comes from the kernel (sysfs) and
        // identifies its input even while PipeWire has not yet reported the
        // device's USB IDs, which happens right after plugging it in.
        let cards = self.primary().map(|p| p.usb.alsa_cards.as_slice()).unwrap_or(&[]);
        let sources = &self.graph.sources;
        sources
            .iter()
            .find(|s| s.alsa_card.is_some_and(|c| cards.contains(&c)))
            .or_else(|| sources.iter().find(|s| s.is_dji()))
    }

    /// Whether `source` belongs to a DJI receiver.
    pub(crate) fn is_receiver_source(&self, source: &AudioSource) -> bool {
        source.is_dji() || self.dji_source().is_some_and(|d| d.node_name == source.node_name)
    }

    /// The input the level meter should measure right now, if any.
    pub fn meter_target(&self) -> Option<String> {
        if !self.meter_wanted || self.audio_service != AudioService::Available {
            return None;
        }
        self.dji_source().map(|s| s.node_name.clone())
    }
}
