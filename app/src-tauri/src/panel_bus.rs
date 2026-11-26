//! Session D-Bus service for the Cinnamon panel applet
//! (`packaging/cinnamon/micdeck@hendrik4k`). The applet shows MicDeck's state
//! in a hover menu; StatusNotifierItem can't, it gets no hover events.
//!
//! - Method `State() -> s`: the current [`PanelState`] as JSON.
//! - Signal `StateChanged(s)`: the same JSON whenever it changes.
//! - Signal `Level(d)`: input peak in dBFS, about ten times a second, only
//!   while the applet is there.
//! - Methods `SetMuted(b)`, `ShowWindow()`, `RestartReceiver()`,
//!   `SetAppSetting(s, b)` (`dockDictation`, `dockMute`, `autostart`).
//!
//! While the applet owns [`APPLET_NAME`], the StatusNotifierItem goes passive
//! so the panel doesn't show MicDeck twice.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use zbus::blocking::{connection, Connection};

use crate::settings::AppSetting;

pub const BUS_NAME: &str = "io.github.hendrik4k.MicDeck";
pub const PATH: &str = "/io/github/hendrik4k/MicDeck";
pub const IFACE: &str = "io.github.hendrik4k.MicDeck";
/// Owned by the applet while it runs.
pub const APPLET_NAME: &str = "io.github.hendrik4k.MicDeck.Applet";

/// What the applet shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelState {
    /// `pending`, `ok`, `warn` or `error`.
    pub level: String,
    pub title: String,
    pub detail: String,
    /// `None`: no input to mute.
    pub muted: Option<bool>,
    /// A DJI receiver is on USB.
    pub receiver: bool,
    pub receiver_off: bool,
    pub restarting: bool,
    /// A program records from the DJI input (MicDeck's own meter not
    /// counted); the applet's icon pulses.
    pub capturing: bool,
    /// The desktop's default input.
    pub input: Option<String>,
    /// `idle`, `recording`, `sending` or `sent`.
    pub dictation: String,
    pub dock_dictation: bool,
    pub dock_mute: bool,
    pub autostart: bool,
    pub simulated: bool,
}

static CONN: OnceLock<Connection> = OnceLock::new();
static STATE: Mutex<Option<PanelState>> = Mutex::new(None);
static APPLET: AtomicBool = AtomicBool::new(false);

/// The applet is running (and shows MicDeck in the panel).
pub fn applet_present() -> bool {
    APPLET.load(Ordering::SeqCst)
}

struct Panel {
    app: AppHandle,
}

#[zbus::interface(name = "io.github.hendrik4k.MicDeck")]
impl Panel {
    fn state(&self) -> String {
        current_json()
    }

    fn set_muted(&self, muted: bool) {
        let app = self.app.clone();
        std::thread::spawn(move || crate::user_mute(&app, muted));
    }

    fn show_window(&self) {
        let app = self.app.clone();
        std::thread::spawn(move || crate::show_window(&app));
    }

    fn restart_receiver(&self) {
        if let Some(engine) = self.app.try_state::<crate::engine::Engine>() {
            engine.restart_receiver();
        }
    }

    fn set_app_setting(&self, name: &str, on: bool) {
        let setting = match name {
            "dockDictation" => AppSetting::DockDictation,
            "dockMute" => AppSetting::DockMute,
            "autostart" => AppSetting::Autostart,
            _ => return,
        };
        let app = self.app.clone();
        std::thread::spawn(move || crate::change_app_setting(&app, setting, on, false));
    }
}

fn current_json() -> String {
    let state = STATE.lock().unwrap_or_else(|p| p.into_inner()).clone().unwrap_or_default();
    serde_json::to_string(&state).unwrap_or_else(|_| "{}".into())
}

/// Claims the bus name and starts watching for the applet. `on_applet` is
/// called whenever the applet comes or goes.
pub fn start(app: AppHandle, on_applet: impl Fn(bool) + Send + 'static) {
    let conn = match connection::Builder::session()
        .and_then(|b| b.name(BUS_NAME))
        .and_then(|b| b.serve_at(PATH, Panel { app }))
        .and_then(|b| b.build())
    {
        Ok(conn) => conn,
        Err(e) => {
            log::warn!("no D-Bus service for the panel applet: {e}");
            return;
        }
    };
    let watch = conn.clone();
    let _ = CONN.set(conn);
    std::thread::spawn(move || {
        let Ok(dbus) = zbus::blocking::fdo::DBusProxy::new(&watch) else { return };
        // Subscribe first, then ask: no change slips through in between.
        let changes = dbus.receive_name_owner_changed_with_args(&[(0, APPLET_NAME)]);
        let present = APPLET_NAME.try_into().ok().and_then(|n| dbus.name_has_owner(n).ok()).unwrap_or(false);
        set_applet(present, &on_applet);
        let Ok(changes) = changes else { return };
        for change in changes {
            if let Ok(args) = change.args() {
                set_applet(args.new_owner().is_some(), &on_applet);
            }
        }
    });
}

fn set_applet(present: bool, on_applet: &impl Fn(bool)) {
    if APPLET.swap(present, Ordering::SeqCst) != present {
        log::info!("panel applet {}", if present { "is there" } else { "is gone" });
        on_applet(present);
    }
}

/// Changes the state and tells the applet if anything differs.
pub fn update(change: impl FnOnce(&mut PanelState)) {
    let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
    let mut next = guard.clone().unwrap_or_default();
    change(&mut next);
    if guard.as_ref() == Some(&next) {
        return;
    }
    *guard = Some(next);
    drop(guard);
    if let Some(conn) = CONN.get() {
        let _ = conn.emit_signal(None::<&str>, PATH, IFACE, "StateChanged", &(current_json(),));
    }
}

/// Peak since the last `Level` signal.
static METER: Mutex<(Option<Instant>, f32)> = Mutex::new((None, f32::NEG_INFINITY));

/// Feeds one level frame; sends the peak at most every 100 ms.
pub fn level(peak_dbfs: f32) {
    if !applet_present() {
        return;
    }
    let mut meter = METER.lock().unwrap_or_else(|p| p.into_inner());
    meter.1 = meter.1.max(peak_dbfs);
    let since = *meter.0.get_or_insert_with(Instant::now);
    if since.elapsed() < Duration::from_millis(100) {
        return;
    }
    let peak = meter.1;
    *meter = (Some(Instant::now()), f32::NEG_INFINITY);
    drop(meter);
    if let Some(conn) = CONN.get() {
        let value = if peak.is_finite() { peak as f64 } else { -120.0 };
        let _ = conn.emit_signal(None::<&str>, PATH, IFACE, "Level", &(value,));
    }
}
