<script lang="ts">
  import type { DeviceImages, Snapshot } from "../lib/types";
  import SettingRow from "../lib/SettingRow.svelte";
  import { slide } from "svelte/transition";
  import { anyPending, byId, errorFor } from "../lib/settings";
  import receiverImg from "../assets/devices/stage-receiver.webp";
  import transmitterImg from "../assets/devices/stage-transmitter.webp";

  let {
    snapshot,
    images,
    now,
    onchange,
  }: {
    snapshot: Snapshot;
    images: DeviceImages;
    now: number;
    onchange: (id: string, value: string) => void;
  } = $props();

  const settings = $derived(snapshot.deviceSettings);
  // Only what MicDeck can change; the plug-free speaker is shown under "Nur
  // Anzeige", voice tone (Mic Mini 2 only) not at all.
  const rx = $derived(settings.filter((s) => s.group === "receiver" && !s.unsupported));
  const tx = $derived(settings.filter((s) => s.group === "transmitter" && !s.unsupported));
  const plugFree = $derived(byId(settings, "plug-free"));
  let open = $state<{ readonly: boolean; info: boolean }>({ readonly: false, info: false });
  const busy = $derived(anyPending(settings));
  /** One reason that locks every setting MicDeck could otherwise send. */
  const banner = $derived.by(() => {
    const reasons = new Set(settings.filter((s) => !s.unsupported).map((s) => s.locked));
    const [only] = reasons;
    return reasons.size === 1 && only ? only : null;
  });
  const receiver = $derived(snapshot.receiver);
  const signed = (db: number) => (db > 0 ? `+${db}` : db < 0 ? `−${-db}` : "0");
</script>

<div class="page">
  <section class="card head">
    <div class="who">
      <h2>{receiver?.name ?? "DJI Mic Mini"}</h2>
      <div class="chips">
        <span class="chip" class:on={receiver !== null}>RX</span>
        {#each snapshot.transmitters as t (t.unit)}
          <span class="chip" class:on={t.connected === true} title={t.battery ? `Akku ${t.battery.label}` : undefined}>
            TX{t.unit}
            {#if t.battery}
              <span class="mini-battery" class:low={t.battery.low}>
                <span style:width="{(t.battery.bars / 6) * 100}%"></span>
              </span>
            {/if}
          </span>
        {/each}
      </div>
      <p class="sub">
        {#if receiver?.firmware}Firmware {receiver.firmware}{receiver.protocolVersion ? ` · Protokoll v${receiver.protocolVersion}` : ""}{:else}Keine Gerätedaten{/if}
      </p>
    </div>
    <div class="pics" aria-hidden="true">
      <img class:custom={images.transmitter !== null} src={images.transmitter ?? transmitterImg} alt="" />
      <img class:custom={images.receiver !== null} src={images.receiver ?? receiverImg} alt="" />
    </div>
  </section>

  {#if banner}
    <p class="banner">{banner}</p>
  {/if}

  <div class="columns">
    <section class="card rx">
      <h3 class="label-caps">RX-Einstellungen</h3>
      <!-- Scrolls inside the card; rows fade out at the edges. -->
      <div class="scroll">
        {#each rx as setting (setting.id)}
          <SettingRow
            {setting}
            {busy}
            hideReason={banner !== null && setting.locked === banner}
            error={errorFor(snapshot.deviceLog, setting.id, now)}
            onchange={(v) => onchange(setting.id, v)}
          />
        {/each}
      </div>
    </section>

    <div class="stack">
      <section class="card">
        <h3 class="label-caps">TX-Einstellungen</h3>
        {#each tx as setting (setting.id)}
          <SettingRow
            {setting}
            {busy}
            hideReason={banner !== null && setting.locked === banner}
            error={errorFor(snapshot.deviceLog, setting.id, now)}
            onchange={(v) => onchange(setting.id, v)}
          />
        {/each}
      </section>

      <section class="card fold">
        <button class="fold-head" aria-expanded={open.readonly} onclick={() => (open.readonly = !open.readonly)}>
          <span class="label-caps">Nur Anzeige</span>
          <span class="chevron" class:up={open.readonly}>›</span>
        </button>
        {#if open.readonly}
          <div transition:slide={{ duration: 200 }}>
            <div class="ro">
              <div>
                <span class="label">Verstärkung (Gain)</span>
                <span class="why">Wird am Drehrad des Empfängers eingestellt, nicht per USB.</span>
              </div>
              <span class="value num">{receiver?.gainDialDb != null ? `${signed(receiver.gainDialDb)} dB` : "–"}</span>
            </div>
            {#if plugFree}
              <div class="ro">
                <div>
                  <span class="label">{plugFree.label}</span>
                  <span class="why">{plugFree.locked}</span>
                </div>
                <span class="value">{plugFree.value === "on" ? "An" : plugFree.value === "off" ? "Aus" : "–"}</span>
              </div>
            {/if}
            <div class="ro">
              <div>
                <span class="label">Firmware-Update</span>
                <span class="why">Nur über die DJI-App. MicDeck verändert keine Firmware.</span>
              </div>
              <span class="tag">nicht steuerbar</span>
            </div>
          </div>
        {/if}
      </section>

      <section class="card fold">
        <button class="fold-head" aria-expanded={open.info} onclick={() => (open.info = !open.info)}>
          <span class="label-caps">Geräte-Info</span>
          <span class="chevron" class:up={open.info}>›</span>
        </button>
        {#if open.info}
          <dl transition:slide={{ duration: 200 }}>
            <dt>Empfänger</dt>
            <dd>{receiver ? receiver.name : "–"}</dd>
            <dt>Seriennummer</dt>
            <dd class="num">{receiver?.serial ?? "–"}</dd>
            <dt>Firmware</dt>
            <dd class="num">{receiver?.firmware ?? "–"}</dd>
            {#each snapshot.transmitters.filter((t) => t.connected === true) as t (t.unit)}
              <dt class="gap">{t.unit === 1 ? "Mikrofon" : `Mikrofon ${t.unit}`}</dt>
              <dd class="gap">{t.name ?? "Verbunden"}</dd>
              <dt>Seriennummer</dt>
              <dd class="num">{t.serial ?? "–"}</dd>
              <dt>Firmware</dt>
              <dd class="num">{t.firmware ?? "–"}</dd>
            {/each}
          </dl>
        {/if}
      </section>
    </div>
  </div>
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 18px 22px;
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 24px;
    font-weight: 750;
    letter-spacing: -0.01em;
  }
  .chips {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    font-weight: 700;
    padding: 3px 9px;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--text-3);
  }
  .chip.on {
    color: var(--text);
  }
  .mini-battery {
    width: 20px;
    height: 9px;
    border-radius: 2px;
    border: 1.2px solid var(--text-2);
    padding: 1px;
    display: inline-flex;
  }
  .mini-battery span {
    border-radius: 1px;
    background: var(--ok);
  }
  .mini-battery.low span {
    background: var(--warn);
  }
  .sub {
    margin: 8px 0 0;
    font-size: 13px;
    color: var(--text-3);
  }
  .pics {
    display: flex;
    align-items: center;
    height: 92px;
  }
  .pics img {
    height: 100%;
    mask-image: radial-gradient(closest-side, #000 55%, transparent 96%);
  }
  /* The user's own pictures are cut out already. */
  .pics img.custom {
    height: 80%;
    margin: 0 6px;
    mask-image: none;
  }
  .banner {
    margin: 0;
    padding: 12px 16px;
    border-radius: 12px;
    font-size: 14px;
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--warn) 30%, transparent);
  }
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 16px;
    align-items: stretch;
  }
  /* As tall as the right column; the rows scroll under the fixed title. */
  .rx {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .scroll {
    flex: 1 1 0;
    height: 0;
    min-height: 260px;
    overflow-y: auto;
    padding: 10px 6px 18px 0;
    margin-top: -8px;
    mask-image: linear-gradient(to bottom, transparent, #000 18px, #000 calc(100% - 26px), transparent);
  }
  .stack {
    align-content: start;
  }
  .fold {
    padding-top: 6px;
    padding-bottom: 6px;
  }
  .fold-head {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 0;
    font: inherit;
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
  }
  .fold-head .label-caps {
    margin: 0;
  }
  .fold-head:hover .label-caps {
    color: var(--text-2);
  }
  .chevron {
    font-size: 20px;
    line-height: 1;
    color: var(--text-3);
    transform: rotate(90deg);
    transition: transform 200ms;
  }
  .chevron.up {
    transform: rotate(-90deg);
  }
  .fold dl {
    padding: 4px 0 12px;
  }
  .stack {
    display: grid;
    gap: 16px;
  }
  .ro {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--line);
  }
  .ro:first-of-type {
    border-top: 0;
    padding-top: 2px;
  }
  .ro > div {
    flex: 1;
    display: grid;
    gap: 2px;
  }
  .label {
    font-size: 15.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  .why {
    font-size: 12px;
    color: var(--text-3);
  }
  .value {
    font-size: 16px;
    font-weight: 650;
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 7px 18px;
    font-size: 14px;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    color: var(--text-2);
    text-align: right;
  }
  .gap {
    margin-top: 8px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    color: var(--text);
    font-weight: 600;
  }
  @media (max-width: 820px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
