import type { AppSettingName, AppSettings, DeviceImages, LevelFrame, Snapshot } from "./types";

export interface Backend {
  initial(): Promise<Snapshot>;
  onSnapshot(callback: (s: Snapshot) => void): Promise<() => void>;
  onLevel(callback: (frame: LevelFrame) => void): Promise<() => void>;
  /** Tell the backend whether a level meter is visible; it only measures then. */
  setMeterWanted(wanted: boolean): Promise<void>;
  /** Mute or unmute the DJI input (in PipeWire and on the receiver itself). */
  setMuted(muted: boolean): Promise<void>;
  /** Record 5 s into memory, play them back once, drop them. */
  startMicTest(): Promise<void>;
  stopMicTest(): Promise<void>;
  /** Change a receiver or transmitter setting; progress shows up in snapshots. */
  setDeviceSetting(id: string, value: string): Promise<void>;
  appSettings(): Promise<AppSettings>;
  setAppSetting(setting: AppSettingName, on: boolean): Promise<AppSettings>;
  /** Changes made elsewhere, e.g. in the panel menu. */
  onAppSettings(callback: (s: AppSettings) => void): Promise<() => void>;
  /** Page asked for on the command line (`--page=device`). */
  startPage(): Promise<string | null>;
  /** The user's own device pictures, if any. */
  deviceImages(): Promise<DeviceImages>;
}

const insideTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function tauriBackend(): Promise<Backend> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  return {
    initial: () => invoke<Snapshot>("get_snapshot"),
    onSnapshot: (cb) => listen<Snapshot>("micdeck:snapshot", (e) => cb(e.payload)),
    onLevel: (cb) => listen<LevelFrame>("micdeck:level", (e) => cb(e.payload)),
    setMeterWanted: (wanted) => invoke("set_meter_wanted", { wanted }),
    setMuted: (muted) => invoke("set_muted", { muted }),
    startMicTest: () => invoke("start_mic_test"),
    stopMicTest: () => invoke("stop_mic_test"),
    setDeviceSetting: (id, value) => invoke("set_device_setting", { id, value }),
    appSettings: () => invoke<AppSettings>("get_app_settings"),
    setAppSetting: (setting, on) => invoke<AppSettings>("set_app_setting", { setting, on }),
    onAppSettings: (cb) => listen<AppSettings>("micdeck:app-settings", (e) => cb(e.payload)),
    startPage: () => invoke<string | null>("start_page"),
    deviceImages: () => invoke<DeviceImages>("device_images"),
  };
}

export async function connect(): Promise<Backend> {
  if (insideTauri) return tauriBackend();
  // Opening the dev server in a normal browser shows a clearly labelled preview.
  if (import.meta.env.DEV) {
    const { previewBackend } = await import("./preview");
    return previewBackend();
  }
  throw new Error("MicDeck läuft nur als Desktop-App.");
}
