<script lang="ts">
  import { tick } from "svelte";
  import type { Check, DeviceImages, LedLayout, LedSpot, Snapshot } from "./types";
  // Built-in placeholders: silhouettes of the devices (AI-generated, see
  // THIRD_PARTY.md). The user's own pictures replace them.
  import receiverImg from "../assets/devices/stage-receiver.webp";
  import transmitterImg from "../assets/devices/stage-transmitter.webp";
  import UsbLock from "./UsbLock.svelte";

  let {
    snapshot,
    images,
    onjack,
  }: { snapshot: Snapshot; images: DeviceImages; onjack?: (nodeName: string) => void } = $props();

  /** Receiver on its 3.5 mm output: sound, but no status data. */
  const jackMode = $derived(snapshot.connection === "jack" && snapshot.jack !== null);
  /** The socket a click on „Klinke“ switches to, if there is another one. */
  const nextJack = $derived.by(() => {
    const j = snapshot.jack;
    if (!j || j.options.length < 2) return null;
    const i = j.options.findIndex((o) => o.nodeName === j.nodeName);
    return j.options[(i + 1) % j.options.length];
  });

  const usb = $derived(snapshot.checks.find((c) => c.id === "usb"));
  const tx1 = $derived(snapshot.transmitters.find((t) => t.unit === 1));
  const tx2 = $derived(snapshot.transmitters.find((t) => t.unit === 2));
  /** Receiver on USB and switched on. */
  const on = $derived(snapshot.receiver !== null && !snapshot.receiver.switchedOff);
  const linked = $derived(on && tx1?.connected === true);

  // Status lights drawn over the pictures, like the ones on the devices:
  // power next to "MIC RX", one per microphone above "1"/"2", and the light
  // on the microphone. Yellow blinks while the microphone is not connected.
  type Light = "green" | "red" | "yellow" | "dark" | null;
  const powerLight = $derived<Light>(!snapshot.receiver ? null : snapshot.receiver.switchedOff ? "red" : "green");
  const unit1Light = $derived<Light>(!on ? null : tx1?.connected === true ? "green" : tx1?.connected === false ? "yellow" : null);
  const unit2Light = $derived<Light>(on && tx2?.connected === true ? "green" : null);
  const txLight = $derived<Light>(!on ? "dark" : tx1?.connected === true ? "green" : tx1?.connected === false ? "yellow" : "dark");

  /** Where the lights sit on the built-in silhouettes, in % of the picture. */
  const BUILTIN_LEDS: LedLayout = {
    receiver: { power: [43.4, 46], unit1: [23.7, 32.2], unit2: [32.9, 31.3] },
    transmitter: { status: [67.2, 64] },
  };
  // Own pictures only get lights where leds.json places them.
  const rxLeds = $derived(images.receiver ? images.leds?.receiver : BUILTIN_LEDS.receiver);
  const txLeds = $derived(images.transmitter ? images.leds?.transmitter : BUILTIN_LEDS.transmitter);

  // The lights are placed on the picture itself, so its frame must match the
  // picture's shape inside the fixed box.
  let rxRatio = $state(0);
  let txRatio = $state(0);
  function fit(boxW: number, boxH: number, ratio: number) {
    const w = ratio ? Math.min(boxW, boxH * ratio) : boxW;
    return { w, h: ratio ? w / ratio : boxH };
  }
  const rxBox = $derived(fit(400, 233, rxRatio));
  const txBox = $derived(fit(260, 300, txRatio));
  const ratioOf = (el: Element) => {
    const img = el as HTMLImageElement;
    return img.naturalHeight ? img.naturalWidth / img.naturalHeight : 0;
  };
  const mark = (c: Check | undefined) => c?.level ?? "pending";
  // Muted with nothing else wrong: still "Einsatzbereit" in green; the red
  // accents show the mute. Details live on the Diagnose page.
  const muteOnly = $derived(
    snapshot.audio.source?.muted === true &&
      snapshot.checks.every((c) => c.id === "mute" || c.id === "signal" || c.level === "ok" || c.level === "inactive"),
  );
  const verdict = $derived(muteOnly ? { level: "ok" as const, title: "Einsatzbereit" } : snapshot.verdict);
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

<section class="stage level-{verdict.level}" aria-live="polite">
  <div class="status">
    <p class="kicker">Dein {snapshot.receiver?.name ?? "DJI Mic Mini"}</p>
    <h1 class:long={verdict.title.length > 15}>{verdict.title}</h1>

    <ul class="units" bind:this={unitsEl}>
      <li>
        <span class="name">Empfänger</span>
        {#if jackMode && snapshot.jack}
          <!-- Hover names the socket, a click moves to the next plugged one. -->
          <button
            class="jack"
            class:switchable={nextJack !== null}
            disabled={nextJack === null}
            onclick={() => nextJack && onjack?.(nextJack.nodeName)}
            title={nextJack ? `Klicken: zu Klinke ${nextJack.label} wechseln` : `Über Klinke ${snapshot.jack.label} verbunden`}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 1.5v5M6 6.5h4v3.2a2 2 0 0 1-4 0zM8 11.7v2.8" /></svg>
            <span>Klinke</span><span class="where">&nbsp;{snapshot.jack.label}</span>
          </button>
        {:else if mark(usb) === "ok"}
          <span class="ok-dot" title={usb?.value}>
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8.5l2.6 2.6L12 5.5" /></svg>
          </span>
        {:else}
          <span class="pill {mark(usb)}">{usb?.value ?? "Prüfe …"}</span>
        {/if}
      </li>
      <!-- Only transmitter 1 for now: most people use one microphone. -->
      {#each snapshot.transmitters.filter((t) => t.unit === 1) as tx (tx.unit)}
        <li class:dim={tx.connected !== true && !jackMode}>
          <span class="name">{tx.unit === 1 ? "Mikrofon" : `Mikrofon ${tx.unit}`}</span>
          {#if jackMode}
            <!-- Battery and radio state only come over USB-C. -->
            <UsbLock chip />
          {:else if tx.connected === true}
            <!-- The battery only shows once it matters (half or less) or while
                 charging; otherwise a tick says the microphone is connected. -->
            {#if tx.battery && (tx.charging === true || tx.battery.gauge >= 4)}
              {@const at = Math.min(tx.battery.bars, 5)}
              <span
                class="battery"
                class:loading={tx.charging === true}
                class:low={tx.battery.low && !tx.charging}
                class:critical={tx.battery.critical && !tx.charging}
                title="Akku {tx.battery.label} · Gerätewert: Stufe {tx.battery.gauge} von 7 (1 = voll)"
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
            {:else if !tx.charging}
              <span class="ok-dot" title={tx.battery ? `Verbunden · Akku ${tx.battery.label}` : "Verbunden"}>
                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8.5l2.6 2.6L12 5.5" /></svg>
              </span>
            {/if}
            {#if tx.charging}
              <svg class="bolt beat" viewBox="0 0 16 16" aria-label="lädt">
                <title>{tx.battery ? `Lädt · ${tx.battery.label}` : "Lädt im Ladecase"}</title>
                <path d="M9 1.5L3.5 9H8l-1 5.5L12.5 7H8z" />
              </svg>
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

  {#snippet led(spot: LedSpot | undefined, light: Light, kind: string)}
    {#if spot && light}
      <span
        class="led {kind} {light}"
        style:left="{spot[0]}%"
        style:top="{spot[1]}%"
        style:width={spot[2] != null ? `${spot[2]}%` : null}
        style:height={spot[3] != null ? `${spot[3]}%` : null}
      ></span>
    {/if}
  {/snippet}

  <div class="devices" class:absent={!snapshot.receiver && !jackMode} aria-hidden="true">
    <div class="txwrap">
      <div class="pic" style:width="{txBox.w}px" style:height="{txBox.h}px">
        <img
          class="tx"
          class:custom={images.transmitter !== null}
          class:off={!linked && !jackMode}
          src={images.transmitter ?? transmitterImg}
          alt=""
          onload={(e) => (txRatio = ratioOf(e.currentTarget))}
        />
        {#if txRatio}{@render led(txLeds?.status, txLight, "status")}{/if}
      </div>
    </div>
    <div class="rx">
      <div class="pic" style:width="{rxBox.w}px" style:height="{rxBox.h}px">
        <img
          class:custom={images.receiver !== null}
          src={images.receiver ?? receiverImg}
          alt=""
          onload={(e) => (rxRatio = ratioOf(e.currentTarget))}
        />
        {#if rxRatio}
          {@render led(rxLeds?.power, powerLight, "power")}
          {@render led(rxLeds?.unit1, unit1Light, "unit")}
          {@render led(rxLeds?.unit2, unit2Light, "unit")}
        {/if}
      </div>
    </div>
  </div>
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

  .kicker {
    margin: 0;
    font-size: 21px;
    font-weight: 600;
    color: var(--text-2);
  }
  h1 {
    /* Long titles wrap before they reach the pictures. */
    max-width: 440px;
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
  /* "Klinke": green like the tick for USB-C; the socket name slides in on hover. */
  .jack {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 12px 3px 9px;
    border-radius: 999px;
    border: 1.5px solid color-mix(in srgb, var(--ok) 55%, transparent);
    background: color-mix(in srgb, var(--ok) 12%, transparent);
    color: var(--ok);
    font: inherit;
    font-size: 13.5px;
    font-weight: 700;
    cursor: default;
    transition: background 150ms, border-color 150ms;
  }
  .jack.switchable {
    cursor: pointer;
  }
  .jack.switchable:hover {
    background: color-mix(in srgb, var(--ok) 22%, transparent);
    border-color: var(--ok);
  }
  .jack:disabled {
    opacity: 1;
  }
  .jack svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .jack .where {
    display: inline-block;
    max-width: 0;
    overflow: hidden;
    white-space: nowrap;
    opacity: 0;
    transition: max-width 220ms ease, opacity 220ms ease;
  }
  .jack:hover .where {
    max-width: 60px;
    opacity: 1;
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
    /* No coloured halo: the status lights on the devices say it all. */
    filter: drop-shadow(0 0 1px rgba(255, 255, 255, 0.35));
    transition: opacity 400ms, filter 400ms;
  }
  .tx.off {
    opacity: 0.45;
    filter: grayscale(1);
  }
  .pic {
    position: relative;
  }
  /* Status lights on the pictures. */
  .led {
    position: absolute;
    z-index: 1;
    transform: translate(-50%, -50%);
    border-radius: 999px;
    --c: var(--ok);
    background: var(--c);
    box-shadow:
      0 0 4px 1px color-mix(in srgb, var(--c) 80%, transparent),
      0 0 14px 3px color-mix(in srgb, var(--c) 45%, transparent);
    transition: background 300ms, box-shadow 300ms;
  }
  .led.power {
    width: 2%;
    height: 7.4%;
  }
  .led.unit {
    width: 2.2%;
    height: 3%;
  }
  .led.status {
    width: 2.6%;
    height: 7.6%;
  }
  .led.red {
    --c: var(--error);
  }
  .led.yellow {
    --c: var(--warn);
    animation: led-blink 1.1s ease-in-out infinite;
  }
  /* Covers a light that is lit in the photo. */
  .led.dark {
    background: #17191b;
    box-shadow: 0 0 3px 2px #17191b;
  }
  @keyframes led-blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.12;
    }
  }
  .absent img {
    opacity: 0.25;
    filter: grayscale(1);
  }
  /* The receiver in the middle, tilted to the left; the transmitter comes
     up from the bottom right corner, tilted the other way and half hidden. */
  .rx,
  .txwrap {
    display: grid;
    place-items: center;
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

  @media (max-width: 820px) {
    .stage {
      grid-template-columns: 1fr;
    }
  }
</style>
