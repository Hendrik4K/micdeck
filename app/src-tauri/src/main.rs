//! MicDeck desktop app: shows whether the DJI Mic Mini is connected, switched
//! on and actually delivering sound to the PC.

mod device_images;
mod device_log;
mod dictation;
mod engine;
mod settings;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use engine::{Engine, Mode, Output};
use ksni::blocking::{Handle, TrayMethods};
use micdeck_core::Snapshot;
use settings::{AppSetting, AppSettingsView};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

type SharedSettings = Mutex<settings::Settings>;

/// The panel icon, if the desktop offers a tray (StatusNotifierWatcher).
struct TrayState(Option<Handle<tray::MicTray>>);

impl TrayState {
    fn active(&self) -> bool {
        self.0.as_ref().is_some_and(|h| !h.is_closed())
    }
}

/// Current state, for the first render; later changes arrive as events.
#[tauri::command]
fn get_snapshot(engine: tauri::State<'_, Engine>) -> Snapshot {
    engine.snapshot()
}

/// The UI reports whether a level meter is visible; samples are only
/// measured while one is.
/// Start the microphone session (listen, record, play back, repeat). The
/// button press is the user's consent; it ends on its own or via Stop.
#[tauri::command]
fn start_mic_test(engine: tauri::State<'_, Engine>) {
    engine.start_mic_test();
}

#[tauri::command]
fn stop_mic_test(engine: tauri::State<'_, Engine>) {
    engine.stop_mic_test();
}

#[tauri::command]
fn set_muted(muted: bool, app: AppHandle) {
    user_mute(&app, muted);
}

/// The user's own device pictures, if any (see `device_images`).
#[tauri::command]
fn device_images() -> device_images::DeviceImages {
    device_images::load_all()
}

/// The page to open first, from `--page=<name>` (e.g. `device`), if given.
#[tauri::command]
fn start_page() -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix("--page=").map(str::to_string))
}

/// Change a receiver or transmitter setting; the steps show up in snapshots.
#[tauri::command]
fn set_device_setting(id: String, value: String, engine: tauri::State<'_, Engine>) {
    engine.set_setting(id, value);
}

#[tauri::command]
fn get_app_settings(app: AppHandle) -> AppSettingsView {
    app_settings(&app)
}

#[tauri::command]
fn set_app_setting(setting: AppSetting, on: bool, app: AppHandle) -> AppSettingsView {
    change_app_setting(&app, setting, on, false);
    app_settings(&app)
}

fn app_settings(app: &AppHandle) -> AppSettingsView {
    app.state::<SharedSettings>().lock().map(|s| s.view()).unwrap_or_else(|_| settings::Settings::default().view())
}

/// Changes one of MicDeck's own settings and tells the window. The panel
/// menu redraws itself after its own clicks (`from_tray`); a change made in
/// the window has to redraw it.
pub(crate) fn change_app_setting(app: &AppHandle, setting: AppSetting, on: bool, from_tray: bool) {
    if setting == AppSetting::Autostart {
        if !settings::AUTOSTART_AVAILABLE {
            return;
        }
        if let Err(e) = settings::set_autostart(on) {
            log::warn!("cannot change autostart: {e}");
        }
    } else if let Ok(mut s) = app.state::<SharedSettings>().lock() {
        s.set(setting, on);
        if let Err(e) = settings::save(&s) {
            log::warn!("cannot save settings: {e}");
        }
    }
    if !from_tray && let Some(tray) = app.try_state::<TrayState>().and_then(|t| t.0.clone()) {
        tray.update(|_| {});
    }
    let _ = app.emit("micdeck:app-settings", app_settings(app));
}

/// Watches a mute MicDeck made, to warn when another program lifts it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum MuteWatch {
    Off,
    /// Asked PipeWire to mute; waiting until the input reports it.
    Requested(Instant),
    /// Muted by MicDeck; unmuting from elsewhere gets announced.
    Armed,
}

static MUTE_WATCH: Mutex<MuteWatch> = Mutex::new(MuteWatch::Off);

/// Mute or unmute the DJI input. In PipeWire this also flips the receiver's
/// own USB mute switch, so no program receives sound while muted.
pub(crate) fn apply_mute(app: &AppHandle, muted: bool) {
    *MUTE_WATCH.lock().unwrap_or_else(|p| p.into_inner()) =
        if muted { MuteWatch::Requested(Instant::now()) } else { MuteWatch::Off };
    MUTED_BY_USER.store(false, Ordering::SeqCst);
    app.state::<Engine>().set_muted(muted);
}

/// The user's own mute button (window or panel menu). A mute made this way is
/// never lifted for other programs.
pub(crate) fn user_mute(app: &AppHandle, muted: bool) {
    *RELEASE.lock().unwrap_or_else(|p| p.into_inner()) = None;
    apply_mute(app, muted);
    MUTED_BY_USER.store(muted, Ordering::SeqCst);
}

/// The current mute was made by the user, not by the charging case.
static MUTED_BY_USER: AtomicBool = AtomicBool::new(false);

/// A temporary release: transmitter 1 sits in its charging case, MicDeck had
/// muted the input for that, and another program started recording.
struct Release {
    /// A re-mute check is already scheduled.
    ending: bool,
}

static RELEASE: Mutex<Option<Release>> = Mutex::new(None);

/// How long no program may record before a release ends.
const RELEASE_GRACE: Duration = Duration::from_secs(2);

fn transmitter_docked(snapshot: &Snapshot) -> bool {
    snapshot.transmitters.iter().any(|t| t.unit == 1 && t.connected == Some(true) && t.charging == Some(true))
}

/// Called with every snapshot. While the input is muted because transmitter 1
/// sits in its case ("Im Ladecase stumm"), a program that starts recording
/// gets the microphone without asking; a notification says which one, and
/// MicDeck mutes again once no program records any more.
fn check_release(app: &AppHandle, snapshot: &Snapshot) {
    let Some(source) = &snapshot.audio.source else { return };
    let docked = transmitter_docked(snapshot);
    let mut release = RELEASE.lock().unwrap_or_else(|p| p.into_inner());
    match release.as_mut() {
        None => {
            let case_muted = docked
                && source.muted == Some(true)
                && !MUTED_BY_USER.load(Ordering::SeqCst)
                && !snapshot.simulated
                && dock_settings(app).1;
            if !case_muted || source.capture_apps.is_empty() {
                return;
            }
            *release = Some(Release { ending: false });
            drop(release);
            let apps = source.capture_apps.join(", ");
            log::info!("released the microphone for {apps}");
            apply_mute(app, false);
            notify(micdeck_core::notify::Notice {
                title: "Mikrofon kurz freigegeben".into(),
                body: format!("{apps} nimmt auf. Danach schaltet MicDeck wieder stumm."),
            });
        }
        // Taken out of the case: from now on the case logic decides.
        Some(_) if !docked => *release = None,
        Some(r) if source.capture_apps.is_empty() && !r.ending => {
            r.ending = true;
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(RELEASE_GRACE);
                let snapshot = app.state::<Engine>().snapshot();
                let mut release = RELEASE.lock().unwrap_or_else(|p| p.into_inner());
                let Some(r) = release.as_mut() else { return };
                let still_recording = snapshot.audio.source.as_ref().is_some_and(|s| !s.capture_apps.is_empty());
                if still_recording {
                    r.ending = false;
                    return;
                }
                *release = None;
                drop(release);
                if transmitter_docked(&snapshot) {
                    log::info!("recording ended, muting again");
                    apply_mute(&app, true);
                }
            });
        }
        Some(_) => {}
    }
}

/// Called with every snapshot: arms the watch once the mute took effect and
/// reports when someone else lifts it.
fn check_mute_watch(muted: Option<bool>) {
    let mut watch = MUTE_WATCH.lock().unwrap_or_else(|p| p.into_inner());
    *watch = match (*watch, muted) {
        (MuteWatch::Requested(_), Some(true)) => MuteWatch::Armed,
        (MuteWatch::Requested(at), _) if at.elapsed() > Duration::from_secs(3) => MuteWatch::Off,
        (MuteWatch::Armed, Some(false)) => {
            log::warn!("mute lifted by another program");
            notify(micdeck_core::notify::Notice {
                title: "Stummschaltung aufgehoben".into(),
                body: "Ein anderes Programm hat das DJI-Mikrofon wieder eingeschaltet.".into(),
            });
            MuteWatch::Off
        }
        (other, _) => other,
    };
}

#[tauri::command]
fn set_meter_wanted(wanted: bool, engine: tauri::State<'_, Engine>) {
    engine.set_meter_wanted(wanted);
}

pub(crate) fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    app.state::<Engine>().set_meter_wanted(true);
}

/// Hide to the panel. The level meter stops: nothing is measured while
/// nobody looks.
pub(crate) fn hide_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    app.state::<Engine>().set_meter_wanted(false);
}

pub(crate) fn toggle_window(app: &AppHandle) {
    let visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
    if visible { hide_window(app) } else { show_window(app) }
}

fn notify(notice: micdeck_core::notify::Notice) {
    // D-Bus round trip; keep it off the engine thread.
    std::thread::spawn(move || {
        let shown = notify_rust::Notification::new()
            .appname("MicDeck")
            .summary(&notice.title)
            .body(&notice.body)
            .icon("micdeck")
            .show();
        if let Err(e) = shown {
            log::warn!("notification not shown: {e}");
        }
    });
}

/// MicDeck started a dictation that hasn't been stopped yet.
static DICTATING: AtomicBool = AtomicBool::new(false);
/// The panel-icon animation thread is running.
static ANIMATING: AtomicBool = AtomicBool::new(false);

/// Show a dictation phase in the panel icon; animated phases get a 10 fps
/// redraw thread that ends by itself when the icon is back to normal.
fn set_dictation_phase(app: &AppHandle, phase: tray::DictationPhase) {
    use tray::DictationPhase;
    let Some(tray) = app.try_state::<TrayState>().and_then(|t| t.0.clone()) else { return };
    tray.update(|t| t.dictation = phase);
    if !phase.animated() || ANIMATING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(100));
        let animated = tray.update(|t| {
            if let DictationPhase::Sent { since } = t.dictation
                && since.elapsed() >= tray::SENT_SHOWN
            {
                t.dictation = DictationPhase::Idle;
            }
            t.dictation.animated()
        });
        if animated != Some(true) {
            ANIMATING.store(false, Ordering::SeqCst);
            // A new phase may have started just now; keep going for it.
            if tray.update(|t| t.dictation.animated()) == Some(true) && !ANIMATING.swap(true, Ordering::SeqCst) {
                continue;
            }
            break;
        }
    });
}

fn dock_settings(app: &AppHandle) -> (bool, bool) {
    app.state::<SharedSettings>().lock().map(|s| (s.dock_dictation, s.dock_mute)).unwrap_or((false, false))
}

/// Transmitter 1 taken out of / put back into its charging case. Taking it
/// out unmutes before dictation starts; putting it back mutes only after a
/// running dictation has been sent, so its last words are not lost.
fn on_dock(app: &AppHandle, event: micdeck_core::dock::DockEvent) {
    use micdeck_core::dock::DockEvent;
    let (dictation, dock_mute) = dock_settings(app);
    match event {
        DockEvent::TakenOut { unit: 1 } => {
            if dock_mute {
                apply_mute(app, false);
            }
            if dictation {
                dock_dictation(app.clone(), event);
            }
        }
        DockEvent::PutBack { unit: 1 } => {
            if dictation && DICTATING.load(Ordering::SeqCst) {
                // Mutes afterwards if wanted (see `dock_dictation`).
                dock_dictation(app.clone(), event);
            } else if dock_mute {
                apply_mute(app, true);
            }
        }
        _ => {}
    }
}

/// Charging-case trigger: out = start dictation in Claude, back in = stop and
/// send. Only transmitter 1, so a second one can't toggle the same dictation.
fn dock_dictation(app: AppHandle, event: micdeck_core::dock::DockEvent) {
    use micdeck_core::dock::DockEvent;
    use tray::DictationPhase;
    let action = match event {
        DockEvent::TakenOut { unit: 1 } => dictation::Action::Start,
        // Only stop what MicDeck started: if starting failed, pressing the
        // toggle now would start a dictation and send an empty message.
        DockEvent::PutBack { unit: 1 } if DICTATING.load(Ordering::SeqCst) => dictation::Action::StopAndSend,
        _ => return,
    };
    std::thread::spawn(move || {
        if let dictation::Action::StopAndSend = action {
            set_dictation_phase(&app, DictationPhase::Sending { since: Instant::now() });
        }
        let result = dictation::run(action);
        if matches!(action, dictation::Action::StopAndSend) && dock_settings(&app).1 {
            apply_mute(&app, true);
        }
        match result {
            Ok(()) => {
                log::info!("dictation {action:?} triggered");
                let (dictating, phase) = match action {
                    dictation::Action::Start => (true, DictationPhase::Recording { since: Instant::now() }),
                    dictation::Action::StopAndSend => (false, DictationPhase::Sent { since: Instant::now() }),
                };
                DICTATING.store(dictating, Ordering::SeqCst);
                set_dictation_phase(&app, phase);
            }
            Err(message) => {
                log::warn!("dictation {action:?} failed: {message}");
                DICTATING.store(false, Ordering::SeqCst);
                set_dictation_phase(&app, DictationPhase::Idle);
                notify(micdeck_core::notify::Notice { title: "Diktat nicht ausgelöst".into(), body: message });
            }
        }
    });
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mode = match Mode::from_args(std::env::args().skip(1)) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    // `--hidden` (used by autostart): start in the panel only.
    let start_hidden = std::env::args().any(|a| a == "--hidden");
    let live = matches!(mode, Mode::Live);

    let mut builder = tauri::Builder::default();
    // Only one live MicDeck: a second start (menu, desktop icon) brings the
    // running window forward instead of competing for the receiver. Demo
    // instances touch no hardware and may run alongside.
    if live {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            log::info!("second start {args:?}");
            if !args.iter().any(|a| a == "--hidden") {
                // Not from inside the plugin's callback, where showing the
                // window has no effect; the panel icon does it the same way.
                let app = app.clone();
                std::thread::spawn(move || show_window(&app));
            }
        }));
    }
    builder
        .setup(move |app| {
            app.manage::<SharedSettings>(Mutex::new(settings::load()));

            let handle = app.handle().clone();
            // Only touch the panel icon when what it shows changes.
            let last_tray = Mutex::new(None);
            let engine = Engine::start(mode, move |output| {
                let result = match output {
                    Output::Snapshot(snapshot) => {
                        let v = &snapshot.verdict;
                        let muted = snapshot.audio.source.as_ref().and_then(|s| s.muted);
                        check_mute_watch(muted);
                        check_release(&handle, &snapshot);
                        let tray_text = (v.level, v.title.clone(), v.detail.clone(), muted);
                        let mut last = last_tray.lock().unwrap_or_else(|p| p.into_inner());
                        if last.as_ref() != Some(&tray_text) {
                            if let Some(tray) = handle.try_state::<TrayState>().and_then(|t| t.0.clone()) {
                                let (level, title, detail, muted) = tray_text.clone();
                                tray.update(|t| {
                                    t.level = level;
                                    t.title = title;
                                    t.detail = detail;
                                    t.muted = muted;
                                });
                            }
                            *last = Some(tray_text);
                        }
                        drop(last);
                        handle.emit("micdeck:snapshot", snapshot)
                    }
                    Output::Level(frame) => handle.emit("micdeck:level", frame),
                    Output::Dock(event) => {
                        on_dock(&handle, event);
                        Ok(())
                    }
                    Output::Notice(notice) => {
                        let enabled = handle.state::<SharedSettings>().lock().map(|s| s.notifications).unwrap_or(true);
                        if enabled {
                            notify(notice);
                        }
                        Ok(())
                    }
                };
                if let Err(e) = result {
                    log::debug!("event not delivered: {e}");
                }
            });
            let first = engine.snapshot().verdict;
            app.manage(engine);

            let tray = tray::MicTray {
                app: app.handle().clone(),
                level: first.level,
                title: first.title,
                detail: first.detail,
                dictation: tray::DictationPhase::Idle,
                muted: None,
            }
            .spawn();
            let tray = match tray {
                Ok(handle) => Some(handle),
                Err(e) => {
                    log::warn!("no panel icon, window only: {e}");
                    None
                }
            };
            let has_tray = tray.is_some();
            app.manage(TrayState(tray));

            // Without a panel icon the window is the only way back in.
            if !start_hidden || !has_tray {
                show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if app.state::<TrayState>().active() {
                    api.prevent_close();
                    hide_window(app);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            set_meter_wanted,
            set_muted,
            start_mic_test,
            stop_mic_test,
            set_device_setting,
            get_app_settings,
            set_app_setting,
            start_page,
            device_images
        ])
        .run(tauri::generate_context!())
        .expect("MicDeck could not start");
}
