<script lang="ts">
  import type { MicTestState } from "./types";

  let {
    test,
    onstop,
  }: {
    test: MicTestState;
    /** Ends the session and returns to the settings. */
    onstop: () => void;
  } = $props();

  const running = $derived(test.kind === "listening" || test.kind === "recording" || test.kind === "playing");

  // Phase timing is local: the backend only reports phase changes.
  let phaseStart = $state(0);
  let now = $state(0);
  let lastKind = "";
  let lastMaxMs = 15000;
  /** How full the ring was when recording ended; playback drains it from there. */
  let recorded = $state(0);

  $effect(() => {
    const kind = test.kind;
    if (kind === lastKind) return;
    // Measured here: by now `test` already reports playback.
    if (lastKind === "recording" && kind === "playing") {
      recorded = Math.min(1, (performance.now() - phaseStart) / lastMaxMs);
    }
    if (test.kind === "recording") lastMaxMs = test.maxMs;
    lastKind = kind;
    phaseStart = performance.now();
    now = phaseStart;
  });

  $effect(() => {
    if (!running) return;
    let frame = requestAnimationFrame(function tick(t) {
      now = t;
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  });

  const elapsed = $derived(now - phaseStart);
  /** Ring: fills while you speak (up to the 15 s limit), drains during playback. */
  const fill = $derived.by(() => {
    if (test.kind === "recording") return Math.min(1, elapsed / test.maxMs);
    if (test.kind === "playing") return recorded * (1 - Math.min(1, elapsed / Math.max(1, test.durationMs)));
    return 0;
  });
  const seconds = $derived(Math.floor(elapsed / 1000));
  const R = 44;
  const C = 2 * Math.PI * R;
</script>

<div class="mictest">
  <div class="stage">
    <svg class="circle" class:rec={test.kind === "recording" || test.kind === "listening"} viewBox="0 0 100 100" aria-hidden="true">
      <circle class="track" cx="50" cy="50" r={R} />
      {#if fill > 0}
        <circle class="ring" class:play={test.kind === "playing"} cx="50" cy="50" r={R} stroke-dasharray="{C * fill} {C}" transform="rotate(-90 50 50)" />
      {/if}
      {#if test.kind === "playing"}
        <path class="icon" d="M42 33v34l28-17z" />
      {:else}
        <circle class="dot" class:live={test.kind === "recording"} cx="50" cy="50" r="22" />
      {/if}
    </svg>
  </div>

  <!-- The status; hovering the panel shows what to do instead. -->
  <div class="msg">
    <strong class="status">
      {#if test.kind === "listening" && test.first}Sprich einfach los …
      {:else if test.kind === "listening"}Sprich, um neu aufzunehmen
      {:else if test.kind === "recording"}Aufnahme · {seconds} s
      {:else if test.kind === "playing"}Wiedergabe
      {:else if test.kind === "failed"}{test.message}
      {:else}Startet …{/if}
    </strong>
    <span class="explain">
      {#if test.kind === "recording"}Eine kurze Pause beendet die Aufnahme.
      {:else if test.kind === "playing"}So kommst du am PC an. Sprich dazwischen für eine neue Aufnahme.
      {:else if test.kind === "listening"}Die Aufnahme startet, sobald du sprichst.
      {:else}Nur im Arbeitsspeicher, nichts wird gespeichert.{/if}
    </span>
  </div>

  <footer>
    <span class="title">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="8.5" y="2.5" width="7" height="12.5" rx="3.5" />
        <path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3.5" />
      </svg>
      Mikrofontest
    </span>
    <button class="end" onclick={onstop}>
      {#if test.kind === "playing" || test.kind === "failed"}
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M10 3L5 8l5 5" /></svg>Zurück
      {:else}
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="4" y="4" width="8" height="8" rx="1.5" /></svg>Beenden
      {/if}
    </button>
  </footer>
</div>

<style>
  .mictest {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* The ring takes whatever height the panel has left. */
  .stage {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
  }
  .circle {
    height: 100%;
    max-height: 140px;
    width: auto;
    aspect-ratio: 1;
  }
  .msg {
    position: relative;
    flex: none;
    height: 40px;
    margin-top: 6px;
    display: grid;
    place-items: center;
    text-align: center;
  }
  .status,
  .explain {
    grid-area: 1 / 1;
    transition: opacity 160ms;
  }
  .status {
    font-size: 17px;
    font-weight: 700;
    color: var(--text);
  }
  .explain {
    font-size: 13px;
    line-height: 1.35;
    color: var(--text-2);
    opacity: 0;
  }
  .mictest:hover .status {
    opacity: 0;
  }
  .mictest:hover .explain {
    opacity: 1;
  }
  footer {
    flex: none;
    margin-top: 10px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .title {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-size: 16px;
    font-weight: 700;
  }
  .title svg {
    width: 22px;
    height: 22px;
    fill: none;
    stroke: var(--state);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .track {
    fill: none;
    stroke: var(--surface-3);
    stroke-width: 5;
  }
  .ring {
    fill: none;
    stroke: var(--error);
    stroke-width: 5;
    stroke-linecap: round;
  }
  .ring.play {
    stroke: var(--accent);
  }
  .dot {
    fill: var(--error);
    opacity: 0.35;
  }
  .rec .dot {
    animation: blink 1.2s ease-in-out infinite;
  }
  .dot.live {
    opacity: 1;
    filter: drop-shadow(0 0 8px rgba(248, 113, 113, 0.7));
  }
  .icon {
    fill: var(--accent);
    filter: drop-shadow(0 0 8px rgba(47, 227, 138, 0.6));
    transform-origin: 56px 50px;
    animation: pulse 1.1s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      transform: scale(0.82);
      opacity: 0.65;
    }
  }
  @keyframes blink {
    50% {
      opacity: 0.25;
    }
  }
  .end {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: inherit;
    font-size: 13px;
    font-weight: 650;
    padding: 6px 12px 6px 10px;
    border-radius: 9px;
    border: 1.5px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }
  .end:hover {
    border-color: var(--text-2);
  }
  .end svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .end rect {
    fill: currentColor;
  }
</style>
