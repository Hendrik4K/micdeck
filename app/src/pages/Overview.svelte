<script lang="ts">
  import type { DeviceImages, LevelFrame, Snapshot } from "../lib/types";
  import type { Page } from "../lib/Nav.svelte";
  import Stage from "../lib/Stage.svelte";
  import LevelMeter from "../lib/LevelMeter.svelte";
  import MicPanel from "../lib/MicPanel.svelte";
  import QuickSettings from "../lib/QuickSettings.svelte";

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
    };
    go: (page: Page) => void;
  } = $props();
</script>

<div class="page">
  <Stage {snapshot} {images} />
  <div class="columns">
    <LevelMeter
      frame={level}
      meter={snapshot.meter}
      signal={snapshot.audio.signal}
      {labels}
      source={snapshot.audio.source?.description ?? null}
      recording={snapshot.audio.source?.captureApps ?? []}
    >
      <MicPanel
        test={snapshot.audio.micTest}
        muted={snapshot.audio.source?.muted ?? null}
        testable={snapshot.audio.source !== null && !snapshot.simulated}
        onToggleMute={actions.toggleMute}
        onStart={actions.startMicTest}
        onStop={actions.stopMicTest}
      />
    </LevelMeter>
    <QuickSettings
      settings={snapshot.deviceSettings}
      log={snapshot.deviceLog}
      {now}
      onchange={actions.setSetting}
      onmore={() => go("device")}
    />
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
    grid-template-columns: minmax(0, 1.12fr) minmax(0, 1fr);
    gap: 16px;
    align-items: stretch;
  }
  @media (max-width: 820px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
