//! Wires the USB and PipeWire adapters (or the demo driver) to the core state
//! and pushes changes to the UI.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use micdeck_core::mock::{demo_level, Scenario};
use micdeck_core::dock::{DockEvent, DockWatcher};
use micdeck_core::notify::{Notice, Notifier};
use micdeck_core::{
    AudioEvent, Input, LevelFrame, MeterState, MicTestState, Snapshot, State, UsbEvent, WritePhase, WriteReport,
};
use usb_link::WriteRequest;

use crate::device_log;

/// Snapshots go to the UI at most this often (only when something changed).
const SNAPSHOT_INTERVAL: Duration = Duration::from_millis(80);
/// How long each demo scenario is shown when cycling through all of them.
const DEMO_SCENARIO_DURATION: Duration = Duration::from_secs(8);

pub enum Mode {
    Live,
    /// Demo data; `None` cycles through every scenario.
    Demo(Option<Scenario>),
}

impl Mode {
    /// `--demo` cycles through all scenarios, `--demo=<name>` shows one.
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut mode = Mode::Live;
        for arg in args {
            if arg == "--demo" {
                mode = Mode::Demo(None);
            } else if let Some(name) = arg.strip_prefix("--demo=") {
                let scenario = Scenario::from_slug(name).ok_or_else(|| {
                    let names: Vec<&str> = Scenario::ALL.iter().map(|s| s.slug()).collect();
                    format!("Unbekanntes Demo-Szenario „{name}“. Möglich: {}", names.join(", "))
                })?;
                mode = Mode::Demo(Some(scenario));
            }
        }
        Ok(mode)
    }
}

pub enum Output {
    Snapshot(Snapshot),
    Level(LevelFrame),
    /// Something worth a desktop notification (never for demo data).
    Notice(Notice),
    /// A transmitter was taken out of or put back into its charging case.
    Dock(DockEvent),
}

enum Msg {
    Input(Input),
    /// Mute or unmute the DJI input (user request).
    SetMute(bool),
    /// Start the microphone session (user request).
    MicTest,
    /// End it at once.
    MicTestStop,
    /// Change a receiver or transmitter setting (user request).
    SetSetting { id: String, value: String },
    /// Start over with an empty state (demo scenario change).
    Reset,
}

trait MeterControl: Send {
    fn set_meter(&self, target: Option<String>);
    fn set_mute(&self, node_name: String, mute: bool);
    /// Start the microphone test; `false` if this mode can't run one.
    fn start_mic_test(&self, node_name: String) -> bool;
    fn stop_mic_test(&self);
    fn write_setting(&self, key: String, request: WriteRequest);
}

struct LiveAdapters {
    usb: usb_link::UsbWatcher,
    audio: audio_pw::AudioMonitor,
}

impl MeterControl for LiveAdapters {
    fn set_meter(&self, target: Option<String>) {
        self.audio.set_meter(target);
    }

    fn set_mute(&self, node_name: String, mute: bool) {
        self.audio.set_mute(node_name, mute);
    }

    fn start_mic_test(&self, node_name: String) -> bool {
        self.audio.start_mic_test(node_name);
        true
    }

    fn stop_mic_test(&self) {
        self.audio.stop_mic_test();
    }

    fn write_setting(&self, key: String, request: WriteRequest) {
        self.usb.write_setting(key, request);
    }
}

pub struct Engine {
    tx: Sender<Msg>,
    latest: Arc<Mutex<Snapshot>>,
}

impl Engine {
    pub fn start(mode: Mode, emit: impl Fn(Output) + Send + 'static) -> Self {
        let simulated = matches!(mode, Mode::Demo(_));
        let (tx, rx) = mpsc::channel();
        let latest = Arc::new(Mutex::new(State::new(simulated).snapshot(0)));

        let control: Box<dyn MeterControl> = match mode {
            Mode::Live => {
                let usb_tx = tx.clone();
                let usb = usb_link::spawn(Arc::new(move |e| {
                    let _ = usb_tx.send(Msg::Input(Input::Usb(e)));
                }));
                let audio_tx = tx.clone();
                let audio = audio_pw::spawn(Arc::new(move |e| {
                    let _ = audio_tx.send(Msg::Input(Input::Audio(e)));
                }));
                Box::new(LiveAdapters { usb, audio })
            }
            Mode::Demo(scenario) => Box::new(DemoDriver::spawn(scenario, tx.clone())),
        };

        let shared = latest.clone();
        thread::Builder::new()
            .name("engine".into())
            .spawn(move || run(rx, &emit, &shared, control.as_ref(), simulated))
            .expect("failed to start engine thread");
        Self { tx, latest }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.latest.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.tx.send(Msg::SetMute(muted));
    }

    pub fn start_mic_test(&self) {
        let _ = self.tx.send(Msg::MicTest);
    }

    pub fn stop_mic_test(&self) {
        let _ = self.tx.send(Msg::MicTestStop);
    }

    pub fn set_setting(&self, id: String, value: String) {
        let _ = self.tx.send(Msg::SetSetting { id, value });
    }

    pub fn set_meter_wanted(&self, wanted: bool) {
        let _ = self.tx.send(Msg::Input(Input::MeterWanted(wanted)));
    }
}

fn run(
    rx: Receiver<Msg>,
    emit: &dyn Fn(Output),
    latest: &Mutex<Snapshot>,
    control: &dyn MeterControl,
    simulated: bool,
) {
    let clock = Instant::now();
    let now = || clock.elapsed().as_millis() as u64;
    let mut state = State::new(simulated);
    let mut meter_wanted = false;
    let mut meter_target: Option<String> = None;
    let mut last_json = String::new();
    let mut last_emit: Option<Instant> = None;
    let mut notifier = Notifier::default();
    let mut dock = DockWatcher::default();
    let mut next_request: u64 = 0;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Msg::Input(input)) => {
                match &input {
                    Input::MeterWanted(w) => meter_wanted = *w,
                    // Levels go straight to the UI, but only for the input being measured.
                    Input::Audio(AudioEvent::Level(frame))
                        if meter_target.as_deref() == Some(frame.node_name.as_str()) =>
                    {
                        emit(Output::Level(frame.clone()));
                    }
                    _ => {}
                }
                let finished_write = match &input {
                    Input::Usb(UsbEvent::Write { report, .. }) if report.phase.is_final() => Some(report.request),
                    _ => None,
                };
                state.apply(input, now());
                if let Some(request) = finished_write {
                    log_write(&state, request);
                }
            }
            Ok(Msg::SetSetting { id, value }) => {
                next_request += 1;
                let request = WriteRequest { request: next_request, setting: id, value };
                match state.setting_target(&request.setting, &request.value, now()) {
                    Ok(key) => control.write_setting(key, request),
                    Err(reason) => {
                        log::info!("not sending {}={}: {reason}", request.setting, request.value);
                        let key = state.snapshot(now()).receiver.map(|r| r.key).unwrap_or_default();
                        let report = WriteReport {
                            request: request.request,
                            setting: request.setting,
                            value: request.value,
                            phase: WritePhase::Rejected { reason },
                            unix_ms: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_or(0, |d| d.as_millis() as u64),
                            seq: None,
                            packet: None,
                        };
                        state.apply(Input::Usb(UsbEvent::Write { key, report }), now());
                        log_write(&state, request.request);
                    }
                }
            }
            Ok(Msg::SetMute(mute)) => {
                if let Some(source) = state.snapshot(now()).audio.source {
                    control.set_mute(source.node_name, mute);
                }
            }
            Ok(Msg::MicTest) => {
                let failed = match state.snapshot(now()).audio.source {
                    Some(source) if control.start_mic_test(source.node_name.clone()) => None,
                    Some(_) => Some("Im Demo-Modus gibt es kein echtes Mikrofon zum Testen."),
                    None => Some("Kein DJI-Eingang zum Testen vorhanden."),
                };
                if let Some(message) = failed {
                    let test = MicTestState::Failed { message: message.into() };
                    state.apply(Input::Audio(AudioEvent::MicTest(test)), now());
                }
            }
            Ok(Msg::MicTestStop) => control.stop_mic_test(),
            Ok(Msg::Reset) => {
                state = State::new(simulated);
                state.apply(Input::MeterWanted(meter_wanted), now());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        let target = state.meter_target();
        if target != meter_target {
            control.set_meter(target.clone());
            meter_target = target;
        }

        if last_emit.is_none_or(|t| t.elapsed() >= SNAPSHOT_INTERVAL) {
            last_emit = Some(Instant::now());
            let snapshot = state.snapshot(now());
            for notice in notifier.update(&snapshot, now()) {
                emit(Output::Notice(notice));
            }
            for event in dock.update(&snapshot, now()) {
                emit(Output::Dock(event));
            }
            let json = serde_json::to_string(&snapshot).unwrap_or_default();
            if json != last_json {
                last_json = json;
                *latest.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
                emit(Output::Snapshot(snapshot));
            }
        }
    }
}

/// Appends a finished setting change to the log file.
fn log_write(state: &State, request: u64) {
    if state.is_simulated() {
        return;
    }
    if let Some((key, from, report)) = state.write_record(request) {
        device_log::append(&device_log::line(&key, from.as_deref(), &report));
    }
}

/// Feeds demo scenarios as if they came from real adapters. Everything it
/// produces is marked as simulated.
struct DemoDriver {
    meter_target: Arc<Mutex<Option<String>>>,
}

impl DemoDriver {
    fn spawn(fixed: Option<Scenario>, tx: Sender<Msg>) -> Self {
        let meter_target = Arc::new(Mutex::new(None::<String>));
        let wanted = meter_target.clone();
        thread::Builder::new()
            .name("demo".into())
            .spawn(move || {
                let send = |msg| tx.send(msg).is_ok();
                let show = |scenario: Scenario| {
                    send(Msg::Reset)
                        && send(Msg::Input(Input::DemoLabel(scenario.title().into())))
                        && scenario.inputs().into_iter().all(|i| send(Msg::Input(i)))
                };
                let mut index = 0;
                let mut scenario = fixed.unwrap_or(Scenario::ALL[0]);
                if !show(scenario) {
                    return;
                }
                let mut shown_since = Instant::now();
                let mut last_periodic = Instant::now();
                let mut metering: Option<(String, Instant)> = None;
                loop {
                    thread::sleep(Duration::from_millis(33));
                    if fixed.is_none() && shown_since.elapsed() >= DEMO_SCENARIO_DURATION {
                        index = (index + 1) % Scenario::ALL.len();
                        scenario = Scenario::ALL[index];
                        if !show(scenario) {
                            return;
                        }
                        shown_since = Instant::now();
                        metering = None;
                    }
                    if last_periodic.elapsed() >= Duration::from_millis(500) {
                        last_periodic = Instant::now();
                        if !scenario.periodic_inputs().into_iter().all(|i| send(Msg::Input(i))) {
                            return;
                        }
                    }
                    let target = wanted.lock().unwrap_or_else(|p| p.into_inner()).clone();
                    let ok = match (target, scenario.signal()) {
                        (Some(node), Some(signal)) => {
                            let started = match &metering {
                                Some((n, at)) if *n == node => *at,
                                _ => {
                                    let at = Instant::now();
                                    metering = Some((node.clone(), at));
                                    if !send(Msg::Input(Input::Audio(AudioEvent::Meter(
                                        MeterState::Running { node_name: node.clone(), channels: 2, rate: 48_000 },
                                    )))) {
                                        return;
                                    }
                                    at
                                }
                            };
                            let t = started.elapsed().as_millis() as u64;
                            send(Msg::Input(Input::Audio(AudioEvent::Level(demo_level(signal, t, &node, 2)))))
                        }
                        _ if metering.take().is_some() => {
                            send(Msg::Input(Input::Audio(AudioEvent::Meter(MeterState::Off))))
                        }
                        _ => true,
                    };
                    if !ok {
                        return;
                    }
                }
            })
            .expect("failed to start demo thread");
        Self { meter_target }
    }
}

impl MeterControl for DemoDriver {
    fn set_meter(&self, target: Option<String>) {
        *self.meter_target.lock().unwrap_or_else(|p| p.into_inner()) = target;
    }

    fn set_mute(&self, _node_name: String, _mute: bool) {
        // Demo data has no real input to mute.
        log::info!("mute ignored in demo mode");
    }

    fn start_mic_test(&self, _node_name: String) -> bool {
        false
    }

    fn stop_mic_test(&self) {}

    fn write_setting(&self, _key: String, request: WriteRequest) {
        // State::setting_target already refuses everything in demo mode.
        log::info!("demo mode: not writing {}", request.setting);
    }
}
