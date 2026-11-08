//! Microphone session: listen, record what is said (up to `MAX_TAKE`), play
//! it back, and record a new take as soon as the user speaks again – also in
//! the middle of a playback, which then fades and stops.
//!
//! One capture stream runs for the whole session; only the current take and
//! a short pre-roll live in memory, and nothing is written anywhere. The
//! session ends on its own when nobody speaks for a while.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::Cursor;
use std::rc::Rc;
use std::time::{Duration, Instant};

use micdeck_core::level::to_dbfs;
use micdeck_core::voice::VoiceDetector;
use micdeck_core::{AudioEvent, MicTestState};
use pipewire as pw;
use pw::properties::properties;
use pw::spa::param::audio::{AudioFormat, AudioInfoRaw};
use pw::spa::param::ParamType;
use pw::spa::pod::serialize::PodSerializer;
use pw::spa::pod::{Object, Pod, Value};
use pw::spa::utils::{Direction, SpaTypes};
use pw::stream::{StreamFlags, StreamListener, StreamRc, StreamState};

use crate::Sink;

/// Longest take.
pub const MAX_TAKE: Duration = Duration::from_secs(15);
/// Speech must last this long to start a take …
const START_MS: f32 = 150.0;
/// … and this long to interrupt a playback.
const BARGE_MS: f32 = 250.0;
/// This much quiet ends a take.
const END_SILENCE_MS: f32 = 1_200.0;
/// Kept before speech was detected, so the first syllable isn't cut.
const PREROLL: Duration = Duration::from_millis(400);
/// Quiet left at the end of a take when it ended by silence.
const KEEP_TAIL_MS: f32 = 300.0;
/// No speech this long after starting: end with a hint.
const FIRST_LISTEN_TIMEOUT: Duration = Duration::from_secs(8);
/// No speech this long after a playback: the session is over.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(6);
/// At the start of a playback, measure how loud its echo reaches the mic …
const ECHO_CALIBRATION: Duration = Duration::from_millis(500);
/// … and only let a voice this much louder interrupt.
const ECHO_MARGIN_DB: f32 = 10.0;
/// Interrupted playback drops to this volume, then stops.
const DUCK_GAIN: f32 = 0.2;
const DUCK_TIME: Duration = Duration::from_millis(250);
/// Let the last buffers reach the speakers before closing the stream.
const PLAYBACK_TAIL: Duration = Duration::from_millis(400);
/// The capture stream must deliver audio within this time.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Capture {
    channels: u32,
    rate: u32,
    recording: bool,
    take: Vec<f32>,
    preroll: VecDeque<f32>,
    voice: VoiceDetector,
    /// Loudest level since the last reset (echo calibration).
    window_max_db: f32,
    failed: Option<String>,
}

impl Capture {
    fn samples_for(&self, d: Duration) -> usize {
        (self.rate as u128 * d.as_millis() / 1000) as usize * self.channels as usize
    }

    fn start_take(&mut self) {
        self.take = self.preroll.drain(..).collect();
        self.recording = true;
        self.voice.reset_counters();
    }
}

struct PlayData {
    samples: Vec<f32>,
    channels: usize,
    pos: usize,
    gain: Rc<Cell<f32>>,
    done: Rc<Cell<bool>>,
}

struct Playback {
    stream: StreamRc,
    _listener: StreamListener<PlayData>,
    gain: Rc<Cell<f32>>,
    done: Rc<Cell<bool>>,
    started: Instant,
    calibrated: bool,
    ducking_since: Option<Instant>,
    done_at: Option<Instant>,
}

enum Phase {
    Listening { since: Instant, first: bool },
    Recording,
    Playing(Playback),
}

pub(crate) struct MicSession {
    stream: StreamRc,
    _listener: StreamListener<Rc<RefCell<Capture>>>,
    capture: Rc<RefCell<Capture>>,
    phase: Option<Phase>,
    sink: Sink,
    started: Instant,
}

fn format_pod(rate: u32, channels: u32) -> Result<Vec<u8>, String> {
    let mut format = AudioInfoRaw::new();
    format.set_format(AudioFormat::F32LE);
    if rate > 0 {
        format.set_rate(rate);
    }
    if channels > 0 {
        format.set_channels(channels);
        let mut position = [0; pw::spa::sys::SPA_AUDIO_MAX_CHANNELS as usize];
        if channels == 2 {
            position[0] = pw::spa::sys::SPA_AUDIO_CHANNEL_FL;
            position[1] = pw::spa::sys::SPA_AUDIO_CHANNEL_FR;
        } else {
            position[0] = pw::spa::sys::SPA_AUDIO_CHANNEL_MONO;
        }
        format.set_position(position);
    }
    let object = Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: format.into(),
    };
    PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(object))
        .map(|(c, _)| c.into_inner())
        .map_err(|e| format!("Format konnte nicht beschrieben werden ({e:?})"))
}

impl MicSession {
    /// Start listening on the input with this `node.name`.
    pub fn start(core: &pw::core::CoreRc, target: &str, sink: Sink) -> Result<Self, String> {
        let props = properties! {
            "media.type" => "Audio",
            "media.category" => "Capture",
            "media.role" => "Production",
            "media.name" => "Mikrofontest",
            "application.name" => "MicDeck",
            "node.name" => "micdeck-mictest",
            "target.object" => target,
            "node.dont-reconnect" => "true",
            "stream.dont-remix" => "true",
        };
        let stream = StreamRc::new(core.clone(), "micdeck-mictest", props)
            .map_err(|e| format!("Aufnahme konnte nicht erstellt werden ({e})"))?;
        let capture = Rc::new(RefCell::new(Capture { window_max_db: f32::NEG_INFINITY, ..Default::default() }));
        let listener = stream
            .add_local_listener_with_user_data(capture.clone())
            .state_changed(|_, data, _, new| {
                if let StreamState::Error(message) = new {
                    data.borrow_mut().failed = Some(message);
                }
            })
            .param_changed(|_, data, id, param| {
                let Some(param) = param else { return };
                if id != ParamType::Format.as_raw() {
                    return;
                }
                let mut info = AudioInfoRaw::new();
                if info.parse(param).is_ok() && info.channels() > 0 && info.rate() > 0 {
                    let mut c = data.borrow_mut();
                    c.channels = info.channels();
                    c.rate = info.rate();
                }
            })
            .process(|stream, data| {
                let Some(mut buffer) = stream.dequeue_buffer() else { return };
                let mut c = data.borrow_mut();
                if c.rate == 0 {
                    return;
                }
                let Some(chunk) = buffer.datas_mut().first_mut() else { return };
                let (offset, size) = (chunk.chunk().offset() as usize, chunk.chunk().size() as usize);
                let Some(bytes) = chunk.data() else { return };
                let end = offset.saturating_add(size).min(bytes.len());
                let samples: Vec<f32> =
                    bytes[offset.min(end)..end].as_chunks::<4>().0.iter().map(|s| f32::from_le_bytes(*s)).collect();
                let channels = c.channels as usize;
                let frames = samples.len() / channels.max(1);
                if frames == 0 {
                    return;
                }
                // Level of the louder channel decides about speech.
                let rms = (0..channels)
                    .map(|ch| {
                        let sum: f32 = samples.iter().skip(ch).step_by(channels).map(|s| s * s).sum();
                        (sum / frames as f32).sqrt()
                    })
                    .fold(0.0, f32::max);
                let db = to_dbfs(rms);
                let dt_ms = frames as f32 * 1000.0 / c.rate as f32;
                c.voice.feed(db, dt_ms);
                c.window_max_db = c.window_max_db.max(db);
                if c.recording {
                    let room = c.samples_for(MAX_TAKE).saturating_sub(c.take.len());
                    c.take.extend(samples.iter().take(room));
                } else {
                    let keep = c.samples_for(PREROLL);
                    c.preroll.extend(samples);
                    let excess = c.preroll.len().saturating_sub(keep);
                    c.preroll.drain(..excess);
                }
            })
            .register()
            .map_err(|e| format!("Aufnahme konnte nicht eingerichtet werden ({e})"))?;
        let bytes = format_pod(0, 0)?;
        let pod = Pod::from_bytes(&bytes).ok_or("Ungültige Formatbeschreibung")?;
        stream
            .connect(Direction::Input, None, StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS, &mut [pod])
            .map_err(|e| format!("Aufnahme konnte nicht verbunden werden ({e})"))?;
        sink(AudioEvent::MicTest(MicTestState::Listening { first: true }));
        Ok(Self {
            stream,
            _listener: listener,
            capture,
            phase: Some(Phase::Listening { since: Instant::now(), first: true }),
            sink,
            started: Instant::now(),
        })
    }

    /// Advance the session; returns `false` once it is over.
    pub fn poll(&mut self, core: &pw::core::CoreRc) -> bool {
        let failed = self.capture.borrow_mut().failed.take();
        if let Some(message) = failed {
            return self.end(Some(message));
        }
        let no_audio = self.capture.borrow().rate == 0;
        if no_audio && self.started.elapsed() > CAPTURE_TIMEOUT {
            return self.end(Some("Vom Mikrofon kam kein Ton an".into()));
        }
        let next = match self.phase.take() {
            Some(Phase::Listening { since, first }) => {
                let mut c = self.capture.borrow_mut();
                if c.voice.speaking_for(START_MS) {
                    c.start_take();
                    drop(c);
                    self.emit(MicTestState::Recording { max_ms: MAX_TAKE.as_millis() as u64 });
                    Phase::Recording
                } else if since.elapsed() > if first { FIRST_LISTEN_TIMEOUT } else { LISTEN_TIMEOUT } {
                    drop(c);
                    let hint = first.then(|| "Nichts gehört – ist das Mikrofon stumm oder zu leise?".to_string());
                    return self.end(hint);
                } else {
                    Phase::Listening { since, first }
                }
            }
            Some(Phase::Recording) => {
                let mut c = self.capture.borrow_mut();
                let full = c.take.len() >= c.samples_for(MAX_TAKE);
                let quiet = c.voice.silence_ms();
                if !full && quiet < END_SILENCE_MS {
                    Phase::Recording
                } else {
                    c.recording = false;
                    let mut take = std::mem::take(&mut c.take);
                    if !full {
                        // Drop most of the closing silence.
                        let cut = c.samples_for(Duration::from_millis((quiet - KEEP_TAIL_MS).max(0.0) as u64));
                        take.truncate(take.len().saturating_sub(cut));
                    }
                    let (rate, channels) = (c.rate, c.channels);
                    // Block interrupting until the echo has been measured.
                    c.voice.set_echo_level(Some(0.0), 0.0);
                    c.window_max_db = f32::NEG_INFINITY;
                    drop(c);
                    let duration_ms = (take.len() / channels.max(1) as usize) as u64 * 1000 / rate.max(1) as u64;
                    match play(core, take, rate, channels) {
                        Ok(playback) => {
                            self.emit(MicTestState::Playing { duration_ms });
                            Phase::Playing(playback)
                        }
                        Err(message) => return self.end(Some(message)),
                    }
                }
            }
            Some(Phase::Playing(mut p)) => {
                let mut c = self.capture.borrow_mut();
                if !p.calibrated && p.started.elapsed() >= ECHO_CALIBRATION {
                    let echo = c.window_max_db;
                    c.voice.set_echo_level(Some(echo), ECHO_MARGIN_DB);
                    p.calibrated = true;
                    log::debug!("mic test: echo {echo:.0} dBFS, interrupt above {:.0}", echo + ECHO_MARGIN_DB);
                }
                if p.calibrated && p.ducking_since.is_none() && c.voice.speaking_for(BARGE_MS) {
                    p.gain.set(DUCK_GAIN);
                    p.ducking_since = Some(Instant::now());
                    log::info!("mic test: playback interrupted by speech");
                }
                if p.ducking_since.is_some_and(|t| t.elapsed() >= DUCK_TIME) {
                    let _ = p.stream.disconnect();
                    c.voice.set_echo_level(None, 0.0);
                    c.start_take();
                    drop(c);
                    self.emit(MicTestState::Recording { max_ms: MAX_TAKE.as_millis() as u64 });
                    Phase::Recording
                } else {
                    if p.done.get() && p.done_at.is_none() {
                        p.done_at = Some(Instant::now());
                    }
                    if p.done_at.is_some_and(|t| t.elapsed() >= PLAYBACK_TAIL) {
                        let _ = p.stream.disconnect();
                        c.voice.set_echo_level(None, 0.0);
                        c.voice.reset_counters();
                        drop(c);
                        self.emit(MicTestState::Listening { first: false });
                        Phase::Listening { since: Instant::now(), first: false }
                    } else {
                        Phase::Playing(p)
                    }
                }
            }
            None => return false,
        };
        self.phase = Some(next);
        true
    }

    fn emit(&self, state: MicTestState) {
        (self.sink)(AudioEvent::MicTest(state));
    }

    /// End the session, with a message if it failed.
    fn end(&mut self, failure: Option<String>) -> bool {
        if let Some(Phase::Playing(p)) = self.phase.take() {
            let _ = p.stream.disconnect();
        }
        let _ = self.stream.disconnect();
        match failure {
            Some(message) => {
                log::info!("microphone session ended: {message}");
                self.emit(MicTestState::Failed { message });
            }
            None => self.emit(MicTestState::Idle),
        }
        false
    }
}

impl Drop for MicSession {
    fn drop(&mut self) {
        if self.phase.is_some() {
            self.end(None);
        }
    }
}

fn play(core: &pw::core::CoreRc, samples: Vec<f32>, rate: u32, channels: u32) -> Result<Playback, String> {
    let props = properties! {
        "media.type" => "Audio",
        "media.category" => "Playback",
        "media.role" => "Production",
        "media.name" => "Mikrofontest – Wiedergabe",
        "application.name" => "MicDeck",
        "node.name" => "micdeck-mictest-playback",
    };
    let stream = StreamRc::new(core.clone(), "micdeck-mictest-playback", props)
        .map_err(|e| format!("Wiedergabe konnte nicht erstellt werden ({e})"))?;
    let gain = Rc::new(Cell::new(1.0));
    let done = Rc::new(Cell::new(false));
    let data = PlayData { samples, channels: channels.max(1) as usize, pos: 0, gain: gain.clone(), done: done.clone() };
    let listener = stream
        .add_local_listener_with_user_data(data)
        .process(|stream, data| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            let stride = 4 * data.channels;
            let Some(d) = buffer.datas_mut().first_mut() else { return };
            let gain = data.gain.get();
            let mut written = 0;
            if let Some(bytes) = d.data() {
                let frames = (bytes.len() / stride).min((data.samples.len() - data.pos) / data.channels);
                let slice = &data.samples[data.pos..data.pos + frames * data.channels];
                for (i, s) in slice.iter().enumerate() {
                    bytes[i * 4..i * 4 + 4].copy_from_slice(&(s * gain).to_le_bytes());
                }
                data.pos += frames * data.channels;
                written = frames * stride;
            }
            let chunk = d.chunk_mut();
            *chunk.offset_mut() = 0;
            *chunk.stride_mut() = stride as i32;
            *chunk.size_mut() = written as u32;
            if data.pos + data.channels > data.samples.len() {
                data.done.set(true);
            }
        })
        .register()
        .map_err(|e| format!("Wiedergabe konnte nicht eingerichtet werden ({e})"))?;
    let bytes = format_pod(rate, channels)?;
    let pod = Pod::from_bytes(&bytes).ok_or("Ungültige Formatbeschreibung")?;
    stream
        .connect(Direction::Output, None, StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS, &mut [pod])
        .map_err(|e| format!("Wiedergabe konnte nicht verbunden werden ({e})"))?;
    Ok(Playback {
        stream,
        _listener: listener,
        gain,
        done,
        started: Instant::now(),
        calibrated: false,
        ducking_since: None,
        done_at: None,
    })
}
