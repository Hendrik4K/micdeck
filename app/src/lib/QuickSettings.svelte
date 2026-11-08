<script lang="ts">
  import type { SettingView, WriteLogEntry } from "./types";
  import SettingRow from "./SettingRow.svelte";
  import { anyPending, byId, errorFor } from "./settings";

  let {
    settings,
    log,
    now,
    onchange,
    onmore,
  }: {
    settings: SettingView[];
    log: WriteLogEntry[];
    now: number;
    onchange: (id: string, value: string) => void;
    onmore: () => void;
  } = $props();

  // The ones DJI's app puts on its start page.
  const QUICK = ["stereo", "noise-cancel-power", "noise-cancel", "low-cut", "safety-track"];
  // A fixed value (mono with one transmitter) is nothing to set: leave it out here.
  const rows = $derived(
    QUICK.map((id) => byId(settings, id)).filter((s): s is SettingView => s !== undefined && !s.fixed),
  );
  const busy = $derived(anyPending(settings));
  /** One reason for everything, e.g. no receiver: say it once. */
  const common = $derived.by(() => {
    const reasons = new Set(rows.map((r) => r.locked));
    return reasons.size === 1 ? [...reasons][0] : null;
  });
</script>

<section class="card quick">
  <header class="card-head">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 8.6a3.4 3.4 0 1 0 0 6.8 3.4 3.4 0 0 0 0-6.8z" />
      <path d="M19.4 13.5a7.6 7.6 0 0 0 0-3l2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2.6-1.5L14 2.5h-4l-.4 2.5A7.6 7.6 0 0 0 7 6.5l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0 0 3l-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2.6 1.5l.4 2.5h4l.4-2.5a7.6 7.6 0 0 0 2.6-1.5l2.4 1 2-3.4z" />
    </svg>
    <div class="grow"><h2>RX-Einstellungen</h2></div>
    <button class="link" onclick={onmore}>Alle Einstellungen ›</button>
  </header>
  {#if common}
    <p class="common">{common}</p>
  {/if}
  <div class="rows">
    {#each rows as setting (setting.id)}
      <SettingRow
        {setting}
        {busy}
        compact
        hideReason={common !== null}
        error={errorFor(log, setting.id, now)}
        onchange={(value) => onchange(setting.id, value)}
      />
    {/each}
  </div>
</section>

<style>
  .common {
    margin: -6px 0 6px;
    font-size: 13px;
    color: var(--text-3);
  }
</style>
