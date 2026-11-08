// Mirrors the serialized types of crates/micdeck-core (serde, camelCase).

export type Level = "pending" | "ok" | "warn" | "error";
export type CheckLevel = "ok" | "warn" | "error" | "pending" | "inactive";
export type CheckId = "usb" | "radio" | "audioInput" | "mute" | "defaultInput" | "signal";

export interface Verdict {
  level: Level;
  title: string;
  detail: string;
}

export interface Check {
  id: CheckId;
  level: CheckLevel;
  label: string;
  value: string;
  hint: string | null;
}

export type LinkState =
  | { kind: "unsupported" }
  | { kind: "opening" }
  | { kind: "permissionDenied" }
  | { kind: "busy" }
  | { kind: "failed"; message: string }
  | { kind: "open" };

export interface ReceiverView {
  key: string;
  name: string;
  usbId: string;
  serial: string | null;
  firmware: string | null;
  protocolVersion: number | null;
  link: LinkState;
  telemetryLive: boolean;
  gainDialDb: number | null;
  settings: Record<string, string>;
  alsaCards: number[];
}

export interface BatteryView {
  gauge: number;
  bars: number;
  label: string;
  charging: boolean;
  low: boolean;
  critical: boolean;
}

export interface TransmitterView {
  unit: number;
  connected: boolean | null;
  name: string | null;
  serial: string | null;
  firmware: string | null;
  battery: BatteryView | null;
  charging: boolean | null;
  radioLevel: number | null;
}

export type AudioService =
  | { kind: "connecting" }
  | { kind: "available" }
  | { kind: "unavailable"; reason: string };

export type NodeRunState = "creating" | "suspended" | "idle" | "running" | "error";

export interface AudioSource {
  id: number;
  nodeName: string;
  description: string;
  usbVendorId: number | null;
  usbProductId: number | null;
  alsaCard: number | null;
  state: NodeRunState | null;
  muted: boolean | null;
  volume: number | null;
  channels: number | null;
  /** Other programs recording from this input right now. */
  captureApps: string[];
}

export interface SourceRef {
  nodeName: string;
  description: string;
  isDji: boolean;
}

export type SignalStatus = "unknown" | "present" | "silent";

export interface SignalView {
  status: SignalStatus;
  peakDbfs: number | null;
  loudnessDbfs: number | null;
  loudness: string | null;
  silentForMs: number | null;
  clippedRecently: boolean;
}

export interface AudioView {
  service: AudioService;
  source: AudioSource | null;
  defaultInput: SourceRef | null;
  defaultConfigured: boolean;
  inputs: SourceRef[];
  signal: SignalView;
  micTest: MicTestState;
}

export type MicTestState =
  | { kind: "idle" }
  | { kind: "listening"; first: boolean }
  | { kind: "recording"; maxMs: number }
  | { kind: "playing"; durationMs: number }
  | { kind: "failed"; message: string };

export type MeterState =
  | { kind: "off" }
  | { kind: "starting"; nodeName: string }
  | { kind: "running"; nodeName: string; channels: number; rate: number }
  | { kind: "failed"; message: string };

export type WritePhase =
  | { kind: "rejected"; reason: string }
  | { kind: "sent" }
  | { kind: "acknowledged" }
  | { kind: "confirmed" }
  | { kind: "refused" }
  | { kind: "noAnswer" }
  | { kind: "notApplied"; reported: string | null }
  | { kind: "failed"; message: string };

export interface SettingOption {
  value: string;
  label: string;
}

/** A receiver or transmitter setting (crates/micdeck-core/src/device_settings.rs). */
export interface SettingView {
  id: string;
  group: "receiver" | "transmitter";
  label: string;
  description: string;
  options: SettingOption[];
  /** Two options off/on: show a switch. */
  switch: boolean;
  /** What the receiver reports right now. */
  value: string | null;
  /** Why it cannot be changed right now. */
  locked: string | null;
  /** MicDeck never sends this setting. */
  unsupported: boolean;
  /** Only the current value makes sense (mono with one transmitter): show it as text. */
  fixed: boolean;
  /** The value being written right now. */
  pending: string | null;
}

export interface WriteLogEntry {
  request: number;
  unixMs: number;
  setting: string;
  label: string;
  from: string | null;
  to: string;
  phase: WritePhase;
  summary: string;
  /** `null` while the change is still running. */
  ok: boolean | null;
  seq: number | null;
  packet: string | null;
}

/** MicDeck's own settings (app/src-tauri/src/settings.rs). */
export interface AppSettings {
  notifications: boolean;
  dockDictation: boolean;
  dockMute: boolean;
  autostart: boolean;
  autostartAvailable: boolean;
}

export type AppSettingName = "notifications" | "dockDictation" | "dockMute" | "autostart";

/** The user's own device pictures as data URLs (~/.config/micdeck/devices/); `null` = built-in. */
export interface DeviceImages {
  receiver: string | null;
  transmitter: string | null;
}

export interface Snapshot {
  simulated: boolean;
  demoLabel: string | null;
  verdict: Verdict;
  checks: Check[];
  receiver: ReceiverView | null;
  otherReceivers: number;
  transmitters: TransmitterView[];
  audio: AudioView;
  meter: MeterState;
  usbWatchError: string | null;
  deviceSettings: SettingView[];
  /** Setting changes, newest first. */
  deviceLog: WriteLogEntry[];
}

export interface LevelFrame {
  nodeName: string;
  peakDbfs: number[];
  rmsDbfs: number[];
  clipped: boolean;
  digitalSilence: boolean;
  frames: number;
  simulated: boolean;
}

/** Lowest level the backend reports (crates/micdeck-core/src/level.rs). */
export const FLOOR_DBFS = -120;
