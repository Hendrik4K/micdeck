<script lang="ts">
  import type { DeviceImages, LevelFrame, Snapshot } from "../lib/types";
  import type { DeviceName } from "../lib/api";
  import type { Page } from "../lib/Nav.svelte";
  import Stage from "../lib/Stage.svelte";
  import LevelMeter from "../lib/LevelMeter.svelte";
  import MuteButton from "../lib/MuteButton.svelte";
  import MicTest from "../lib/MicTest.svelte";
  import QuickSettings from "../lib/QuickSettings.svelte";
  import { fade } from "svelte/transition";

  let {
    snapshot,
    level,
    images,
    labels,
    now,
    actions,
    go,
  }: {
    snapshot: Snapshot;
    level: LevelFrame | null;
    images: DeviceImages;
    labels: string[];
    now: number;
    actions: {
      toggleMute: () => void;
      startMicTest: () => void;
      stopMicTest: () => void;
      setSetting: (id: string, value: string) => void;
      setJack: (nodeName: string) => void;
      setImage: (device: DeviceName, file: File) => Promise<void>;
      resetImage: (device: DeviceName) => Promise<void>;
    };
    go: (page: Page) => void;
  } = $props();

  const test = $derived(snapshot.audio.micTest);
  const muted = $derived(snapshot.audio.source?.muted ?? null);
  // The test takes the settings' place from the click until the session ends.
  let opened = $state(false);
  let wasRunning = false;
  const running = $derived(test.kind === "listening" || test.kind === "recording" || test.kind === "playing");
  const showTest = $derived(opened || running || test.kind === "failed");
  $effect(() => {
    if (wasRunning && test.kind === "idle") opened = false;
    wasRunning = running;
  });

  function startTest() {
    opened = true;
    actions.startMicTest();
  }
  function stopTest() {
    opened = false;
    actions.stopMicTest();
  }
</script>

<div class="page">
  <Stage {snapshot} {images} onjack={actions.setJack} onimage={actions.setImage} onresetimage={actions.resetImage} />
  <div class="columns">
    <LevelMeter
      frame={level}
      meter={snapshot.meter}
      signal={snapshot.audio.signal}
      {labels}
      source={snapshot.audio.source?.description ?? null}
      recording={snapshot.audio.source?.captureApps ?? []}
    >
      {#snippet side()}
        {#if muted !== null}
          <MuteButton {muted} onclick={actions.toggleMute} />
        {/if}
      {/snippet}
    </LevelMeter>
    <section class="card side">
      <!-- The settings stay in place (hidden during the test), so the panel
           keeps its size; the test is laid over them. -->
      <div class="swap" class:away={showTest} inert={showTest}>
        <QuickSettings
          settings={snapshot.deviceSettings}
          log={snapshot.deviceLog}
          {now}
          onchange={actions.setSetting}
          onmore={() => go("device")}
          testable={snapshot.audio.source !== null && !snapshot.simulated}
          ontest={startTest}
          usbOnly={snapshot.connection === "jack"}
        />
      </div>
      {#if showTest}
        <div class="over" in:fade={{ duration: 180, delay: 80 }}>
          <MicTest {test} onstop={stopTest} />
        </div>
      {/if}
    </section>
  </div>
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 16px;
    align-items: stretch;
  }
  .side {
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .swap {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    transition: opacity 160ms;
  }
  .swap.away {
    opacity: 0;
    visibility: hidden;
  }
  /* Same box as the card's content (card padding 20px 22px). */
  .over {
    position: absolute;
    inset: 20px 22px;
    display: flex;
    flex-direction: column;
  }
  @media (max-width: 820px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
