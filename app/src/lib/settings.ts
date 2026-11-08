import type { SettingView, WriteLogEntry } from "./types";

/** How long a failed change stays explained next to the setting. */
const ERROR_MS = 8000;

/** Why the last change of `id` failed, while that is recent. Success needs
 *  no message: the switch simply stays where you put it. */
export function errorFor(log: WriteLogEntry[], id: string, nowMs: number): string | null {
  const entry = log.find((e) => e.setting === id);
  if (!entry || entry.ok !== false || nowMs - entry.unixMs > ERROR_MS) return null;
  return entry.summary;
}

/** MicDeck sends one change at a time. */
export function anyPending(settings: SettingView[]): boolean {
  return settings.some((s) => s.pending !== null);
}

export function byId(settings: SettingView[], id: string): SettingView | undefined {
  return settings.find((s) => s.id === id);
}
