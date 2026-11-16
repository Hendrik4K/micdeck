<script lang="ts">
  import type { SettingView, WriteLogEntry } from "./types";
  import SettingRow from "./SettingRow.svelte";
  import Segmented from "./ui/Segmented.svelte";
  import { slide } from "svelte/transition";
  import { anyPending, byId, errorFor } from "./settings";

  let {
    settings,
    log,
    now,
    onchange,
    onmore,
    testable = false,
    ontest,
  }: {
    settings: SettingView[];
    log: WriteLogEntry[];
    now: number;
    onchange: (id: string, value: string) => void;
    onmore: () => void;
    /** A microphone test can run (real DJI input, not demo data). */
    testable?: boolean;
    /** Opens the microphone test in this panel's place. */
    ontest?: () => void;
  } = $props();

  // The start page keeps the two noise filters; everything else is under
  // "Mehr Einstellungen".
  const power = $derived(byId(settings, "noise-cancel-power"));
  const level = $derived(byId(settings, "noise-cancel"));
  const busy = $derived(anyPending(settings));
  const powerOn = $derived((power?.pending ?? power?.value) === "on");
  /** One reason for both, e.g. no receiver: say it once. */
  const common = $derived(power?.locked && level?.locked === power.locked ? power.locked : null);
  /** Noise suppression waits for noise reduction; a click on it switches that on. */
  const canWake = $derived(power !== undefined && power.locked === null && !powerOn);
  const nlError = $derived(level ? errorFor(log, level.id, now) : null);

  function wake() {
    if (canWake && !busy) onchange("noise-cancel-power", "on");
  }
</script>

<header class="card-head">
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M12 8.6a3.4 3.4 0 1 0 0 6.8 3.4 3.4 0 0 0 0-6.8z" />
    <path d="M19.4 13.5a7.6 7.6 0 0 0 0-3l2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2.6-1.5L14 2.5h-4l-.4 2.5A7.6 7.6 0 0 0 7 6.5l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0 0 3l-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2.6 1.5l.4 2.5h4l.4-2.5a7.6 7.6 0 0 0 2.6-1.5l2.4 1 2-3.4z" />
  </svg>
  <div class="grow"><h2>Einstellungen</h2></div>
</header>

{#if common}
  <p class="common">{common}</p>
{/if}

<div class="rows">
  {#if power}
    <SettingRow
      setting={power}
      {busy}
      compact
      hideReason={common !== null}
      error={errorFor(log, power.id, now)}
      onchange={(value) => onchange(power.id, value)}
    />
  {/if}

  {#if level}
    {#if powerOn}
      <div class="nc">
        <span class="label">{level.label}</span>
        <div transition:slide={{ duration: 220 }}>
          <Segmented
            options={level.options}
            value={level.pending ?? level.value}
            disabled={level.locked !== null}
            tone="nc"
            stretch
            label={level.label}
            onchange={(value) => !busy && onchange(level.id, value)}
          />
        </div>
        {#if nlError}<span class="err">{nlError}</span>{/if}
      </div>
    {:else}
      <button class="nc wake" disabled={!canWake} onclick={wake} title={canWake ? "Schaltet die Rauschminderung ein" : undefined}>
        <span class="label">{level.label}</span>
        {#if common === null && level.locked}<span class="why">{level.locked}</span>{/if}
      </button>
    {/if}
  {/if}
</div>

<footer>
  {#if ontest}
    <button class="test" disabled={!testable} onclick={ontest} title="Sprechen, anhören, nachjustieren">
      <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6" /><circle class="dot" cx="8" cy="8" r="2.6" /></svg>
      Mikrofon testen
    </button>
  {/if}
  <button class="link more" onclick={onmore}>Mehr Einstellungen ›</button>
</footer>

<style>
  .common {
    margin: -6px 0 6px;
    font-size: 13px;
    color: var(--text-3);
  }
  .rows {
    flex: 1;
  }
  .nc {
    display: grid;
    gap: 10px;
    padding: 12px 0 4px;
    border-top: 1px solid var(--line);
  }
  .wake {
    width: 100%;
    gap: 2px;
    text-align: left;
    font: inherit;
    color: inherit;
    background: none;
    border: 0;
    border-top: 1px solid var(--line);
    padding: 12px 0;
    cursor: pointer;
    border-radius: 0;
  }
  .wake:disabled {
    cursor: default;
  }
  .label {
    font-size: 15.5px;
    font-weight: 600;
  }
  .wake .label {
    color: var(--text-2);
  }
  .why {
    font-size: 12px;
    color: var(--text-3);
  }
  .wake:hover:not(:disabled) .label {
    color: var(--text);
  }
  .wake:hover:not(:disabled) .why {
    color: var(--nc);
  }
  .err {
    font-size: 12px;
    font-weight: 600;
    color: var(--error);
  }
  footer {
    margin-top: 10px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .more {
    margin: 0 -6px 0 auto;
  }
  .test {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    padding: 6px 12px 6px 9px;
    border-radius: 9px;
    border: 1.5px solid var(--line-2);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: border-color 120ms, background 120ms;
  }
  .test:hover:not(:disabled) {
    border-color: var(--text-2);
    background: rgba(255, 255, 255, 0.03);
  }
  .test:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .test svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
  }
  .test .dot {
    fill: var(--error);
    stroke: none;
  }
</style>
