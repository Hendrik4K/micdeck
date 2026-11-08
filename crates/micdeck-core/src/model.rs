//! What the adapters observe. These types cross thread boundaries and are
//! also serialized to the UI, so they carry no live handles.

use dji_protocol::DeviceStatus;
use serde::Serialize;

/// USB vendor id of DJI.
pub const DJI_VENDOR_ID: u16 = 0x2ca3;

/// State of the link to the receiver's vendor (telemetry) interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LinkState {
    /// The product id is not in MicDeck's model table; only USB and audio
    /// checks apply.
    Unsupported,
    /// Trying to open the interface.
    Opening,
    /// The device node is not accessible for this user (udev rule missing).
    PermissionDenied,
    /// Another program holds the interface.
    Busy,
    /// Opening or reading failed for another reason.
    Failed { message: String },
    /// The interface is open; status frames are expected.
    Open,
}

/// One USB interface of the receiver and the kernel driver bound to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbInterface {
    pub number: u8,
    pub class: u8,
    pub driver: Option<String>,
}

/// A DJI receiver seen on the USB bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbReceiver {
    /// Stable name of the USB port it sits in, e.g. `1-6`.
    pub key: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// Model name from MicDeck's model table, if the product id is known.
    pub model_name: Option<String>,
    pub product_string: Option<String>,
    pub serial: Option<String>,
    pub interfaces: Vec<UsbInterface>,
    /// ALSA sound cards the kernel created for this device.
    pub alsa_cards: Vec<u32>,
    pub link: LinkState,
}

impl UsbReceiver {
    /// True once the kernel's USB audio driver has claimed an interface.
    pub fn audio_driver_bound(&self) -> bool {
        self.interfaces
            .iter()
            .any(|i| i.driver.as_deref() == Some("snd-usb-audio"))
    }
}

/// Everything the USB adapter reports.
#[derive(Debug, Clone, PartialEq)]
pub enum UsbEvent {
    /// The first bus scan is complete; absence now really means absent.
    Ready,
    /// A receiver appeared or one of its properties changed (keyed by `key`).
    Receiver(UsbReceiver),
    /// The receiver at `key` was unplugged.
    Removed { key: String },
    /// A decoded status frame from the receiver at `key`.
    Telemetry { key: String, status: DeviceStatus },
    /// Hotplug notifications are unavailable; the adapter falls back to polling.
    WatchFailed { message: String },
    /// One step of a setting change on the receiver at `key`.
    Write { key: String, report: WriteReport },
}

/// Where a request to change a device setting stands. Only `Confirmed`
/// means the receiver now reports the new value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum WritePhase {
    /// MicDeck did not send it; `reason` says why.
    Rejected { reason: String },
    /// Sent; waiting for the receiver's acknowledgement.
    Sent,
    /// Acknowledged; waiting for the receiver to report the new value.
    Acknowledged,
    /// The receiver reports the new value.
    Confirmed,
    /// The receiver answered that it does not accept the command.
    Refused,
    /// No acknowledgement arrived in time.
    NoAnswer,
    /// Acknowledged, but the receiver keeps reporting `reported`.
    NotApplied { reported: Option<String> },
    /// The command could not be written to the USB endpoint.
    Failed { message: String },
}

impl WritePhase {
    /// True once nothing more will happen for this request.
    pub fn is_final(&self) -> bool {
        !matches!(self, WritePhase::Sent | WritePhase::Acknowledged)
    }
}

/// One step of a setting change. All steps of one change share `request`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteReport {
    pub request: u64,
    pub setting: String,
    pub value: String,
    pub phase: WritePhase,
    /// Wall-clock time of this step, in Unix milliseconds.
    pub unix_ms: u64,
    /// Sequence number of the command, once one was built.
    pub seq: Option<u16>,
    /// The command as sent, in hex, for the log.
    pub packet: Option<String>,
}

/// Whether the PipeWire connection works.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AudioService {
    Connecting,
    Available,
    Unavailable { reason: String },
}

/// PipeWire node state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeRunState {
    Creating,
    Suspended,
    Idle,
    Running,
    Error,
}

/// An audio input (PipeWire `Audio/Source` node).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSource {
    pub id: u32,
    pub node_name: String,
    pub description: String,
    pub usb_vendor_id: Option<u16>,
    pub usb_product_id: Option<u16>,
    pub alsa_card: Option<u32>,
    pub state: Option<NodeRunState>,
    pub muted: Option<bool>,
    /// Volume as desktop mixers show it (cubic scale, 1.0 = 100 %).
    pub volume: Option<f32>,
    pub channels: Option<u32>,
    /// Other programs recording from this input right now (MicDeck's own
    /// meter and microphone test are not listed).
    pub capture_apps: Vec<String>,
}

impl AudioSource {
    /// True if this input belongs to a DJI USB device.
    pub fn is_dji(&self) -> bool {
        self.usb_vendor_id == Some(DJI_VENDOR_ID)
    }
}

/// The audio inputs PipeWire offers and which one is the default.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioGraph {
    pub sources: Vec<AudioSource>,
    /// `node.name` of the default input WirePlumber currently uses.
    pub default_source: Option<String>,
    /// `node.name` the user explicitly chose as default, if any.
    pub configured_default_source: Option<String>,
}

/// The microphone session: MicDeck listens, records what you say (up to a
/// limit), plays it back, and starts over when you speak again. Everything
/// stays in memory and is dropped when the take is replaced or the session
/// ends. Only ever started by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MicTestState {
    #[default]
    Idle,
    /// Waiting for speech; `first` before the first take, otherwise after a
    /// playback (speaking records a new take).
    Listening { first: bool },
    /// Recording a take, at most `max_ms` long.
    Recording { max_ms: u64 },
    /// Playing the take back (`duration_ms` long).
    Playing { duration_ms: u64 },
    Failed { message: String },
}

/// State of MicDeck's own level-meter stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MeterState {
    Off,
    Starting { node_name: String },
    Running { node_name: String, channels: u32, rate: u32 },
    Failed { message: String },
}

/// Levels measured from real samples over one short window (~33 ms).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelFrame {
    pub node_name: String,
    /// Peak level per channel in dBFS (floored at [`crate::level::FLOOR_DBFS`]).
    pub peak_dbfs: Vec<f32>,
    /// RMS level per channel in dBFS.
    pub rms_dbfs: Vec<f32>,
    /// A sample reached full scale in this window.
    pub clipped: bool,
    /// Every sample in this window was exactly zero.
    pub digital_silence: bool,
    /// Samples per channel in this window.
    pub frames: u32,
    /// True for demo data; the UI must label it as such.
    pub simulated: bool,
}

/// Everything the PipeWire adapter reports.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Service(AudioService),
    Graph(AudioGraph),
    Meter(MeterState),
    Level(LevelFrame),
    MicTest(MicTestState),
}
