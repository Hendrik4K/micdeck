<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { FLOOR_DBFS, type LevelFrame, type MeterState, type SignalView } from "./types";

  let {
    frame,
    meter,
    signal,
    labels,
    source = null,
    recording = [],
    muted = null,
    large = false,
    onToggleMute,
    children,
  }: {
    frame: LevelFrame | null;
    meter: MeterState;
    signal: SignalView;
    labels: string[];
    /** Name of the measured input, for the subtitle. */
    source?: string | null;
    /** Other programs recording from it right now. */
    recording?: string[];
    /** Mute state of the DJI input; `null` when there is no input to mute. */
    muted?: boolean | null;
    /** The audio page: taller bars. */
    large?: boolean;
    onToggleMute?: () => void;
    children?: Snippet;
  } = $props();

  const MIN_DB = -60;
  const FALL_DB_PER_S = 24;
  const HOLD_MS = 1500;
  const STALE_MS = 300;
  const TICKS = [-48, -36, -24, -18, -12, -6, 0];

  let shown = $state<number[]>([]);
  let held = $state<number[]>([]);

  let latest: LevelFrame | null = null;
  let receivedAt = 0;
  let heldAt: number[] = [];

  $effect(() => {
    if (frame) {
      latest = frame;
      receivedAt = performance.now();
    }
  });

  const running = $derived(meter.kind === "running");
  const channels = $derived(meter.kind === "running" ? Math.max(1, meter.channels) : labels.length || 2);

  onMount(() => {
    let raf = 0;
    let last = performance.now();
    const step = (now: number) => {
      const dt = (now - last) / 1000;
      last = now;
      const fresh = latest && now - receivedAt < STALE_MS;
      const nextShown: number[] = [];
      const nextHeld: number[] = [];
      for (let c = 0; c < channels; c++) {
        const target = fresh ? (latest!.peakDbfs[c] ?? FLOOR_DBFS) : FLOOR_DBFS;
        const previous = shown[c] ?? FLOOR_DBFS;
        const value = Math.max(target, previous - FALL_DB_PER_S * dt);
        nextShown.push(value);
        const h = held[c] ?? FLOOR_DBFS;
        if (target >= h || now - (heldAt[c] ?? 0) > HOLD_MS) {
          nextHeld.push(target);
          heldAt[c] = now;
        } else {
          nextHeld.push(h);
        }
      }
      shown = nextShown;
      held = nextHeld;
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  const fraction = (db: number) => Math.min(1, Math.max(0, (db - MIN_DB) / -MIN_DB));
  const readout = (db: number | undefined) =>
    db === undefined || db <= MIN_DB ? "−∞" : db.toFixed(1).replace("-", "−");
  const signed = (db: number) => (db < 0 ? `−${-db}` : `${db}`);

  const overlay = $derived.by(() => {
    switch (meter.kind) {
      case "off":
        return "Pegelmessung aus – wartet auf den DJI-Audioeingang";
      case "starting":
        return "Messung startet …";
      case "failed":
        return `Messung fehlgeschlagen: ${meter.message}`;
      default:
        return null;
    }
  });
</script>

<section class="card meter" class:large>
  <header class="card-head">
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12h1.5M6.5 8.5v7M10 5v14M13.5 8v8M17 10v4M20.5 12H21" /></svg>
    <div class="grow">
      <h2>Audio-Monitor</h2>
      <p>{source ? `${source} – Live-Pegel` : "Live-Pegel des DJI-Eingangs"}</p>
    </div>
    <div class="flags">
      {#if recording.length}
        <span class="rec" title="Diese Programme nehmen gerade vom DJI-Mikrofon auf">
          <i></i>{recording.join(", ")} nimmt auf
        </span>
      {/if}
      {#if frame?.simulated}<span class="sim">Simuliert</span>{/if}
      {#if muted !== null}
        <button
          class="mute"
          class:on={muted}
          onclick={onToggleMute}
          title={muted
            ? "Stummschaltung aufheben"
            : "Stummschalten – auch am Empfänger selbst, kein Programm bekommt dann Ton"}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <rect x="5.5" y="1.5" width="5" height="8" rx="2.5" />
            <path d="M3.5 7.5a4.5 4.5 0 0 0 9 0M8 12v2.5" />
            {#if muted}<path class="slash" d="M2 2l12 12" />{/if}
          </svg>
          {muted ? "Stumm" : "Mikro an"}
        </button>
      {/if}
    </div>
  </header>

  <div class="rows" class:idle={!running}>
    {#each Array.from({ length: channels }, (_, i) => i) as c (c)}
      <div class="row">
        <span class="name">{labels[c] ?? `Kanal ${c + 1}`}</span>
        <div class="track">
          <div
            class="fill"
            style:clip-path="inset(0 {(1 - fraction(shown[c] ?? FLOOR_DBFS)) * 100}% 0 0 round 999px)"
          ></div>
          <div class="hold" style:left="{fraction(held[c] ?? FLOOR_DBFS) * 100}%"></div>
        </div>
        <span class="db num">{readout(held[c])}<small>dBFS</small></span>
      </div>
    {/each}
    <div class="scale" aria-hidden="true">
      {#each TICKS as t (t)}
        <span style:left="{fraction(t) * 100}%">{t === 0 ? "0" : String(t).replace("-", "−")}</span>
      {/each}
    </div>
    {#if overlay}
      <div class="overlay">{overlay}</div>
    {/if}
  </div>

  <footer>
    {#if signal.status === "present" && signal.loudness}
      <span class="state ok" title="Gleitender Mittelwert der letzten Sekunden">
        {signal.loudness}{signal.loudnessDbfs !== null ? ` · Ø ${signed(signal.loudnessDbfs)} dB` : ""}
      </span>
    {:else if signal.status === "present"}
      <span class="state ok">Ton kommt an</span>
    {:else if signal.status === "silent"}
      <span class="state warn">Nur digitale Stille</span>
    {:else}
      <span class="state">Kein Messwert</span>
    {/if}
    <span class="unit">Spitzenpegel · gemessen am PC</span>
  </footer>
  {@render children?.()}
</section>

<style>
  .meter {
    display: flex;
    flex-direction: column;
  }
  .flags {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .sim {
    font-size: 11px;
    font-weight: 600;
    color: var(--warn);
    border: 1px solid color-mix(in srgb, var(--warn) 50%, transparent);
    border-radius: 6px;
    padding: 1px 6px;
  }
  .rec {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 180px;
    font-size: 12px;
    font-weight: 600;
    color: var(--error);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rec i {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--error);
    animation: blink 1.4s ease-in-out infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.3;
    }
  }
  .mute {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    padding: 4px 11px 4px 9px;
    border-radius: 8px;
    border: 1px solid var(--line-2);
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    transition: background 120ms, color 120ms, border-color 120ms;
  }
  .mute:hover {
    color: var(--text);
    border-color: var(--text-3);
  }
  .mute.on {
    color: #1b1503;
    background: var(--warn);
    border-color: var(--warn);
  }
  .mute svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .rows {
    position: relative;
    display: grid;
    gap: 16px;
    padding-bottom: 24px;
  }
  .row {
    display: grid;
    grid-template-columns: 96px 1fr 104px;
    gap: 14px;
    align-items: center;
  }
  .name {
    color: var(--text);
    font-size: 15px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track {
    position: relative;
    height: 22px;
    border-radius: 999px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .large .track {
    height: 30px;
  }
  .fill {
    position: absolute;
    inset: 0;
    border-radius: 999px;
    /* Fixed to the dB scale (−60…0 dBFS) and revealed by clip-path, so the
       colours stay put: green up to −18, yellow to −6, red above. */
    background: linear-gradient(
      90deg,
      var(--accent-deep) 0%,
      var(--accent) 66%,
      var(--warn) 72%,
      var(--warn) 88%,
      var(--error) 92%
    );
    will-change: clip-path;
  }
  .hold {
    position: absolute;
    top: 3px;
    bottom: 3px;
    width: 3px;
    margin-left: -1.5px;
    border-radius: 2px;
    background: #e9fff2;
    box-shadow: 0 0 6px rgba(47, 227, 138, 0.8);
  }
  .db {
    text-align: right;
    font-size: 18px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
  }
  .db small {
    margin-left: 5px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-2);
  }
  .scale {
    position: absolute;
    left: 110px;
    right: 118px;
    bottom: 0;
    height: 16px;
  }
  .scale span {
    position: absolute;
    transform: translateX(-50%);
    font-size: 12px;
    color: var(--text-3);
  }
  .idle .track {
    opacity: 0.35;
  }
  .overlay {
    position: absolute;
    inset: 0 0 22px;
    display: grid;
    place-items: center;
    color: var(--text-2);
    font-size: 13px;
    text-align: center;
    background: color-mix(in srgb, var(--surface) 60%, transparent);
    border-radius: var(--radius-sm);
  }
  footer {
    margin-top: 12px;
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .state.ok {
    color: var(--ok);
  }
  .state.warn {
    color: var(--warn);
  }
</style>
