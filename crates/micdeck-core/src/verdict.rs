//! Turns [`State`] into what the UI shows: the overall verdict, the list of
//! checks behind it, and device views. All user-facing text lives here.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::device_settings::{SettingView, WriteLogView};
use crate::model::{AudioService, AudioSource, LinkState, MeterState, MicTestState, NodeRunState};
use crate::state::{
    ReceiverEntry, State, AUDIO_SETUP_GRACE_MS, CLIP_HOLD_MS, LEVEL_STALE_MS,
    SILENCE_AFTER_MS, TELEMETRY_GRACE_MS,
};

/// Overall readiness, shown as the big traffic light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// Still gathering information.
    Pending,
    /// Connected and ready.
    Ok,
    /// Connected, but not fully ready.
    Warn,
    /// Disconnected or broken.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub level: Level,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckId {
    Usb,
    Radio,
    AudioInput,
    Mute,
    DefaultInput,
    Signal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckLevel {
    Ok,
    Warn,
    Error,
    Pending,
    /// Not applicable right now (e.g. no receiver to check).
    Inactive,
}

/// One line of the "what MicDeck checked" list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: CheckId,
    pub level: CheckLevel,
    pub label: String,
    pub value: String,
    /// What the user can do about a problem.
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverView {
    pub key: String,
    pub name: String,
    pub usb_id: String,
    pub serial: Option<String>,
    pub firmware: Option<String>,
    pub protocol_version: Option<u8>,
    pub link: LinkState,
    /// Status frames are arriving right now.
    pub telemetry_live: bool,
    pub gain_dial_db: Option<i8>,
    /// Current value of every setting the receiver reports.
    pub settings: BTreeMap<String, String>,
    pub alsa_cards: Vec<u32>,
}

/// Battery as the device reports it: a 7-step gauge, no percentage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryView {
    /// Raw gauge, 1 (full) … 7 (empty, about to switch off).
    pub gauge: u8,
    /// Filled bars out of six.
    pub bars: u8,
    pub label: String,
    pub charging: bool,
    pub low: bool,
    pub critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransmitterView {
    pub unit: u8,
    /// `None` when MicDeck has no status data, i.e. the radio state is unknown.
    pub connected: Option<bool>,
    pub name: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
    pub battery: Option<BatteryView>,
    /// Docked/charging, when the firmware reports it (even without a battery gauge).
    pub charging: Option<bool>,
    /// Raw input level the receiver reports for this transmitter (no unit).
    pub radio_level: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub node_name: String,
    pub description: String,
    pub is_dji: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SignalStatus {
    Unknown,
    Present,
    Silent,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalView {
    pub status: SignalStatus,
    pub peak_dbfs: Option<f32>,
    /// Smoothed RMS level over the last seconds.
    pub loudness_dbfs: Option<f32>,
    /// Plain-language label for `loudness_dbfs`, e.g. "Normale Sprechlautstärke".
    pub loudness: Option<&'static str>,
    pub silent_for_ms: Option<u64>,
    pub clipped_recently: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioView {
    pub service: AudioService,
    /// The DJI receiver's input, if PipeWire offers one.
    pub source: Option<AudioSource>,
    pub default_input: Option<SourceRef>,
    /// The user picked the default explicitly (vs. WirePlumber's automatic choice).
    pub default_configured: bool,
    pub inputs: Vec<SourceRef>,
    pub signal: SignalView,
    pub mic_test: MicTestState,
}

/// Everything the UI renders, produced by [`State::snapshot`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Demo data; the UI must say so prominently.
    pub simulated: bool,
    /// The demo scenario being shown, in demo mode.
    pub demo_label: Option<String>,
    pub verdict: Verdict,
    pub checks: Vec<Check>,
    pub receiver: Option<ReceiverView>,
    /// Further DJI receivers beyond the one shown.
    pub other_receivers: usize,
    pub transmitters: Vec<TransmitterView>,
    pub audio: AudioView,
    pub meter: MeterState,
    /// Hotplug notifications failed; MicDeck polls instead.
    pub usb_watch_error: Option<String>,
    /// Receiver and transmitter settings, in the order of DJI's app.
    pub device_settings: Vec<SettingView>,
    /// Setting changes, newest first.
    pub device_log: Vec<WriteLogView>,
}

fn check(id: CheckId, level: CheckLevel, label: &str, value: impl Into<String>) -> Check {
    Check { id, level, label: label.to_string(), value: value.into(), hint: None }
}

impl Check {
    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

fn battery_view(gauge: u8, charging: bool) -> Option<BatteryView> {
    let label = match gauge {
        1 => "voll",
        2 => "hoch",
        3 => "gut",
        4 => "mittel",
        5 => "niedrig",
        6 => "sehr niedrig",
        7 => "leer",
        _ => return None,
    };
    Some(BatteryView {
        gauge,
        bars: 7 - gauge,
        label: label.to_string(),
        charging,
        low: gauge >= 6,
        critical: gauge == 7,
    })
}

fn unit_list(units: &[u8]) -> String {
    match units {
        [] => "Kein Sender verbunden".into(),
        [u] => format!("Sender {u} verbunden"),
        _ => "Beide Sender verbunden".into(),
    }
}

/// What the radio check concluded, kept for the verdict.
enum Radio {
    /// No receiver, or the model has no telemetry.
    NotApplicable,
    Pending,
    /// Status data is not available; the reason is in the check.
    Unknown,
    /// Live status data: these units are connected.
    Known(Vec<u8>),
}

impl State {
    /// Everything the UI shows, as of `now_ms`.
    pub fn snapshot(&self, now_ms: u64) -> Snapshot {
        let primary = self.primary();
        let source = self.dji_source();
        let status = primary.and_then(|p| p.live_status(now_ms));
        let signal = self.signal_view(now_ms);

        let receiver_name = primary.map(|p| receiver_name(p, status));
        let transmitters: Vec<TransmitterView> = (0..2)
            .map(|i| {
                let unit = i as u8 + 1;
                let Some(st) = status else {
                    return TransmitterView {
                        unit,
                        connected: None,
                        name: None,
                        serial: None,
                        firmware: None,
                        battery: None,
                        charging: None,
                        radio_level: None,
                    };
                };
                match &st.tx[i] {
                    None => TransmitterView {
                        unit,
                        connected: Some(false),
                        name: None,
                        serial: None,
                        firmware: None,
                        battery: None,
                        charging: None,
                        radio_level: None,
                    },
                    Some(tx) => TransmitterView {
                        unit,
                        connected: Some(true),
                        name: tx.product_name.clone(),
                        serial: tx.serial.clone(),
                        firmware: tx.firmware.map(|f| f.to_string()),
                        battery: tx
                            .battery
                            .and_then(|g| battery_view(g, tx.charging.unwrap_or(false))),
                        charging: tx.charging,
                        radio_level: tx.level,
                    },
                }
            })
            .collect();

        let (usb_check, radio_check, radio) = self.usb_and_radio_checks(primary, source, &transmitters, now_ms);
        let audio_check = self.audio_input_check(primary, source, now_ms);
        let mute_check = mute_check(source);
        let default_check = self.default_check(source);
        let signal_check = self.signal_check(source, &signal, &radio);

        let verdict = self.verdict(
            primary,
            source,
            &radio,
            &signal,
            &transmitters,
            receiver_name.as_deref(),
            now_ms,
        );

        let inputs = self
            .graph
            .sources
            .iter()
            .map(|s| SourceRef {
                node_name: s.node_name.clone(),
                description: s.description.clone(),
                is_dji: self.is_receiver_source(s),
            })
            .collect::<Vec<_>>();
        let default_input = self.graph.default_source.as_ref().map(|name| {
            inputs.iter().find(|r| &r.node_name == name).cloned().unwrap_or(SourceRef {
                node_name: name.clone(),
                description: name.clone(),
                is_dji: false,
            })
        });

        Snapshot {
            simulated: self.simulated,
            demo_label: self.demo_label.clone(),
            verdict,
            checks: vec![usb_check, radio_check, audio_check, mute_check, default_check, signal_check],
            receiver: primary.map(|p| ReceiverView {
                key: p.usb.key.clone(),
                name: receiver_name.clone().unwrap_or_default(),
                usb_id: format!("{:04x}:{:04x}", p.usb.vendor_id, p.usb.product_id),
                serial: status
                    .and_then(|s| s.rx.as_ref())
                    .and_then(|rx| rx.serial.clone())
                    .or_else(|| p.usb.serial.clone()),
                firmware: status
                    .and_then(|s| s.rx.as_ref())
                    .and_then(|rx| rx.firmware.map(|f| f.to_string())),
                protocol_version: status.and_then(|s| s.protocol_version),
                link: p.usb.link.clone(),
                telemetry_live: status.is_some(),
                gain_dial_db: status.and_then(|s| s.gain_dial),
                settings: status.map(|s| s.settings.clone()).unwrap_or_default(),
                alsa_cards: p.usb.alsa_cards.clone(),
            }),
            other_receivers: self.receivers.len().saturating_sub(1),
            transmitters,
            audio: AudioView {
                service: self.audio_service.clone(),
                source: source.cloned(),
                default_input,
                default_configured: self.graph.configured_default_source.is_some(),
                inputs,
                signal,
                mic_test: self.mic_test.clone(),
            },
            meter: self.meter.clone(),
            usb_watch_error: self.usb_watch_error.clone(),
            device_settings: self.setting_views(now_ms),
            device_log: self.write_log(now_ms),
        }
    }

    fn signal_view(&self, now_ms: u64) -> SignalView {
        let s = &self.signal;
        let running = matches!(self.meter, MeterState::Running { .. });
        let fresh = s.last_frame_ms.is_some_and(|t| now_ms.saturating_sub(t) <= LEVEL_STALE_MS);
        if !(running && fresh) {
            return SignalView {
                status: SignalStatus::Unknown,
                peak_dbfs: None,
                loudness_dbfs: None,
                loudness: None,
                silent_for_ms: None,
                clipped_recently: false,
            };
        }
        let silent_for = s.silent_since_ms.map(|t| now_ms.saturating_sub(t));
        let status = match silent_for {
            Some(d) if d >= SILENCE_AFTER_MS => SignalStatus::Silent,
            _ if s.last_sound_ms.is_some() => SignalStatus::Present,
            _ => SignalStatus::Unknown,
        };
        SignalView {
            status,
            peak_dbfs: s.last_peak_dbfs,
            // Rounded so the snapshot (and the UI) only changes when the value does.
            loudness_dbfs: s.loudness.dbfs().map(f32::round),
            loudness: s.loudness.class().map(|c| c.label()),
            silent_for_ms: silent_for,
            clipped_recently: s.last_clip_ms.is_some_and(|t| now_ms.saturating_sub(t) <= CLIP_HOLD_MS),
        }
    }

    fn usb_and_radio_checks(
        &self,
        primary: Option<&ReceiverEntry>,
        source: Option<&AudioSource>,
        transmitters: &[TransmitterView],
        now_ms: u64,
    ) -> (Check, Check, Radio) {
        const USB: &str = "Empfänger (USB)";
        const RADIO: &str = "Funkverbindung";

        let Some(p) = primary else {
            let usb = if !self.usb_ready {
                check(CheckId::Usb, CheckLevel::Pending, USB, "Wird gesucht …")
            } else if source.is_some() {
                check(CheckId::Usb, CheckLevel::Ok, USB, "Über das Audiosystem erkannt")
                    .hint("USB-Details sind gerade nicht verfügbar.")
            } else {
                check(CheckId::Usb, CheckLevel::Error, USB, "Nicht angeschlossen").hint(
                    "Empfänger mit einem USB-C-Datenkabel an den PC anschließen. \
                     Reine Ladekabel übertragen keine Daten.",
                )
            };
            let radio = check(CheckId::Radio, CheckLevel::Inactive, RADIO, "–");
            return (usb, radio, Radio::NotApplicable);
        };

        let usb_id = format!("{:04x}:{:04x}", p.usb.vendor_id, p.usb.product_id);
        let usb = match &p.usb.model_name {
            Some(name) => check(CheckId::Usb, CheckLevel::Ok, USB, format!("{name} angeschlossen")),
            None => check(CheckId::Usb, CheckLevel::Warn, USB, format!("Unbekanntes DJI-Gerät ({usb_id})"))
                .hint("MicDeck kennt dieses Modell noch nicht. Ton und Pegel werden trotzdem geprüft."),
        };

        let (radio_check, radio) = match &p.usb.link {
            LinkState::Unsupported => (
                check(CheckId::Radio, CheckLevel::Inactive, RADIO, "Für dieses Modell nicht verfügbar"),
                Radio::NotApplicable,
            ),
            LinkState::Opening => (
                check(CheckId::Radio, CheckLevel::Pending, RADIO, "Verbinde …"),
                Radio::Pending,
            ),
            LinkState::PermissionDenied => (
                check(CheckId::Radio, CheckLevel::Warn, RADIO, "Keine Berechtigung für Statusdaten").hint(
                    "Für Funkstatus und Akku braucht MicDeck einmalig eine udev-Regel \
                     (siehe Einrichtung). Ton und Pegel funktionieren auch ohne.",
                ),
                Radio::Unknown,
            ),
            LinkState::Busy => (
                check(CheckId::Radio, CheckLevel::Warn, RADIO, "Von anderem Programm belegt").hint(
                    "Ein anderes Programm nutzt die Steuerschnittstelle des Empfängers \
                     (z. B. DJI Mic Control oder dji-mic-mo im Browser). Schließe es, \
                     dann verbindet sich MicDeck automatisch.",
                ),
                Radio::Unknown,
            ),
            LinkState::Failed { message } => (
                check(CheckId::Radio, CheckLevel::Warn, RADIO, "Steuerverbindung gestört")
                    .hint(format!("{message}. Empfänger einmal aus- und wieder einstecken.")),
                Radio::Unknown,
            ),
            LinkState::Open => {
                if p.live_status(now_ms).is_some() {
                    let units: Vec<u8> = transmitters
                        .iter()
                        .filter(|t| t.connected == Some(true))
                        .map(|t| t.unit)
                        .collect();
                    let c = if units.is_empty() {
                        check(CheckId::Radio, CheckLevel::Warn, RADIO, unit_list(&units)).hint(
                            "Sender einschalten (Einschalttaste) und prüfen, ob sie mit diesem \
                             Empfänger gekoppelt sind. Ohne Sender liefert der Empfänger nur Stille.",
                        )
                    } else {
                        check(CheckId::Radio, CheckLevel::Ok, RADIO, unit_list(&units))
                    };
                    (c, Radio::Known(units))
                } else if [p.link_open_ms, p.last_frame_ms]
                    .iter()
                    .flatten()
                    .any(|&t| now_ms.saturating_sub(t) < TELEMETRY_GRACE_MS)
                {
                    // Just opened, or a short gap in an otherwise working stream.
                    (
                        check(CheckId::Radio, CheckLevel::Pending, RADIO, "Warte auf Statusdaten …"),
                        Radio::Pending,
                    )
                } else {
                    (
                        check(CheckId::Radio, CheckLevel::Warn, RADIO, "Keine Statusdaten").hint(
                            "Der Empfänger sendet keine lesbaren Statusdaten – vermutlich eine \
                             Firmware, die MicDeck noch nicht kennt. Ton und Pegel werden trotzdem geprüft.",
                        ),
                        Radio::Unknown,
                    )
                }
            }
        };
        (usb, radio_check, radio)
    }

    fn audio_input_check(
        &self,
        primary: Option<&ReceiverEntry>,
        source: Option<&AudioSource>,
        now_ms: u64,
    ) -> Check {
        const LABEL: &str = "Audioeingang";
        match &self.audio_service {
            AudioService::Unavailable { reason } => {
                check(CheckId::AudioInput, CheckLevel::Error, LABEL, "PipeWire nicht erreichbar").hint(
                    format!(
                        "{reason}. Ab- und wieder anmelden oder im Terminal \
                         „systemctl --user restart pipewire wireplumber“ ausführen."
                    ),
                )
            }
            AudioService::Connecting => {
                check(CheckId::AudioInput, CheckLevel::Pending, LABEL, "Verbinde mit PipeWire …")
            }
            AudioService::Available => match (source, primary) {
                (Some(s), _) if s.state == Some(NodeRunState::Error) => {
                    check(CheckId::AudioInput, CheckLevel::Warn, LABEL, format!("„{}“ meldet einen Fehler", s.description))
                        .hint("Empfänger einmal aus- und wieder einstecken.")
                }
                (Some(s), _) => check(CheckId::AudioInput, CheckLevel::Ok, LABEL, format!("„{}“", s.description)),
                (None, Some(p)) if now_ms.saturating_sub(p.attached_ms) < AUDIO_SETUP_GRACE_MS => {
                    check(CheckId::AudioInput, CheckLevel::Pending, LABEL, "Wird eingerichtet …")
                }
                (None, Some(p)) if p.usb.alsa_cards.is_empty() && !p.usb.audio_driver_bound() => {
                    check(CheckId::AudioInput, CheckLevel::Error, LABEL, "Kein Audiotreiber geladen").hint(
                        "Linux erkennt das USB-Gerät, aber nicht als Audiogerät. Einen anderen \
                         USB-Anschluss oder ein anderes Kabel probieren und den Empfänger neu einstecken.",
                    )
                }
                (None, Some(_)) => {
                    check(CheckId::AudioInput, CheckLevel::Error, LABEL, "Im Audiosystem nicht verfügbar").hint(
                        "Das Gerät ist als Soundkarte bekannt, bietet aber keinen Eingang an. In den \
                         Toneinstellungen beim Gerät ein Profil mit Eingang wählen (nicht „Aus“).",
                    )
                }
                (None, None) => check(CheckId::AudioInput, CheckLevel::Inactive, LABEL, "–"),
            },
        }
    }

    fn default_check(&self, source: Option<&AudioSource>) -> Check {
        const LABEL: &str = "Standard-Mikrofon";
        let Some(s) = source else {
            return check(CheckId::DefaultInput, CheckLevel::Inactive, LABEL, "–");
        };
        match &self.graph.default_source {
            Some(d) if *d == s.node_name => {
                check(CheckId::DefaultInput, CheckLevel::Ok, LABEL, "Ist Standard-Mikrofon")
            }
            Some(d) => {
                let other = self
                    .graph
                    .sources
                    .iter()
                    .find(|x| &x.node_name == d)
                    .map(|x| x.description.clone())
                    .unwrap_or_else(|| d.clone());
                check(CheckId::DefaultInput, CheckLevel::Warn, LABEL, format!("Standard ist „{other}“")).hint(
                    "Programme, die das Standard-Mikrofon nutzen (z. B. Discord oder der Browser), \
                     hören den DJI-Empfänger nicht. In den Toneinstellungen unter „Eingabe“ den \
                     DJI-Empfänger auswählen.",
                )
            }
            None => check(CheckId::DefaultInput, CheckLevel::Warn, LABEL, "Kein Standard festgelegt").hint(
                "Linux hat gerade kein Standard-Mikrofon; Programme wählen dann selbst eins. \
                 In den Toneinstellungen unter „Eingabe“ den DJI-Empfänger auswählen.",
            ),
        }
    }

    fn signal_check(&self, source: Option<&AudioSource>, signal: &SignalView, radio: &Radio) -> Check {
        const LABEL: &str = "Ton kommt an";
        if source.is_none() {
            return check(CheckId::Signal, CheckLevel::Inactive, LABEL, "–");
        }
        match signal.status {
            SignalStatus::Present if signal.clipped_recently => {
                check(CheckId::Signal, CheckLevel::Warn, LABEL, "Übersteuert").hint(
                    "Der Pegel erreicht 0 dBFS. Gain am Empfänger senken oder mehr Abstand zum Mikrofon halten.",
                )
            }
            SignalStatus::Present => {
                let value = match (signal.loudness, signal.loudness_dbfs) {
                    (Some(label), Some(db)) => format!("{label} · Ø\u{a0}{}\u{a0}dB", format!("{db:.0}").replace('-', "−")),
                    _ => "Ja".into(),
                };
                check(CheckId::Signal, CheckLevel::Ok, LABEL, value)
            }
            SignalStatus::Silent => {
                let muted = source.is_some_and(|s| s.muted == Some(true));
                let zero_volume = source.is_some_and(|s| s.volume.is_some_and(|v| v <= 0.0));
                let hint = match radio {
                    // The mute check already says how to fix this; don't send
                    // people off to replug hardware that works.
                    _ if muted => "Der Eingang ist stummgeschaltet, deshalb kommt nur Stille an.",
                    _ if zero_volume => "Die Eingangslautstärke steht auf 0\u{a0}%, deshalb kommt nur Stille an.",
                    Radio::Known(units) if units.is_empty() => {
                        "Es ist kein Sender verbunden, deshalb liefert der Empfänger nur Stille."
                    }
                    Radio::Known(_) => {
                        "Ein Sender ist verbunden, trotzdem kommt nur Stille an. Sender aus- und \
                         wieder einschalten; hilft das nicht, den Empfänger neu einstecken."
                    }
                    _ => "Es kommt nur digitale Stille an. Ist ein Sender eingeschaltet?",
                };
                check(CheckId::Signal, CheckLevel::Warn, LABEL, "Nur Stille").hint(hint)
            }
            SignalStatus::Unknown => match &self.meter {
                MeterState::Failed { message } => {
                    check(CheckId::Signal, CheckLevel::Warn, LABEL, "Pegelmessung fehlgeschlagen").hint(message.clone())
                }
                _ if !self.meter_wanted => {
                    check(CheckId::Signal, CheckLevel::Inactive, LABEL, "Pegelmessung pausiert")
                }
                _ => check(CheckId::Signal, CheckLevel::Pending, LABEL, "Wird gemessen …"),
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn verdict(
        &self,
        primary: Option<&ReceiverEntry>,
        source: Option<&AudioSource>,
        radio: &Radio,
        signal: &SignalView,
        transmitters: &[TransmitterView],
        receiver_name: Option<&str>,
        now_ms: u64,
    ) -> Verdict {
        let v = |level, title: &str, detail: String| Verdict { level, title: title.into(), detail };

        if let AudioService::Unavailable { .. } = self.audio_service {
            return v(
                Level::Error,
                "Audiosystem nicht erreichbar",
                "PipeWire läuft nicht – gerade kann kein Programm aufnehmen.".into(),
            );
        }
        if !self.usb_ready || self.audio_service == AudioService::Connecting {
            return v(Level::Pending, "Wird geprüft …", "MicDeck sucht nach dem Empfänger.".into());
        }
        let Some(s) = source else {
            return match primary {
                None => v(Level::Error, "Nicht verbunden", "Kein DJI-Empfänger am USB gefunden.".into()),
                Some(p) if now_ms.saturating_sub(p.attached_ms) < AUDIO_SETUP_GRACE_MS => v(
                    Level::Pending,
                    "Empfänger erkannt",
                    "Der Audioeingang wird eingerichtet …".into(),
                ),
                Some(_) => v(
                    Level::Error,
                    "Kein Audioeingang",
                    "Der Empfänger steckt am USB, aber Linux stellt keinen Audioeingang bereit.".into(),
                ),
            };
        };

        if s.muted == Some(true) {
            return v(Level::Warn, "Stummgeschaltet", "Der DJI-Eingang ist in Linux stummgeschaltet.".into());
        }
        if matches!(radio, Radio::Known(units) if units.is_empty()) {
            return v(
                Level::Warn,
                "Verbunden – kein Sender aktiv",
                "Der Empfänger ist bereit, aber kein Sender ist eingeschaltet. Es kommt kein Ton an.".into(),
            );
        }
        if signal.status == SignalStatus::Silent {
            return v(
                Level::Warn,
                "Verbunden – aber kein Ton",
                "Am PC kommt nur digitale Stille an.".into(),
            );
        }
        if s.volume.is_some_and(|vol| vol <= 0.0) {
            return v(Level::Warn, "Lautstärke auf 0 %", "Der DJI-Eingang ist ganz leise gestellt.".into());
        }
        if self.graph.default_source.as_deref() != Some(s.node_name.as_str()) {
            return v(
                Level::Warn,
                "Nicht als Standard ausgewählt",
                "Programme, die das Standard-Mikrofon nutzen, hören ein anderes Gerät.".into(),
            );
        }
        if let Some(t) = transmitters.iter().find(|t| t.battery.as_ref().is_some_and(|b| b.critical)) {
            return v(
                Level::Warn,
                "Akku fast leer",
                format!("Sender {} schaltet sich gleich ab.", t.unit),
            );
        }
        let sound_confirmed = signal.status == SignalStatus::Present;
        match radio {
            Radio::Pending if !sound_confirmed => {
                return v(Level::Pending, "Wird geprüft …", "Warte auf Daten vom Empfänger.".into());
            }
            Radio::Unknown | Radio::NotApplicable if !sound_confirmed => {
                return v(
                    Level::Warn,
                    "Verbunden – Sender-Status unbekannt",
                    "Ob ein Sender sendet, lässt sich gerade nicht prüfen.".into(),
                );
            }
            _ => {}
        }

        let mut parts = vec![receiver_name.unwrap_or("DJI-Empfänger").to_string()];
        if let Radio::Known(units) = radio {
            parts.push(unit_list(units));
        }
        if sound_confirmed {
            parts.push("Ton kommt an".into());
        }
        v(Level::Ok, "Einsatzbereit", parts.join(" · "))
    }
}

fn mute_check(source: Option<&AudioSource>) -> Check {
    const LABEL: &str = "Lautstärke";
    let Some(s) = source else {
        return check(CheckId::Mute, CheckLevel::Inactive, LABEL, "–");
    };
    if s.muted == Some(true) {
        return check(CheckId::Mute, CheckLevel::Warn, LABEL, "Stummgeschaltet")
            .hint("In den Toneinstellungen unter „Eingabe“ die Stummschaltung aufheben.");
    }
    // No-break space keeps "100 %" together when the value wraps.
    match s.volume {
        Some(v) if v <= 0.0 => check(CheckId::Mute, CheckLevel::Warn, LABEL, "0\u{a0}%")
            .hint("In den Toneinstellungen die Eingangslautstärke erhöhen."),
        Some(v) => check(CheckId::Mute, CheckLevel::Ok, LABEL, format!("{:.0}\u{a0}% · nicht stumm", v * 100.0)),
        None => check(CheckId::Mute, CheckLevel::Ok, LABEL, "Nicht stumm"),
    }
}

fn receiver_name(p: &ReceiverEntry, status: Option<&dji_protocol::DeviceStatus>) -> String {
    status
        .and_then(|s| s.rx.as_ref())
        .and_then(|rx| rx.product_name.clone())
        .or_else(|| p.usb.model_name.clone())
        .or_else(|| p.usb.product_string.clone())
        .unwrap_or_else(|| "DJI-Empfänger".into())
}
