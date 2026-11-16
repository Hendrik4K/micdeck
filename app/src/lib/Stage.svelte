<script lang="ts">
  import { tick } from "svelte";
  import type { Check, DeviceImages, Snapshot, TransmitterView } from "./types";
  // Built-in placeholders: silhouettes of the devices (AI-generated, see
  // THIRD_PARTY.md). The user's own pictures replace them.
  import receiverImg from "../assets/devices/stage-receiver.webp";
  import transmitterImg from "../assets/devices/stage-transmitter.webp";

  let { snapshot, images }: { snapshot: Snapshot; images: DeviceImages } = $props();

  const usb = $derived(snapshot.checks.find((c) => c.id === "usb"));
  const tx1 = $derived(snapshot.transmitters.find((t) => t.unit === 1));
  const linked = $derived(snapshot.receiver !== null && tx1?.connected === true);

  function txText(tx: TransmitterView): string {
    if (tx.battery) return `${capital(tx.battery.label)}${tx.battery.charging ? " · lädt" : ""}`;
    if (tx.charging) return "Im Ladecase · lädt";
    return "Verbunden";
  }
  const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  const mark = (c: Check | undefined) => c?.level ?? "pending";
  // Muted with nothing else wrong: still "Einsatzbereit" in green, but the
  // light and the radio waves turn red. Details live on the Diagnose page.
  const muteOnly = $derived(
    snapshot.audio.source?.muted === true &&
      snapshot.checks.every((c) => c.id === "mute" || c.id === "signal" || c.level === "ok" || c.level === "inactive"),
  );
  const verdict = $derived(muteOnly ? { level: "ok" as const, title: "Einsatzbereit" } : snapshot.verdict);
  const muted = $derived(snapshot.audio.source?.muted === true);
  /** Charging: bar, blinking cell and bolt share one beat (see `sync`). */
  let unitsEl = $state<HTMLElement | null>(null);
  const chargeKey = $derived(snapshot.transmitters.map((t) => `${t.charging}:${t.battery?.bars}`).join(","));
  $effect(() => {
    void chargeKey;
    void sync();
  });
  async function sync() {
    await tick();
    // Align every charging animation to the same clock, whenever one starts.
    for (const el of unitsEl?.querySelectorAll<HTMLElement>(".beat") ?? []) {
      for (const a of el.getAnimations()) a.startTime = 0;
    }
  }
</script>

<section class="stage level-{verdict.level}" class:muted aria-live="polite">
  <div class="status">
    <p class="kicker">Dein {snapshot.receiver?.name ?? "DJI Mic Mini"}</p>
    <h1 class:long={verdict.title.length > 18}>{verdict.title}</h1>

    <ul class="units" bind:this={unitsEl}>
      <li>
        <span class="name">Empfänger</span>
        {#if mark(usb) === "ok"}
          <span class="ok-dot" title={usb?.value}>
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8.5l2.6 2.6L12 5.5" /></svg>
          </span>
        {:else}
          <span class="pill {mark(usb)}">{usb?.value ?? "Prüfe …"}</span>
        {/if}
      </li>
      <!-- Only transmitter 1 for now: most people use one microphone. -->
      {#each snapshot.transmitters.filter((t) => t.unit === 1) as tx (tx.unit)}
        <li class:dim={tx.connected !== true}>
          <span class="name">{tx.unit === 1 ? "Mikrofon" : `Mikrofon ${tx.unit}`}</span>
          {#if tx.connected === true}
            {#if tx.battery}
              {@const at = Math.min(tx.battery.bars, 5)}
              <span
                class="battery"
                class:loading={tx.charging === true}
                class:low={tx.battery.low && !tx.charging}
                class:critical={tx.battery.critical && !tx.charging}
                title="Gerätewert: Stufe {tx.battery.gauge} von 7 (1 = voll)"
              >
                {#each Array.from({ length: 6 }, (_, i) => i) as i (i)}
                  <span
                    class="cell"
                    class:on={i < tx.battery.bars}
                    class:beat={tx.charging === true && i <= at}
                    class:next={tx.charging === true && i === at}
                    style:--i={i}
                  ></span>
                {/each}
              </span>
            {/if}
            {#if tx.charging}
              <svg class="bolt beat" viewBox="0 0 16 16" aria-label="lädt">
                <title>{tx.battery ? `Lädt · ${tx.battery.label}` : "Lädt im Ladecase"}</title>
                <path d="M9 1.5L3.5 9H8l-1 5.5L12.5 7H8z" />
              </svg>
            {:else if tx.battery && tx.battery.gauge === 1}
              <span class="mark full" title="Akku voll">
                <svg viewBox="0 0 16 16" aria-label="voll"><path d="M4 8.5l2.6 2.6L12 5.5" /></svg>
              </span>
            {:else if !tx.battery || tx.battery.low}
              <span class="value">{txText(tx)}</span>
            {/if}
          {:else if tx.connected === false}
            <span class="pill">Nicht verbunden</span>
          {:else}
            <span class="pill" title="Ohne Statusdaten vom Empfänger nicht prüfbar">Status unbekannt</span>
          {/if}
        </li>
      {/each}
    </ul>
  </div>

  <div class="devices" class:absent={!snapshot.receiver} aria-hidden="true">
    <div class="txwrap">
      <img
        class="tx"
        class:custom={images.transmitter !== null}
        class:off={tx1?.connected !== true}
        src={images.transmitter ?? transmitterImg}
        alt=""
      />
    </div>
    <svg class="waves" class:on={linked} viewBox="0 0 40 60">
      <path d="M8 22a11 11 0 0 1 0 16" />
      <path d="M16 15a20 20 0 0 1 0 30" />
      <path d="M24 8a29 29 0 0 1 0 44" />
    </svg>
    <img class="rx" class:custom={images.receiver !== null} src={images.receiver ?? receiverImg} alt="" />
  </div>
  <!-- Status-coloured light over the pictures, which sit on pure black. -->
  <div class="glow" aria-hidden="true"></div>
</section>

<style>
  .stage {
    --tone: var(--pending);
    position: relative;
    display: grid;
    grid-template-columns: minmax(300px, 0.9fr) minmax(0, 1.2fr);
    align-items: center;
    gap: 12px;
    min-height: 336px;
    padding: 26px 34px;
    border-radius: 22px;
    border: 1px solid var(--line);
    /* The device pictures have a pure black backdrop; the right side matches it. */
    background: linear-gradient(90deg, var(--surface) 0%, var(--surface) 14%, #08090b 38%, #000 56%) !important;
    overflow: hidden;
  }
  .status {
    position: relative;
    z-index: 2;
  }
  .level-ok { --tone: var(--ok); }
  .level-warn { --tone: var(--warn); }
  .level-error { --tone: var(--error); }
  /* The light over the pictures: the status colour, red while muted. */
  .stage {
    --light: var(--tone);
  }
  .stage.muted {
    --light: var(--error);
  }

  .kicker {
    margin: 0;
    font-size: 21px;
    font-weight: 600;
    color: var(--text-2);
  }
  h1 {
    margin: 2px 0 4px;
    font-size: clamp(34px, 4.4vw, 54px);
    line-height: 1.05;
    font-weight: 800;
    letter-spacing: -0.02em;
    color: var(--tone);
    text-shadow: 0 0 28px color-mix(in srgb, var(--tone) 30%, transparent);
  }
  h1.long {
    font-size: clamp(28px, 3.2vw, 40px);
  }
  .level-pending h1 {
    color: var(--text);
    text-shadow: none;
  }
  .units {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    max-width: 430px;
  }
  .units li {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 0;
    font-size: 16px;
  }
  /* Divider that fades out to the right instead of ending hard. */
  .units li + li::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(90deg, var(--line-2) 0%, var(--line) 45%, transparent 100%);
  }
  .units li.dim .name {
    color: var(--text-2);
  }
  .name {
    font-weight: 600;
  }
  .value {
    color: var(--text-2);
    font-size: 15px;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
  }
  .mark svg {
    width: 14px;
    height: 14px;
  }
  .mark.full {
    background: var(--ok);
  }
  .mark.full svg {
    fill: none;
    stroke: var(--accent-ink);
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ok-dot {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--ok);
  }
  .ok-dot svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: var(--accent-ink);
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .pill {
    font-size: 13px;
    color: var(--text-2);
    padding: 3px 11px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pill.warn {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 45%, transparent);
  }
  .pill.error {
    color: var(--error);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
  }
  .battery {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: 5px;
    border: 1.5px solid var(--ok);
    position: relative;
    margin-right: 3px;
  }
  .battery::after {
    content: "";
    position: absolute;
    right: -5px;
    top: 5px;
    width: 3px;
    height: 7px;
    border-radius: 0 2px 2px 0;
    background: var(--ok);
  }
  .cell {
    width: 6px;
    height: 11px;
    border-radius: 1.5px;
    background: var(--surface-3);
  }
  .cell.on {
    background: var(--ok);
  }
  .battery.low {
    border-color: var(--warn);
  }
  .battery.low::after,
  .battery.low .cell.on {
    background: var(--warn);
  }
  .battery.critical {
    border-color: var(--error);
  }
  .battery.critical::after,
  .battery.critical .cell.on {
    background: var(--error);
  }

  /* Charging: a light runs through the filled cells, the cell being charged
     blinks, and the bolt beside it pulses on the same beat. */
  .battery.loading .cell.on {
    animation: sweep 1.4s ease-in-out infinite;
    animation-delay: calc(var(--i) * 90ms);
  }
  .battery.loading .cell.next {
    background: var(--ok);
    animation: charge 1.4s ease-in-out infinite;
    animation-delay: 0s;
  }
  @keyframes sweep {
    0%,
    60%,
    100% {
      filter: none;
    }
    25% {
      filter: brightness(1.55) saturate(0.8);
    }
  }
  @keyframes charge {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.12;
    }
  }
  .bolt {
    width: 20px;
    height: 20px;
    fill: var(--warn);
    filter: drop-shadow(0 0 6px rgba(251, 191, 36, 0.55));
    transform-origin: center;
    animation: bolt 1.4s ease-in-out infinite;
  }
  @keyframes bolt {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.3;
      transform: scale(0.82);
    }
  }

  /* The pictures fill the right part of the stage up to its edges, so the
     receiver can rise from the bottom edge, partly cut off. */
  .status {
    grid-column: 1;
  }
  .devices {
    position: absolute;
    inset: 0 0 0 36%;
    --tx-h: 300px;
  }
  .devices > * {
    position: absolute;
  }
  .glow {
    position: absolute;
    inset: 0;
    z-index: 1;
    pointer-events: none;
    background: radial-gradient(var(--glow-size, 34% 70%) at var(--glow-at, 64% 50%), color-mix(in srgb, var(--light) 18%, transparent), transparent 72%);
    mix-blend-mode: screen;
    transition: background 400ms ease;
  }
  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
    /* The pictures sit on a dark studio backdrop; fade its edges out. */
    mask-image: radial-gradient(closest-side, #000 82%, transparent 100%);
    transition: opacity 400ms, filter 400ms;
  }
  /* The user's own pictures are cut out: no edge fade, a soft rim instead. */
  img.custom {
    mask-image: none;
    filter: drop-shadow(0 0 1px rgba(255, 255, 255, 0.35))
      drop-shadow(0 8px 26px color-mix(in srgb, var(--light) 32%, transparent));
  }
  .tx.off {
    opacity: 0.35;
    filter: grayscale(1);
  }
  .absent img {
    opacity: 0.25;
    filter: grayscale(1);
  }
  .waves {
    width: 40px;
    height: 60px;
    z-index: 2;
    fill: none;
    stroke: var(--light);
    stroke-width: 3.2;
    stroke-linecap: round;
    opacity: 0;
    transition: opacity 400ms;
  }
  .waves.on {
    opacity: 1;
  }
  .waves.on path {
    animation: wave 2.4s ease-in-out infinite;
  }
  .waves path:nth-child(2) {
    animation-delay: 0.25s;
  }
  .waves path:nth-child(3) {
    animation-delay: 0.5s;
  }
  @keyframes wave {
    0%,
    100% {
      opacity: 0.3;
    }
    40% {
      opacity: 1;
    }
  }

  /* The receiver in the middle, tilted to the left; the transmitter comes
     up from the bottom right corner, tilted the other way and half hidden.
     The waves run between them. */
  .stage {
    --glow-at: 52% 50%;
  }
  .rx {
    z-index: 1;
    left: 6%;
    top: 50%;
    width: 400px;
    height: 233px;
    transform: translateY(-50%) rotate(-9deg);
  }
  .txwrap {
    z-index: 2;
    right: 2%;
    bottom: -64px;
    width: 260px;
    height: var(--tx-h);
    transform: rotate(11deg);
  }
  .waves {
    left: 70%;
    top: 22%;
    transform: rotate(200deg);
  }

  @media (max-width: 820px) {
    .stage {
      grid-template-columns: 1fr;
    }
  }
</style>
