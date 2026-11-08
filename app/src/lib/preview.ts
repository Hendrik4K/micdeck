// Browser-only preview for UI work (`pnpm dev` opened in a normal browser).
// Never used inside the app; every value here is fake and labelled as such.
// Pick a state with the URL hash: #ready (default), #no-tx, #unplugged, #no-permission.

import type { Backend } from "./api";
import type { AppSettings, Check, LevelFrame, SettingView, Snapshot, TransmitterView, WriteLogEntry } from "./types";

const NODE = "preview.dji-mic-mini.input";

type Spec = Omit<SettingView, "switch" | "value" | "locked" | "unsupported" | "fixed" | "pending"> & {
  locked: string | null;
  needsTx: boolean;
};

// Mirrors SPECS in crates/micdeck-core/src/device_settings.rs (preview only).
const SPECS: Spec[] = [
  { id: "noise-cancel-power", group: "receiver", label: "Rauschminderung", description: "Filtert Umgebungsgeräusche heraus. Ein kurzer Druck auf die Sendertaste schaltet sie ebenfalls um.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: false },
  { id: "noise-cancel", group: "receiver", label: "Geräuschunterdrückung", description: "Einfach bewahrt die Klangtreue in ruhigen Räumen, Stark filtert mehr Lärm, etwa draußen.",
    options: [{ value: "basic", label: "Einfach" }, { value: "strong", label: "Stark" }], locked: null, needsTx: false },
  { id: "stereo", group: "receiver", label: "Audio-Kanal", description: "Stereo legt Sender 1 auf links und Sender 2 auf rechts.",
    options: [{ value: "mono", label: "Mono" }, { value: "stereo", label: "Stereo" }], locked: null, needsTx: false },
  { id: "safety-track", group: "receiver", label: "Backup-Aufnahme", description: "Nimmt eine zweite, leisere Spur als Reserve auf. Die Spuren trennst du später beim Schnitt.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: false },
  { id: "low-cut", group: "receiver", label: "Hochpassfilter", description: "Senkt tiefe Störgeräusche wie Wind, Trittschall und Brummen ab.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: true },
  { id: "clip-limiter", group: "receiver", label: "Clipping-Kontrolle", description: "Nimmt die Lautstärke automatisch zurück, bevor das Signal übersteuert.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: false },
  { id: "auto-off-15m", group: "receiver", label: "Automatische Abschaltung", description: "Schaltet den Empfänger aus, wenn er 15 Minuten ohne Verbindung und unbenutzt ist.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: false },
  { id: "camera-power", group: "receiver", label: "Empfänger An/Aus mit Kamera", description: "Schaltet den Empfänger zusammen mit der Kamera ein und aus.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: false },
  { id: "plug-free", group: "receiver", label: "Externer Lautsprecher ohne Stecker", description: "Tonausgabe über den Adapter für Mobilgeräte am Erweiterungsanschluss.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: "Das Umschalten startet den Empfänger neu und unterbricht den Ton am PC. Bitte in der DJI-App ändern.", needsTx: false },
  { id: "tx-auto-off-15m", group: "transmitter", label: "Automatische Abschaltung", description: "Schaltet den Sender aus, wenn er 15 Minuten nicht verbunden ist, nicht aufnimmt und nicht bedient wird.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: true },
  { id: "noise-cancel-button", group: "transmitter", label: "Einschalttaste für Rauschminderung", description: "Ein kurzer Druck auf die Sendertaste schaltet die Rauschminderung ein oder aus.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: true },
  { id: "mic-leds", group: "transmitter", label: "Mikrofon-LED", description: "Statusleuchte am Sender.",
    options: [{ value: "off", label: "Aus" }, { value: "on", label: "An" }], locked: null, needsTx: true },
  { id: "voice-tone", group: "transmitter", label: "Klangfarbe", description: "Standard, voller oder heller Klang.",
    options: [{ value: "standard", label: "Standard" }, { value: "rich", label: "Voll" }, { value: "bright", label: "Hell" }], locked: "Gibt es nur beim DJI Mic Mini 2.", needsTx: true },
];

/** What the simulated receiver reports. */
const values: Record<string, string> = {
  "noise-cancel-power": "on", "noise-cancel": "basic", stereo: "mono", "safety-track": "off",
  "low-cut": "on", "clip-limiter": "on", "auto-off-15m": "on", "camera-power": "off", "plug-free": "off",
  "tx-auto-off-15m": "on", "noise-cancel-button": "on", "mic-leds": "on", "voice-tone": "standard",
};
const log: WriteLogEntry[] = [];
let pending: { id: string; value: string } | null = null;
let appSettings: AppSettings = {
  notifications: true, dockDictation: true, dockMute: false, autostart: false, autostartAvailable: true,
};

function settingViews(kind: string): SettingView[] {
  const live = kind === "ready" || kind === "no-tx";
  return SPECS.map((spec) => {
    const value = !live || (spec.needsTx && kind === "no-tx") ? null : values[spec.id];
    let locked = spec.locked;
    if (!locked && kind === "unplugged") locked = "Kein Empfänger angeschlossen.";
    else if (!locked && !live) locked = "Keine Statusdaten vom Empfänger.";
    else if (!locked && value === null) locked = "Erst einen Sender einschalten, dann lässt sich das einstellen.";
    else if (!locked && spec.id === "noise-cancel" && values["noise-cancel-power"] === "off") locked = "Erst die Rauschminderung einschalten.";
    else if (!locked && spec.id === "safety-track" && values.stereo === "stereo") locked = "Nur im Mono-Modus verfügbar.";
    else if (!locked && spec.id === "stereo" && values["safety-track"] === "on") locked = "Erst die Backup-Aufnahme ausschalten.";
    return {
      id: spec.id, group: spec.group, label: spec.label, description: spec.description, options: spec.options,
      switch: spec.options.length === 2 && spec.options[0].value === "off" && spec.options[1].value === "on",
      value, locked, unsupported: spec.locked !== null,
      fixed: spec.id === "stereo" && kind === "no-tx" && values.stereo === "mono",
      pending: pending?.id === spec.id ? pending.value : null,
    };
  });
}

function checks(kind: string): Check[] {
  const c = (id: Check["id"], level: Check["level"], label: string, value: string, hint: string | null = null): Check =>
    ({ id, level, label, value, hint });
  if (kind === "unplugged") {
    return [
      c("usb", "error", "Empfänger (USB)", "Nicht angeschlossen", "Empfänger mit einem USB-C-Datenkabel an den PC anschließen."),
      c("radio", "inactive", "Funkverbindung", "–"),
      c("audioInput", "inactive", "Audioeingang", "–"),
      c("mute", "inactive", "Lautstärke", "–"),
      c("defaultInput", "inactive", "Standard-Mikrofon", "–"),
      c("signal", "inactive", "Ton kommt an", "–"),
    ];
  }
  return [
    c("usb", "ok", "Empfänger (USB)", "DJI Mic Mini angeschlossen"),
    kind === "no-tx"
      ? c("radio", "warn", "Funkverbindung", "Kein Sender verbunden", "Sender einschalten und Kopplung prüfen.")
      : kind === "no-permission"
        ? c("radio", "warn", "Funkverbindung", "Keine Berechtigung für Statusdaten", "Einmalig eine udev-Regel einrichten.")
        : c("radio", "ok", "Funkverbindung", "Beide Sender verbunden"),
    c("audioInput", "ok", "Audioeingang", "„DJI Mic Mini (Vorschau)“"),
    c("mute", "ok", "Lautstärke", "100\u00a0% · nicht stumm"),
    c("defaultInput", "ok", "Standard-Mikrofon", "Ist Standard-Mikrofon"),
    kind === "no-tx"
      ? c("signal", "warn", "Ton kommt an", "Nur Stille", "Es ist kein Sender verbunden.")
      : c("signal", "ok", "Ton kommt an", "Normale Sprechlautstärke · Ø\u00a0−30\u00a0dB"),
  ];
}

function snapshot(kind: string): Snapshot {
  const tx = (unit: number, gauge: number): TransmitterView => ({
    unit,
    connected: kind === "no-permission" || kind === "unplugged" ? null : kind !== "no-tx",
    name: kind === "no-tx" || kind === "no-permission" ? null : "DJI Mic Mini",
    serial: null,
    firmware: kind === "no-tx" ? null : "02.03.17.00",
    battery: kind === "no-tx" || kind === "no-permission"
      ? null
      : { gauge, bars: 7 - gauge, label: gauge <= 2 ? "hoch" : "mittel", charging: false, low: false, critical: false },
    charging: null,
    radioLevel: kind === "no-tx" ? null : 0x30,
  });
  const verdict =
    kind === "unplugged"
      ? { level: "error" as const, title: "Nicht verbunden", detail: "Kein DJI-Empfänger am USB gefunden." }
      : kind === "no-tx"
        ? { level: "warn" as const, title: "Verbunden – kein Sender aktiv", detail: "Der Empfänger ist bereit, aber kein Sender ist eingeschaltet." }
        : { level: "ok" as const, title: "Einsatzbereit", detail: "DJI Mic Mini · Beide Sender verbunden · Ton kommt an" };
  const present = kind !== "unplugged";
  return {
    simulated: true,
    demoLabel: "Browser-Vorschau",
    verdict,
    checks: checks(kind),
    receiver: present
      ? {
          key: "preview",
          name: "DJI Mic Mini",
          usbId: "2ca3:4011",
          serial: null,
          firmware: kind === "no-permission" ? null : "02.03.17.00",
          protocolVersion: kind === "no-permission" ? null : 2,
          link: kind === "no-permission" ? { kind: "permissionDenied" } : { kind: "open" },
          telemetryLive: kind !== "no-permission",
          gainDialDb: kind === "no-permission" ? null : 0,
          settings: {},
          alsaCards: [5],
        }
      : null,
    otherReceivers: 0,
    transmitters: [tx(1, 2), tx(2, 4)],
    audio: {
      service: { kind: "available" },
      source: present
        ? {
            id: 1, nodeName: NODE, description: "DJI Mic Mini (Vorschau)", usbVendorId: 0x2ca3, usbProductId: 0x4011,
            alsaCard: 5, state: "running", muted: false, volume: 1, channels: 2, captureApps: ["Firefox"],
          }
        : null,
      defaultInput: present ? { nodeName: NODE, description: "DJI Mic Mini (Vorschau)", isDji: true } : null,
      defaultConfigured: true,
      inputs: present ? [{ nodeName: NODE, description: "DJI Mic Mini (Vorschau)", isDji: true }] : [],
      signal: {
        status: !present ? "unknown" : kind === "no-tx" ? "silent" : "present",
        peakDbfs: present ? -14 : null,
        loudnessDbfs: present ? -30 : null,
        loudness: present ? "Normale Sprechlautstärke" : null,
        silentForMs: null,
        clippedRecently: false,
      },
      micTest: { kind: "idle" },
    },
    meter: present ? { kind: "running", nodeName: NODE, channels: 2, rate: 48000 } : { kind: "off" },
    usbWatchError: null,
    deviceSettings: settingViews(kind),
    deviceLog: [...log],
  };
}

function level(t: number, silent: boolean): LevelFrame {
  const ch = [0, 1].map((c) => {
    const syllable = Math.floor(t / 180) + c * 7;
    const speaking = (syllable * 2654435761) % 5 !== 0;
    const env = Math.sin(((t % 180) / 180) * Math.PI);
    const amp = 0.0015 + (speaking ? (0.1 + ((syllable * 40503) % 100) / 400) * env : 0);
    return 20 * Math.log10(amp);
  });
  return {
    nodeName: NODE,
    peakDbfs: silent ? [-120, -120] : ch,
    rmsDbfs: silent ? [-120, -120] : ch.map((p) => p - 9),
    clipped: false,
    digitalSilence: silent,
    frames: 1600,
    simulated: true,
  };
}

export function previewBackend(): Backend {
  const kind = () => location.hash.slice(1) || "ready";
  const listeners = new Set<() => void>();
  const changed = () => listeners.forEach((l) => l());
  const label = (id: string, v: string | undefined) =>
    SPECS.find((s) => s.id === id)?.options.find((o) => o.value === v)?.label ?? v ?? null;
  return {
    initial: async () => snapshot(kind()),
    onSnapshot: async (cb) => {
      const handler = () => cb(snapshot(kind()));
      listeners.add(handler);
      window.addEventListener("hashchange", handler);
      return () => {
        listeners.delete(handler);
        window.removeEventListener("hashchange", handler);
      };
    },
    onLevel: async (cb) => {
      const start = performance.now();
      const timer = window.setInterval(() => {
        if (kind() !== "unplugged") cb(level(performance.now() - start, kind() === "no-tx"));
      }, 33);
      return () => window.clearInterval(timer);
    },
    setMeterWanted: async () => {},
    setMuted: async () => {},
    startMicTest: async () => {},
    stopMicTest: async () => {},
    // Simulates a receiver that acknowledges and applies every change.
    setDeviceSetting: async (id, value) => {
      if (pending) return;
      const entry: WriteLogEntry = {
        request: log.length + 1, unixMs: Date.now(), setting: id,
        label: SPECS.find((s) => s.id === id)?.label ?? id, from: label(id, values[id]), to: label(id, value) ?? value,
        phase: { kind: "sent" }, summary: "Gesendet, wartet auf Bestätigung …", ok: null, seq: log.length + 1,
        packet: "55 16 04 fc 02 5a … (Vorschau)",
      };
      pending = { id, value };
      log.unshift(entry);
      changed();
      window.setTimeout(() => {
        values[id] = value;
        pending = null;
        log[log.indexOf(entry)] = { ...entry, phase: { kind: "confirmed" }, summary: "Übernommen und zurückgelesen", ok: true };
        changed();
      }, 700);
    },
    appSettings: async () => appSettings,
    setAppSetting: async (setting, on) => {
      appSettings = { ...appSettings, [setting]: on };
      return appSettings;
    },
    onAppSettings: async () => () => {},
    startPage: async () => null,
    deviceImages: async () => ({ receiver: null, transmitter: null }),
  };
}
