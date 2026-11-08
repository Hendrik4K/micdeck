<script lang="ts">
  import type { MicTestState } from "./types";

  let {
    test,
    muted,
    testable,
    onToggleMute,
    onStart,
    onStop,
  }: {
    test: MicTestState;
    /** Mute state of the DJI input; `null` when there is no input. */
    muted: boolean | null;
    /** A microphone test can run (real DJI input, not demo data). */
    testable: boolean;
    onToggleMute: () => void;
    onStart: () => void;
    onStop: () => void;
  } = $props();

  // The test panel covers the mute button from the click until the session ends.
  let opened = $state(false);
  const running = $derived(test.kind === "listening" || test.kind === "recording" || test.kind === "playing");
  const showTest = $derived(opened || running || test.kind === "failed");

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
    if (lastKind !== "" && kind === "idle") opened = false;
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

  function start() {
    opened = true;
    onStart();
  }
  function stop() {
    opened = false;
    onStop();
  }
</script>

{#if showTest}
  <div class="test">
    <svg class="circle" class:rec={test.kind === "recording" || test.kind === "listening"} class:play={test.kind === "playing"} viewBox="0 0 100 100" aria-hidden="true">
      <circle class="track" cx="50" cy="50" r={R} />
      {#if fill > 0}
        <circle class="ring" cx="50" cy="50" r={R} stroke-dasharray="{C * fill} {C}" transform="rotate(-90 50 50)" />
      {/if}
      {#if test.kind === "playing"}
        <path class="icon" d="M42 33v34l28-17z" />
      {:else}
        <circle class="dot" class:live={test.kind === "recording"} cx="50" cy="50" r="22" />
      {/if}
    </svg>
    <div class="text">
      <strong>
        {#if test.kind === "listening" && test.first}Sprich einfach los …
        {:else if test.kind === "listening"}Sprich, um neu aufzunehmen
        {:else if test.kind === "recording"}Aufnahme · {seconds} s
        {:else if test.kind === "playing"}Wiedergabe
        {:else if test.kind === "failed"}{test.message}
        {:else}Startet …{/if}
      </strong>
      <span>
        {#if test.kind === "recording"}Eine kurze Pause beendet die Aufnahme (höchstens 15 s).
        {:else if test.kind === "playing"}So kommst du am PC an. Sprich dazwischen für eine neue Aufnahme.
        {:else if test.kind === "listening"}Die Aufnahme startet, sobald du sprichst.
        {:else}Nur im Arbeitsspeicher, nichts wird gespeichert.{/if}
      </span>
    </div>
    <button class="end" onclick={stop}>
      {#if test.kind === "playing" || test.kind === "failed"}
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M10 3L5 8l5 5" /></svg>Zurück
      {:else}
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="4" y="4" width="8" height="8" rx="1.5" /></svg>Beenden
      {/if}
    </button>
  </div>
{:else}
  <div class="idle">
    <div class="text">
      <strong>Mikrofontest</strong>
      <button class="link" onclick={start} disabled={!testable}>Test starten</button>
      <span>Sprechen, anhören, nachjustieren.</span>
    </div>
    {#if muted !== null}
      <button
        class="mute"
        class:on={muted}
        onclick={onToggleMute}
        title={muted ? "Stummschaltung aufheben" : "Stummschalten – auch am Empfänger selbst"}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <rect x="8.5" y="3" width="7" height="11.5" rx="3.5" />
          <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
          {#if muted}<path class="slash" d="M3.5 3.5l17 17" />{/if}
        </svg>
        <span>{muted ? "Stumm" : "Mikro an"}</span>
      </button>
    {/if}
  </div>
{/if}

<style>
  .test,
  .idle {
    margin-top: 18px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 18px;
    min-height: 104px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 3px;
    font-size: 13.5px;
    color: var(--text-2);
    justify-items: start;
  }
  strong {
    font-size: 17px;
    font-weight: 700;
    color: var(--text);
  }
  .link {
    font: inherit;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    background: none;
    border: 0;
    padding: 0;
    text-decoration: underline;
    text-underline-offset: 4px;
    text-decoration-thickness: 1.5px;
    cursor: pointer;
  }
  .link:hover:not(:disabled) {
    color: var(--accent);
  }
  .link:disabled {
    color: var(--text-3);
    cursor: default;
  }

  .mute {
    flex: none;
    width: 96px;
    height: 96px;
    border-radius: 50%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 2px;
    font: inherit;
    font-size: 12.5px;
    font-weight: 650;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    border: 2px solid var(--accent);
    box-shadow: 0 0 18px rgba(47, 227, 138, 0.2);
    cursor: pointer;
    transition: background 140ms, color 140ms, border-color 140ms, box-shadow 140ms;
  }
  .mute:hover {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .mute.on {
    color: #1b1503;
    background: var(--warn);
    border-color: var(--warn);
    box-shadow: 0 0 18px rgba(251, 191, 36, 0.3);
  }
  .mute svg {
    width: 34px;
    height: 34px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
  }

  .circle {
    flex: none;
    width: 96px;
    height: 96px;
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
  .play .ring {
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
    flex: none;
    min-width: 132px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    font: inherit;
    font-size: 15px;
    font-weight: 650;
    padding: 14px 20px;
    border-radius: 14px;
    border: 1.5px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }
  .end:hover {
    border-color: var(--text-2);
  }
  .end svg {
    width: 16px;
    height: 16px;
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
