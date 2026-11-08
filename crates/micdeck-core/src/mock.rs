//! Demo scenarios for trying the UI without hardware.
//!
//! Every snapshot built in demo mode is marked `simulated`, and so is every
//! generated [`LevelFrame`]; the UI must label both as demo data. Nothing here
//! is ever mixed with real measurements.

use std::collections::BTreeMap;

use dji_protocol::{DeviceStatus, Firmware, RxInfo, TxInfo};

use crate::level::{to_dbfs, FLOOR_DBFS};
use crate::model::{
    AudioEvent, AudioGraph, AudioService, AudioSource, LevelFrame, LinkState, NodeRunState,
    UsbEvent, UsbInterface, UsbReceiver, DJI_VENDOR_ID,
};
use crate::state::Input;

/// `node.name` of the demo receiver's input.
pub const DEMO_SOURCE: &str = "demo.dji-mic-mini.input";
const DEMO_OTHER_SOURCE: &str = "demo.builtin-mic.input";
const DEMO_KEY: &str = "demo-1";

/// What the demo meter shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoSignal {
    Speech,
    Silence,
    Clipping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    Ready,
    NoTransmitter,
    Silent,
    Muted,
    NotDefault,
    Clipping,
    LowBattery,
    NoPermission,
    Busy,
    Unplugged,
    NoAudioInput,
    PipeWireDown,
}

impl Scenario {
    pub const ALL: [Scenario; 12] = [
        Scenario::Ready,
        Scenario::NoTransmitter,
        Scenario::Silent,
        Scenario::Muted,
        Scenario::NotDefault,
        Scenario::Clipping,
        Scenario::LowBattery,
        Scenario::NoPermission,
        Scenario::Busy,
        Scenario::Unplugged,
        Scenario::NoAudioInput,
        Scenario::PipeWireDown,
    ];

    /// Name used on the command line (`--mock=<slug>`).
    pub fn slug(self) -> &'static str {
        match self {
            Scenario::Ready => "ready",
            Scenario::NoTransmitter => "no-transmitter",
            Scenario::Silent => "silent",
            Scenario::Muted => "muted",
            Scenario::NotDefault => "not-default",
            Scenario::Clipping => "clipping",
            Scenario::LowBattery => "low-battery",
            Scenario::NoPermission => "no-permission",
            Scenario::Busy => "busy",
            Scenario::Unplugged => "unplugged",
            Scenario::NoAudioInput => "no-audio-input",
            Scenario::PipeWireDown => "pipewire-down",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }

    /// Short German description for the demo banner.
    pub fn title(self) -> &'static str {
        match self {
            Scenario::Ready => "Alles bereit",
            Scenario::NoTransmitter => "Kein Sender eingeschaltet",
            Scenario::Silent => "Sender verbunden, aber Stille",
            Scenario::Muted => "Eingang stummgeschaltet",
            Scenario::NotDefault => "Anderes Standard-Mikrofon",
            Scenario::Clipping => "Übersteuerung",
            Scenario::LowBattery => "Akku fast leer",
            Scenario::NoPermission => "Keine USB-Berechtigung",
            Scenario::Busy => "Steuerung von anderem Programm belegt",
            Scenario::Unplugged => "Empfänger nicht angeschlossen",
            Scenario::NoAudioInput => "USB erkannt, aber kein Audioeingang",
            Scenario::PipeWireDown => "PipeWire nicht erreichbar",
        }
    }

    /// Inputs describing the scenario, to apply to a fresh [`crate::State`].
    pub fn inputs(self) -> Vec<Input> {
        let mut inputs = vec![Input::Usb(UsbEvent::Ready)];
        if self == Scenario::PipeWireDown {
            inputs.push(Input::Audio(AudioEvent::Service(AudioService::Unavailable {
                reason: "Keine Verbindung zum PipeWire-Dienst".into(),
            })));
            inputs.extend(self.receiver_inputs());
            return inputs;
        }
        inputs.extend(self.receiver_inputs());
        inputs.push(Input::Audio(AudioEvent::Service(AudioService::Available)));
        inputs.push(Input::Audio(AudioEvent::Graph(self.graph())));
        inputs
    }

    /// Inputs to repeat about twice a second, like a real receiver's status stream.
    pub fn periodic_inputs(self) -> Vec<Input> {
        match self.link() {
            Some(LinkState::Open) => vec![Input::Usb(UsbEvent::Telemetry {
                key: DEMO_KEY.into(),
                status: self.status(),
            })],
            _ => Vec::new(),
        }
    }

    /// What the demo meter should show, if the scenario has a DJI input at all.
    pub fn signal(self) -> Option<DemoSignal> {
        match self {
            Scenario::Unplugged | Scenario::NoAudioInput | Scenario::PipeWireDown => None,
            Scenario::NoTransmitter | Scenario::Silent | Scenario::Muted => Some(DemoSignal::Silence),
            Scenario::Clipping => Some(DemoSignal::Clipping),
            _ => Some(DemoSignal::Speech),
        }
    }

    fn link(self) -> Option<LinkState> {
        match self {
            Scenario::Unplugged => None,
            Scenario::NoPermission => Some(LinkState::PermissionDenied),
            Scenario::Busy => Some(LinkState::Busy),
            _ => Some(LinkState::Open),
        }
    }

    fn receiver_inputs(self) -> Vec<Input> {
        let Some(link) = self.link() else { return Vec::new() };
        let audio_ok = self != Scenario::NoAudioInput;
        let mut interfaces = vec![UsbInterface { number: 0, class: 0x03, driver: Some("usbhid".into()) }];
        if audio_ok {
            for n in 1..=3 {
                interfaces.push(UsbInterface { number: n, class: 0x01, driver: Some("snd-usb-audio".into()) });
            }
        }
        interfaces.push(UsbInterface { number: 6, class: 0xff, driver: None });
        let receiver = UsbReceiver {
            key: DEMO_KEY.into(),
            vendor_id: DJI_VENDOR_ID,
            product_id: 0x4011,
            model_name: Some("DJI Mic Mini".into()),
            product_string: Some("Demo-Empfänger".into()),
            serial: Some("DEMO-RX-0001".into()),
            interfaces,
            alsa_cards: if audio_ok { vec![5] } else { Vec::new() },
            link: link.clone(),
        };
        let mut inputs = vec![Input::Usb(UsbEvent::Receiver(receiver))];
        if link == LinkState::Open {
            inputs.push(Input::Usb(UsbEvent::Telemetry { key: DEMO_KEY.into(), status: self.status() }));
        }
        inputs
    }

    fn status(self) -> DeviceStatus {
        let fw = Some(Firmware([2, 3, 17, 0]));
        let tx = |unit: u8, battery: u8, level: u8| TxInfo {
            level: Some(level),
            serial: Some(format!("DEMO-TX-000{unit}")),
            firmware: fw,
            product_name: Some("DJI Mic Mini".into()),
            voice_tone: Some("standard".into()),
            charging: Some(false),
            battery: Some(battery),
        };
        let transmitters = match self {
            Scenario::NoTransmitter => [None, None],
            Scenario::LowBattery => [Some(tx(1, 7, 0x24)), None],
            Scenario::Silent => [Some(tx(1, 2, 0x14)), None],
            _ => [Some(tx(1, 2, 0x38)), Some(tx(2, 4, 0x30))],
        };
        // What v2 firmware reports; the receiver only knows the per-sender
        // settings while a sender is connected.
        let with_sender = transmitters.iter().any(Option::is_some);
        let settings: BTreeMap<String, String> = [
            ("noise-cancel", "basic", false),
            ("noise-cancel-power", "on", false),
            ("stereo", "mono", false),
            ("safety-track", "off", false),
            ("clip-limiter", "on", false),
            ("auto-off-15m", "on", false),
            ("camera-power", "off", false),
            ("plug-free", "off", false),
            ("low-cut", "off", true),
            ("tx-auto-off-15m", "on", true),
            ("noise-cancel-button", "on", true),
            ("mic-leds", "on", true),
        ]
        .into_iter()
        .filter(|&(_, _, per_sender)| with_sender || !per_sender)
        .map(|(k, v, _)| (k.to_string(), v.to_string()))
        .collect();
        DeviceStatus {
            model_id: "dji-mic-mini".into(),
            connected: true,
            nc_enabled: true,
            tx: transmitters,
            rx: Some(RxInfo {
                serial: Some("DEMO-RX-0001".into()),
                firmware: fw,
                product_name: Some("DJI Mic Mini".into()),
            }),
            settings,
            protocol_version: Some(2),
            gain_dial: Some(0),
        }
    }

    fn graph(self) -> AudioGraph {
        let other = AudioSource {
            id: 102,
            node_name: DEMO_OTHER_SOURCE.into(),
            description: "Eingebautes Mikrofon (Demo)".into(),
            usb_vendor_id: None,
            usb_product_id: None,
            alsa_card: Some(1),
            state: Some(NodeRunState::Suspended),
            muted: Some(false),
            volume: Some(1.0),
            channels: Some(2),
            capture_apps: Vec::new(),
        };
        let mut sources = vec![other];
        let has_dji = !matches!(self, Scenario::Unplugged | Scenario::NoAudioInput);
        if has_dji {
            sources.insert(
                0,
                AudioSource {
                    id: 101,
                    node_name: DEMO_SOURCE.into(),
                    description: "DJI Mic Mini (Demo)".into(),
                    usb_vendor_id: Some(DJI_VENDOR_ID),
                    usb_product_id: Some(0x4011),
                    alsa_card: Some(5),
                    state: Some(NodeRunState::Running),
                    muted: Some(self == Scenario::Muted),
                    volume: Some(1.0),
                    channels: Some(2),
                    capture_apps: Vec::new(),
                },
            );
        }
        let default = if has_dji && self != Scenario::NotDefault { DEMO_SOURCE } else { DEMO_OTHER_SOURCE };
        AudioGraph {
            sources,
            default_source: Some(default.into()),
            configured_default_source: Some(default.into()),
        }
    }
}

/// A demo level frame at `t_ms` since the meter started. Deterministic, so
/// tests and screenshots are reproducible.
pub fn demo_level(signal: DemoSignal, t_ms: u64, node_name: &str, channels: usize) -> LevelFrame {
    let channels = channels.max(1);
    if signal == DemoSignal::Silence {
        return LevelFrame {
            node_name: node_name.into(),
            peak_dbfs: vec![FLOOR_DBFS; channels],
            rms_dbfs: vec![FLOOR_DBFS; channels],
            clipped: false,
            digital_silence: true,
            frames: 1600,
            simulated: true,
        };
    }
    let mut peak = Vec::with_capacity(channels);
    let mut rms = Vec::with_capacity(channels);
    let mut clipped = false;
    for c in 0..channels {
        // Syllables of ~180 ms with pauses; amplitude varies per syllable.
        let syllable = (t_ms / 180) as u32 + c as u32 * 7;
        let speaking = !hash(syllable / 9).is_multiple_of(5) && !hash(syllable).is_multiple_of(4);
        let phase = (t_ms % 180) as f32 / 180.0;
        let envelope = (phase * std::f32::consts::PI).sin();
        let loudness = 0.08 + (hash(syllable) % 100) as f32 / 100.0 * 0.28;
        let noise = 0.0012 + (hash(t_ms as u32 / 33 + c as u32) % 10) as f32 * 0.0002;
        let mut amp = if speaking { noise + loudness * envelope } else { noise };
        if signal == DemoSignal::Clipping && t_ms % 2_000 < 250 {
            amp = 1.0;
            clipped = true;
        }
        peak.push(to_dbfs(amp));
        rms.push(to_dbfs(amp * 0.35));
    }
    LevelFrame {
        node_name: node_name.into(),
        peak_dbfs: peak,
        rms_dbfs: rms,
        clipped,
        digital_silence: false,
        frames: 1600,
        simulated: true,
    }
}

fn hash(x: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    h ^= h >> 15;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 13)
}
