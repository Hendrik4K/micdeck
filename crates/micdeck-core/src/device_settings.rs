//! The receiver and transmitter settings MicDeck shows, laid out like the
//! device settings page of DJI's phone app, and the rules for when one can be
//! changed. Only settings marked [`Access::Writable`] are ever sent; the USB
//! adapter checks [`writable`] again right before writing.

use serde::Serialize;

use crate::model::WritePhase;
use crate::state::{State, WRITE_KEEP_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Receiver,
    Transmitter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// MicDeck changes it over USB and reads the result back.
    Writable,
    /// Shown, but never sent; the text says why.
    Locked(&'static str),
}

/// One setting as MicDeck offers it. `id` and the option values are the
/// setting table's (`dji_protocol`); everything else is MicDeck's wording.
#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub id: &'static str,
    pub group: Group,
    pub label: &'static str,
    pub description: &'static str,
    /// `(value, label)`; `off`/`on` make a switch.
    pub options: &'static [(&'static str, &'static str)],
    pub access: Access,
    /// The receiver only reports this while a transmitter is connected.
    pub needs_transmitter: bool,
}

const SWITCH: &[(&str, &str)] = &[("off", "Aus"), ("on", "An")];

/// In the order of DJI's app.
pub const SPECS: &[Spec] = &[
    Spec {
        id: "noise-cancel-power",
        group: Group::Receiver,
        label: "Rauschminderung",
        description: "Filtert Umgebungsgeräusche heraus. Ein kurzer Druck auf die Sendertaste schaltet sie ebenfalls um.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: false,
    },
    Spec {
        id: "noise-cancel",
        group: Group::Receiver,
        label: "Geräuschunterdrückung",
        description: "Einfach bewahrt die Klangtreue in ruhigen Räumen, Stark filtert mehr Lärm, etwa draußen.",
        options: &[("basic", "Einfach"), ("strong", "Stark")],
        access: Access::Writable,
        needs_transmitter: false,
    },
    Spec {
        id: "stereo",
        group: Group::Receiver,
        label: "Audio-Kanal",
        description: "Stereo legt Sender 1 auf links und Sender 2 auf rechts.",
        options: &[("mono", "Mono"), ("stereo", "Stereo")],
        access: Access::Writable,
        needs_transmitter: false,
    },
    Spec {
        id: "safety-track",
        group: Group::Receiver,
        label: "Backup-Aufnahme",
        description: "Nimmt eine zweite, leisere Spur als Reserve auf. Die Spuren trennst du später beim Schnitt.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: false,
    },
    Spec {
        id: "low-cut",
        group: Group::Receiver,
        label: "Hochpassfilter",
        description: "Senkt tiefe Störgeräusche wie Wind, Trittschall und Brummen ab.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: true,
    },
    Spec {
        id: "clip-limiter",
        group: Group::Receiver,
        label: "Clipping-Kontrolle",
        description: "Nimmt die Lautstärke automatisch zurück, bevor das Signal übersteuert.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: false,
    },
    Spec {
        id: "auto-off-15m",
        group: Group::Receiver,
        label: "Automatische Abschaltung",
        description: "Schaltet den Empfänger aus, wenn er 15 Minuten ohne Verbindung und unbenutzt ist.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: false,
    },
    Spec {
        id: "camera-power",
        group: Group::Receiver,
        label: "Empfänger An/Aus mit Kamera",
        description: "Schaltet den Empfänger zusammen mit der Kamera ein und aus.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: false,
    },
    Spec {
        id: "plug-free",
        group: Group::Receiver,
        label: "Externer Lautsprecher ohne Stecker",
        description: "Tonausgabe über den Adapter für Mobilgeräte am Erweiterungsanschluss.",
        options: SWITCH,
        access: Access::Locked(
            "Das Umschalten startet den Empfänger neu und unterbricht den Ton am PC. Bitte in der DJI-App ändern.",
        ),
        needs_transmitter: false,
    },
    Spec {
        id: "tx-auto-off-15m",
        group: Group::Transmitter,
        label: "Automatische Abschaltung",
        description: "Schaltet den Sender aus, wenn er 15 Minuten nicht verbunden ist, nicht aufnimmt und nicht bedient wird.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: true,
    },
    Spec {
        id: "noise-cancel-button",
        group: Group::Transmitter,
        label: "Einschalttaste für Rauschminderung",
        description: "Ein kurzer Druck auf die Sendertaste schaltet die Rauschminderung ein oder aus.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: true,
    },
    Spec {
        id: "mic-leds",
        group: Group::Transmitter,
        label: "Mikrofon-LED",
        description: "Statusleuchte am Sender.",
        options: SWITCH,
        access: Access::Writable,
        needs_transmitter: true,
    },
    Spec {
        id: "voice-tone",
        group: Group::Transmitter,
        label: "Klangfarbe",
        description: "Standard, voller oder heller Klang.",
        options: &[("standard", "Standard"), ("rich", "Voll"), ("bright", "Hell")],
        access: Access::Locked("Gibt es nur beim DJI Mic Mini 2."),
        needs_transmitter: true,
    },
];

/// The spec for `id`, if MicDeck shows that setting.
pub fn spec(id: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.id == id)
}

/// True if MicDeck may send `value` for setting `id` at all.
pub fn writable(id: &str, value: &str) -> bool {
    spec(id).is_some_and(|s| s.access == Access::Writable && s.options.iter().any(|(v, _)| *v == value))
}

/// Label of `value` for setting `id`, falling back to the raw value.
pub fn value_label(id: &str, value: &str) -> String {
    spec(id)
        .and_then(|s| s.options.iter().find(|(v, _)| *v == value))
        .map_or_else(|| value.to_string(), |(_, label)| (*label).to_string())
}

/// What the device has to look like for a setting to be changeable.
pub(crate) struct DeviceFacts<'a> {
    pub status: &'a dji_protocol::DeviceStatus,
    /// Transmitters switched on and linked right now.
    pub transmitters: usize,
}

impl DeviceFacts<'_> {
    fn is(&self, id: &str, value: &str) -> bool {
        self.status.settings.get(id).is_some_and(|v| v == value)
    }

    /// Mono with fewer than two transmitters: stereo would leave one side
    /// silent (transmitter 1 left, 2 right), so mono is the only sensible value.
    fn mono_only(&self, spec: &Spec) -> bool {
        spec.id == "stereo" && self.transmitters < 2 && self.is("stereo", "mono")
    }
}

/// Why `spec` cannot be changed right now, or `None` if it can.
pub(crate) fn lock_reason(spec: &Spec, facts: &DeviceFacts) -> Option<String> {
    if let Access::Locked(reason) = spec.access {
        return Some(reason.into());
    }
    let command = dji_protocol::model_by_id(&facts.status.model_id)
        .and_then(|m| m.settings().iter().find(|s| s.id == spec.id));
    match (command, facts.status.protocol_version) {
        (None, _) => return Some("Für dieses Modell nicht bekannt.".into()),
        (Some(c), Some(1)) if c.v1_command.is_none() => {
            return Some("Braucht die Empfänger-Firmware 02 oder neuer.".into());
        }
        (_, None) => return Some("Der Empfänger meldet sich noch.".into()),
        _ => {}
    }
    if !facts.status.settings.contains_key(spec.id) {
        return Some(if spec.needs_transmitter && facts.transmitters == 0 {
            "Erst einen Sender einschalten, dann lässt sich das einstellen.".into()
        } else {
            "Der Empfänger meldet diesen Wert gerade nicht.".into()
        });
    }
    if facts.mono_only(spec) {
        return Some("Stereo braucht zwei Sender (Sender 1 links, Sender 2 rechts).".into());
    }
    match spec.id {
        "noise-cancel" if facts.is("noise-cancel-power", "off") => {
            Some("Erst die Rauschminderung einschalten.".into())
        }
        "safety-track" if facts.is("stereo", "stereo") => Some("Nur im Mono-Modus verfügbar.".into()),
        "stereo" if facts.is("safety-track", "on") => Some("Erst die Backup-Aufnahme ausschalten.".into()),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionView {
    pub value: &'static str,
    pub label: &'static str,
}

/// One setting as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingView {
    pub id: &'static str,
    pub group: Group,
    pub label: &'static str,
    pub description: &'static str,
    pub options: Vec<OptionView>,
    /// Two options `off`/`on`: show a switch.
    pub switch: bool,
    /// What the receiver reports right now.
    pub value: Option<String>,
    /// Why it cannot be changed right now.
    pub locked: Option<String>,
    /// MicDeck never sends this setting (as opposed to a temporary lock).
    pub unsupported: bool,
    /// Only the current value makes sense right now (mono with one
    /// transmitter): show it as text, not as a control.
    pub fixed: bool,
    /// The value being written right now.
    pub pending: Option<String>,
}

/// One setting change in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteLogView {
    pub request: u64,
    pub unix_ms: u64,
    pub setting: String,
    pub label: String,
    pub from: Option<String>,
    pub to: String,
    pub phase: WritePhase,
    pub summary: String,
    /// `None` while the change is still running.
    pub ok: Option<bool>,
    pub seq: Option<u16>,
    pub packet: Option<String>,
}

/// German one-liner for a phase of a change to `setting`.
pub fn phase_summary(setting: &str, phase: &WritePhase) -> String {
    match phase {
        WritePhase::Rejected { reason } => format!("Nicht gesendet: {reason}"),
        WritePhase::Sent => "Gesendet, wartet auf Bestätigung …".into(),
        WritePhase::Acknowledged => "Bestätigt, wird zurückgelesen …".into(),
        WritePhase::Confirmed => "Übernommen und zurückgelesen".into(),
        WritePhase::Refused => "Vom Empfänger abgelehnt".into(),
        WritePhase::NoAnswer => "Keine Antwort vom Empfänger".into(),
        WritePhase::NotApplied { reported: Some(v) } => {
            format!("Bestätigt, aber der Empfänger meldet weiter „{}“", value_label(setting, v))
        }
        WritePhase::NotApplied { reported: None } => "Bestätigt, aber nicht zurückgelesen".into(),
        WritePhase::Failed { message } => format!("Fehler: {message}"),
    }
}

impl State {
    /// Every setting MicDeck shows, with its value and whether it can be changed.
    pub(crate) fn setting_views(&self, now_ms: u64) -> Vec<SettingView> {
        let primary = self.primary();
        let status = primary.and_then(|p| p.live_status(now_ms));
        let pending = self.pending_write(now_ms);
        SPECS
            .iter()
            .map(|spec| {
                let facts = status.map(|status| DeviceFacts {
                    status,
                    transmitters: status.tx.iter().flatten().count(),
                });
                let locked = match &facts {
                    _ if self.simulated && spec.access == Access::Writable => {
                        Some("Im Demo-Modus wird nichts an ein Gerät gesendet.".to_string())
                    }
                    Some(facts) => lock_reason(spec, facts),
                    None if primary.is_none() => Some("Kein Empfänger angeschlossen.".into()),
                    None => Some("Keine Statusdaten vom Empfänger.".into()),
                };
                SettingView {
                    id: spec.id,
                    group: spec.group,
                    label: spec.label,
                    description: spec.description,
                    options: spec.options.iter().map(|&(value, label)| OptionView { value, label }).collect(),
                    switch: spec.options.iter().map(|(v, _)| *v).eq(["off", "on"]),
                    value: status.and_then(|s| s.settings.get(spec.id).cloned()),
                    locked,
                    unsupported: matches!(spec.access, Access::Locked(_)),
                    fixed: facts.as_ref().is_some_and(|f| f.mono_only(spec)),
                    pending: pending.filter(|r| r.report.setting == spec.id).map(|r| r.report.value.clone()),
                }
            })
            .collect()
    }

    /// The setting changes, newest first.
    pub(crate) fn write_log(&self, now_ms: u64) -> Vec<WriteLogView> {
        self.writes
            .iter()
            .rev()
            .filter(|r| now_ms.saturating_sub(r.started_ms) <= WRITE_KEEP_MS)
            .map(|r| {
                let phase = r.phase(now_ms);
                let setting = r.report.setting.as_str();
                WriteLogView {
                    request: r.report.request,
                    unix_ms: r.report.unix_ms,
                    setting: setting.to_string(),
                    label: spec(setting).map_or_else(|| setting.to_string(), |s| s.label.to_string()),
                    from: r.from.as_deref().map(|v| value_label(setting, v)),
                    to: value_label(setting, &r.report.value),
                    summary: phase_summary(setting, &phase),
                    ok: phase.is_final().then_some(phase == WritePhase::Confirmed),
                    phase,
                    seq: r.report.seq,
                    packet: r.report.packet.clone(),
                }
            })
            .collect()
    }

    /// Checks a requested change. `Ok` holds the receiver to send it to,
    /// `Err` the reason it must not be sent.
    pub fn setting_target(&self, id: &str, value: &str, now_ms: u64) -> Result<String, String> {
        if self.simulated {
            return Err("Im Demo-Modus wird nichts an ein Gerät gesendet.".into());
        }
        if !spec(id).is_some_and(|s| s.options.iter().any(|(v, _)| *v == value)) {
            return Err("Unbekannte Einstellung.".into());
        }
        if self.pending_write(now_ms).is_some() {
            return Err("Eine andere Änderung läuft noch.".into());
        }
        let view = self.setting_views(now_ms).into_iter().find(|v| v.id == id);
        match view {
            Some(SettingView { locked: Some(reason), .. }) => Err(reason),
            Some(SettingView { value: Some(current), .. }) if current == value => {
                Err("Ist bereits so eingestellt.".into())
            }
            _ => self.primary().map(|p| p.usb.key.clone()).ok_or_else(|| "Kein Empfänger angeschlossen.".into()),
        }
    }
}
