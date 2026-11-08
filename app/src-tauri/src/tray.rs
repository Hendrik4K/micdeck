//! Panel icon (StatusNotifierItem, via ksni): a microphone in the status
//! colour, the verdict as tooltip, left click shows or hides the window.
//! While dictation runs it shows an animated sonar (a lime dot with a ring
//! travelling outwards), then a progress bar while the message is sent, then
//! a tick. Tauri's own tray gets no click events on Cinnamon, hence ksni.

use ksni::menu::{CheckmarkItem, StandardItem};
use std::time::{Duration, Instant};

use ksni::{Icon, MenuItem, ToolTip, Tray};
use micdeck_core::Level;
use tauri::{AppHandle, Manager};

use crate::settings::{self, AppSetting};
use crate::SharedSettings;

pub struct MicTray {
    pub app: AppHandle,
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub dictation: DictationPhase,
    /// The DJI input is muted (`None`: no input to mute).
    pub muted: Option<bool>,
}

/// What the icon shows for the charging-case dictation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DictationPhase {
    Idle,
    Recording { since: Instant },
    Sending { since: Instant },
    Sent { since: Instant },
}

/// How long the tick stays before the microphone returns.
pub const SENT_SHOWN: Duration = Duration::from_millis(1_500);

impl DictationPhase {
    /// Needs a timer: the sonar moves, and the tick returns to the
    /// microphone after `SENT_SHOWN`.
    pub fn animated(self) -> bool {
        matches!(self, DictationPhase::Recording { .. } | DictationPhase::Sent { .. })
    }
}

/// Panel sizes the icons come in.
const SIZES: [i32; 5] = [16, 22, 24, 32, 48];

/// One icon in all `SIZES`, as raw RGBA (made from `icons/tray/source/`).
macro_rules! tray_bitmap {
    ($name:literal) => {
        [
            include_bytes!(concat!("../icons/tray/", $name, "-16.rgba")).as_slice(),
            include_bytes!(concat!("../icons/tray/", $name, "-22.rgba")).as_slice(),
            include_bytes!(concat!("../icons/tray/", $name, "-24.rgba")).as_slice(),
            include_bytes!(concat!("../icons/tray/", $name, "-32.rgba")).as_slice(),
            include_bytes!(concat!("../icons/tray/", $name, "-48.rgba")).as_slice(),
        ]
    };
}

/// The panel icon set (see THIRD_PARTY.md for where it comes from).
static BITMAPS: [(&str, [&[u8]; 5]); 8] = [
    ("ok", tray_bitmap!("ok")),
    ("warn", tray_bitmap!("warn")),
    ("error", tray_bitmap!("error")),
    ("pending", tray_bitmap!("pending")),
    ("muted", tray_bitmap!("muted")),
    ("recording", tray_bitmap!("recording")),
    ("sending", tray_bitmap!("sending")),
    ("sent", tray_bitmap!("sent")),
];

/// All sizes of one icon, converted to the ARGB (big endian) the panel wants.
fn bitmap(name: &str) -> Vec<Icon> {
    let Some((_, sizes)) = BITMAPS.iter().find(|(n, _)| *n == name) else { return Vec::new() };
    SIZES
        .iter()
        .zip(sizes)
        .map(|(&size, rgba)| Icon {
            width: size,
            height: size,
            data: rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect(),
        })
        .collect()
}

/// Lime of the dictation phases.
const LIME: [u8; 3] = [0xd1, 0xfe, 0x17];

/// One frame of the dictation sonar in every panel size, as ARGB: a lime dot
/// and one faint ring that travels outwards every 2.6 s (style "C, very
/// subtle", chosen on 06.11.2026).
fn sonar(elapsed: Duration) -> Vec<Icon> {
    const DOT: f32 = 0.13;
    const PERIOD_S: f32 = 2.6;
    const RING_ALPHA: f32 = 0.35;
    let phase = (elapsed.as_secs_f32() / PERIOD_S).fract();
    let radius = DOT + 0.06 + phase * (0.45 - DOT - 0.06);
    let fade = (phase / 0.15).min(1.0) * (1.0 - phase);
    SIZES
        .iter()
        .map(|&size| {
            let s = size as f32;
            let stroke = (1.0 / s).max(0.04);
            // Share of a pixel covered by a shape `d` (in icon units) away.
            let cover = |d: f32| (0.5 - d * s).clamp(0.0, 1.0);
            let data = (0..size * size)
                .flat_map(|i| {
                    let (x, y) = (((i % size) as f32 + 0.5) / s - 0.5, ((i / size) as f32 + 0.5) / s - 0.5);
                    let d = x.hypot(y);
                    let alpha = cover(d - DOT).max(cover((d - radius).abs() - stroke / 2.0) * fade * RING_ALPHA);
                    [(alpha * 255.0).round() as u8, LIME[0], LIME[1], LIME[2]]
                })
                .collect();
            Icon { width: size, height: size, data }
        })
        .collect()
}

impl MicTray {
    fn icon_name(&self) -> &'static str {
        match self.dictation {
            DictationPhase::Recording { .. } => "recording",
            DictationPhase::Sending { .. } => "sending",
            DictationPhase::Sent { .. } => "sent",
            DictationPhase::Idle if self.muted == Some(true) => "muted",
            DictationPhase::Idle => match self.level {
                Level::Ok => "ok",
                Level::Warn => "warn",
                Level::Error => "error",
                Level::Pending => "pending",
            },
        }
    }
}

impl Tray for MicTray {
    fn id(&self) -> String {
        "micdeck".into()
    }

    fn title(&self) -> String {
        "MicDeck".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        match self.dictation {
            DictationPhase::Recording { since } => sonar(since.elapsed()),
            _ => bitmap(self.icon_name()),
        }
    }

    fn tool_tip(&self) -> ToolTip {
        let (title, description) = match self.dictation {
            DictationPhase::Idle => (format!("MicDeck – {}", self.title), self.detail.clone()),
            DictationPhase::Recording { .. } => {
                ("MicDeck – Diktat läuft".into(), "Sender zurück ins Ladecase legen, um abzuschicken.".into())
            }
            DictationPhase::Sending { .. } => ("MicDeck – Wird gesendet …".into(), String::new()),
            DictationPhase::Sent { .. } => ("MicDeck – Gesendet".into(), String::new()),
        };
        ToolTip { title, description, ..Default::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        crate::toggle_window(&self.app);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let (notifications, dock_dictation, dock_mute) = self
            .app
            .state::<SharedSettings>()
            .lock()
            .map(|s| (s.notifications, s.dock_dictation, s.dock_mute))
            .unwrap_or((true, false, false));
        let autostart = settings::autostart_enabled();
        let autostart_label = if settings::AUTOSTART_AVAILABLE {
            "Beim Anmelden starten"
        } else {
            "Beim Anmelden starten (nur installierte App)"
        };
        let mute_label = match self.muted {
            Some(true) => "Stummschaltung aufheben",
            _ => "Stummschalten",
        };
        vec![
            StandardItem {
                label: "MicDeck öffnen".into(),
                activate: Box::new(|t: &mut Self| crate::show_window(&t.app)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: mute_label.into(),
                enabled: self.muted.is_some(),
                activate: Box::new(|t: &mut Self| crate::user_mute(&t.app, t.muted != Some(true))),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Benachrichtigungen".into(),
                checked: notifications,
                activate: Box::new(move |t: &mut Self| {
                    crate::change_app_setting(&t.app, AppSetting::Notifications, !notifications, true)
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Diktat über Ladecase (Claude)".into(),
                checked: dock_dictation,
                activate: Box::new(move |t: &mut Self| {
                    crate::change_app_setting(&t.app, AppSetting::DockDictation, !dock_dictation, true)
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Im Ladecase stumm".into(),
                checked: dock_mute,
                activate: Box::new(move |t: &mut Self| {
                    crate::change_app_setting(&t.app, AppSetting::DockMute, !dock_mute, true)
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: autostart_label.into(),
                enabled: settings::AUTOSTART_AVAILABLE,
                checked: autostart,
                activate: Box::new(move |t: &mut Self| {
                    crate::change_app_setting(&t.app, AppSetting::Autostart, !autostart, true)
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Beenden".into(),
                activate: Box::new(|t: &mut Self| t.app.exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_every_size() {
        for (name, _) in BITMAPS {
            let icons = bitmap(name);
            assert_eq!(icons.len(), SIZES.len(), "{name}");
            for icon in icons {
                assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize, "{name}");
                let alphas: Vec<u8> = icon.data.iter().step_by(4).copied().collect();
                assert!(alphas.contains(&0) && alphas.iter().any(|&a| a > 200), "{name} needs a shape on a clear background");
            }
        }
    }

    #[test]
    fn sonar_has_a_solid_dot_and_a_moving_ring() {
        let early = sonar(Duration::from_millis(500));
        let late = sonar(Duration::from_millis(1_800));
        assert_eq!(early.len(), SIZES.len());
        for (a, b) in early.iter().zip(&late) {
            let size = a.width as usize;
            let alpha = |icon: &Icon, x: usize, y: usize| icon.data[(y * size + x) * 4];
            assert_eq!(a.data.len(), size * size * 4);
            assert_eq!(alpha(a, size / 2, size / 2), 255, "dot in the middle");
            assert_eq!(alpha(a, 0, 0), 0, "clear corner");
            assert_ne!(a.data, b.data, "the ring moves");
        }
    }
}
