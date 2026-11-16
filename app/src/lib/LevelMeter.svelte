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
    testable = false,
    side,
    onTest,
  }: {
    frame: LevelFrame | null;
    meter: MeterState;
    signal: SignalView;
    labels: string[];
    /** Name of the measured input, for the subtitle. */
    source?: string | null;
    /** Other programs recording from it right now. */
    recording?: string[];
    /** A microphone test can run (real DJI input, not demo data). */
    testable?: boolean;
    /** Right of the bars, centred on them (the mute button). */
    side?: Snippet;
    /** Opens the microphone test; `undefined` hides the button. */
    onTest?: () => void;
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
  /** Short enough to sit at the start of the bars, left of the −48 mark. */
  const SHORT: Record<string, string> = { "Normale Sprechlautstärke": "Normal" };
  const loudness = $derived.by(() => {
    if (signal.status === "present") return signal.loudness ? (SHORT[signal.loudness] ?? signal.loudness) : null;
    if (signal.status === "silent") return "Stille";
    return null;
  });

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

<section class="card meter">
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
    </div>
  </header>

  <div class="body">
    <div class="rows" class:idle={!running}>
      {#each Array.from({ length: channels }, (_, i) => i) as c (c)}
        <div class="row">
          <span class="name">{labels[c] ?? `Kanal ${c + 1}`}</span>
          <div class="track" title="Spitzenpegel, gemessen am PC">
            <div
              class="fill"
              style:clip-path="inset(0 {(1 - fraction(shown[c] ?? FLOOR_DBFS)) * 100}% 0 0 round 999px)"
            ></div>
            <div class="hold" style:left="{fraction(held[c] ?? FLOOR_DBFS) * 100}%"></div>
          </div>
          <span class="db num">{readout(held[c])}<small>dBFS</small></span>
        </div>
      {/each}
      {#if overlay}
        <div class="overlay">{overlay}</div>
      {/if}
    </div>
    {#if side}
      <div class="side">{@render side()}</div>
    {/if}
    <!-- Below the bars: how loud on the left, the scale, the average on the right. -->
    <div class="row scale-row" title="Gleitender Mittelwert der letzten Sekunden">
      <span></span>
      <div class="scale" aria-hidden="true">
        {#if loudness}
          <span class="loud" class:warn={signal.status === "silent"}>{loudness}</span>
        {/if}
        {#each TICKS as t (t)}
          <span class="tick" style:left="{fraction(t) * 100}%">{t === 0 ? "0" : String(t).replace("-", "−")}</span>
        {/each}
      </div>
      <span class="avg num">
        {#if signal.status === "present" && signal.loudnessDbfs !== null}Ø {signed(signal.loudnessDbfs)} dB{/if}
      </span>
    </div>
  </div>

  {#if onTest}
    <footer>
      <button class="btn test" disabled={!testable} onclick={onTest} title="Sprechen, anhören, nachjustieren">
        <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6" /><circle class="dot" cx="8" cy="8" r="2.6" /></svg>
        Mikrofon testen
      </button>
    </footer>
  {/if}
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
    max-width: 220px;
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

  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    column-gap: 22px;
    row-gap: 8px;
    align-items: center;
  }
  .side {
    grid-row: 1;
    grid-column: 2;
    display: grid;
    place-items: center;
  }
  .rows {
    position: relative;
    display: grid;
    gap: 16px;
  }
  .row {
    display: grid;
    grid-template-columns: 82px 1fr 100px;
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
  .scale-row {
    grid-column: 1;
  }
  .scale {
    position: relative;
    height: 16px;
    font-size: 12px;
    color: var(--text-3);
  }
  .tick {
    position: absolute;
    transform: translateX(-50%);
  }
  .loud {
    position: absolute;
    left: 0;
    font-weight: 650;
    color: var(--state);
    white-space: nowrap;
  }
  .loud.warn {
    color: var(--warn);
  }
  .avg {
    text-align: center;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--state);
    white-space: nowrap;
  }
  .idle .track {
    opacity: 0.35;
  }
  .overlay {
    position: absolute;
    inset: -4px 0;
    display: grid;
    place-items: center;
    color: var(--text-2);
    font-size: 13px;
    text-align: center;
    background: color-mix(in srgb, var(--surface) 60%, transparent);
    border-radius: var(--radius-sm);
  }
  /* Bottom right, centred under the dB readouts and the mute button
     (100 px column + 22 px gap + 86 px button). */
  footer {
    margin-top: auto;
    padding-top: 18px;
    display: flex;
    justify-content: flex-end;
  }
  .test {
    width: 208px;
    justify-content: center;
  }
  .test svg .dot {
    fill: var(--error);
    stroke: none;
  }
</style>
