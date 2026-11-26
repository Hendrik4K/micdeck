import type { AppSettingName, AppSettings, DeviceImages, LevelFrame, Snapshot } from "./types";

export interface Backend {
  initial(): Promise<Snapshot>;
  onSnapshot(callback: (s: Snapshot) => void): Promise<() => void>;
  onLevel(callback: (frame: LevelFrame) => void): Promise<() => void>;
  /** Tell the backend whether a level meter is visible; it only measures then. */
  setMeterWanted(wanted: boolean): Promise<void>;
  /** Makes the window as tall as the page (logical px), width unchanged. */
  fitHeight(height: number): Promise<void>;
  /** Mute or unmute the DJI input (in PipeWire and on the receiver itself). */
  setMuted(muted: boolean): Promise<void>;
  /** Record 5 s into memory, play them back once, drop them. */
  startMicTest(): Promise<void>;
  stopMicTest(): Promise<void>;
  /** Change a receiver or transmitter setting; progress shows up in snapshots. */
  setDeviceSetting(id: string, value: string): Promise<void>;
  /** Restart the receiver (about 6 s without sound); snapshots show it. */
  restartReceiver(): Promise<void>;
  /** Use this jack input (receiver on its 3.5 mm output); remembered. */
  setJackInput(nodeName: string): Promise<void>;
  appSettings(): Promise<AppSettings>;
  setAppSetting(setting: AppSettingName, on: boolean): Promise<AppSettings>;
  /** Changes made elsewhere, e.g. in the panel menu. */
  onAppSettings(callback: (s: AppSettings) => void): Promise<() => void>;
  /** Page asked for on the command line (`--page=device`). */
  startPage(): Promise<string | null>;
  /** The user's own device pictures, if any. */
  deviceImages(): Promise<DeviceImages>;
  /** Copies a picked picture into MicDeck's own folder; the new set. */
  setDeviceImage(device: DeviceName, file: File): Promise<DeviceImages>;
  /** Back to the built-in picture; the new set. */
  resetDeviceImage(device: DeviceName): Promise<DeviceImages>;
}

export type DeviceName = "receiver" | "transmitter";

const insideTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function tauriBackend(): Promise<Backend> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  return {
    initial: () => invoke<Snapshot>("get_snapshot"),
    onSnapshot: (cb) => listen<Snapshot>("micdeck:snapshot", (e) => cb(e.payload)),
    onLevel: (cb) => listen<LevelFrame>("micdeck:level", (e) => cb(e.payload)),
    setMeterWanted: (wanted) => invoke("set_meter_wanted", { wanted }),
    fitHeight: (height) => invoke("fit_height", { height }),
    setMuted: (muted) => invoke("set_muted", { muted }),
    startMicTest: () => invoke("start_mic_test"),
    stopMicTest: () => invoke("stop_mic_test"),
    setDeviceSetting: (id, value) => invoke("set_device_setting", { id, value }),
    setJackInput: (nodeName) => invoke("set_jack_input", { nodeName }),
    restartReceiver: () => invoke("restart_receiver"),
    appSettings: () => invoke<AppSettings>("get_app_settings"),
    setAppSetting: (setting, on) => invoke<AppSettings>("set_app_setting", { setting, on }),
    onAppSettings: (cb) => listen<AppSettings>("micdeck:app-settings", (e) => cb(e.payload)),
    startPage: () => invoke<string | null>("start_page"),
    deviceImages: () => invoke<DeviceImages>("device_images"),
    setDeviceImage: async (device, file) =>
      invoke<DeviceImages>("set_device_image", new Uint8Array(await file.arrayBuffer()), { headers: { device } }),
    resetDeviceImage: (device) => invoke<DeviceImages>("reset_device_image", { device }),
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
