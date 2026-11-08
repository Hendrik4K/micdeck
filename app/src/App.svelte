<script lang="ts">
  import { onMount, tick } from "svelte";
  import { connect, type Backend } from "./lib/api";
  import type { AppSettingName, AppSettings, CheckLevel, DeviceImages, LevelFrame, Snapshot } from "./lib/types";
  import Nav, { type Page } from "./lib/Nav.svelte";
  import Overview from "./pages/Overview.svelte";
  import Device from "./pages/Device.svelte";
  import Diagnose from "./pages/Diagnose.svelte";
  import AppPage from "./pages/AppPage.svelte";

  let snapshot = $state<Snapshot | null>(null);
  let level = $state<LevelFrame | null>(null);
  let appSettings = $state<AppSettings | null>(null);
  let images = $state<DeviceImages>({ receiver: null, transmitter: null });
  let failure = $state<string | null>(null);
  let page = $state<Page>(storedPage());
  /** Wall clock for fading out setting feedback. */
  let now = $state(Date.now());
  let backend: Backend | undefined;

  const PAGES: Page[] = ["overview", "device", "diagnose", "app"];
  /** Window width the layout is drawn for (default window size). */
  const DESIGN_WIDTH = 1240;
  // One dark frame holds the bar and, below it, the page's first panel, as
  // wide as that panel. The active page's tab runs down into that panel like
  // an index card. Switching pages folds it all up and open again (see `go`).
  /** Space between the frame's edge and the bar. */
  const FRAME_PAD = 6;
  const FOLD_MS = 240;
  const MOVE_MS = 220;
  const FADE_MS = 170;
  const FOLD_EASING = "cubic-bezier(0.3, 0.7, 0.2, 1)";
  let mainEl = $state<HTMLElement | null>(null);
  let header = $state<HTMLElement | null>(null);
  let frame = $state<{ top: number; height: number } | null>(null);
  let tab = $state<{ left: number; top: number; width: number; height: number; radius: string } | null>(null);
  /** How far the tab reaches under the panel, behind its rounded corners. */
  const TAB_OVERLAP = 24;
  /** Folded up during a page change: frame and tab end at the bar. */
  let folded = $state(false);
  let observer: ResizeObserver | undefined;

  async function measureFrame() {
    await tick();
    if (!header || !mainEl) return;
    const top = header.offsetTop - FRAME_PAD;
    const panel = folded ? null : mainEl.querySelector<HTMLElement>(".page > :first-child");
    const bottom = panel ? panel.offsetTop + panel.offsetHeight : header.offsetTop + header.offsetHeight + FRAME_PAD;
    frame = { top, height: bottom - top };
    const active = header.querySelector<HTMLElement>('nav button[aria-current="page"]');
    if (active && folded) {
      // Folded up: a round bubble around the label, which then moves.
      const height = active.offsetHeight - 12;
      tab = {
        left: active.offsetLeft + 4,
        top: active.offsetTop + 6,
        width: active.offsetWidth - 8,
        height,
        radius: `${height / 2}px`,
      };
    } else if (active) {
      // Open: square at the bottom, running into the panel. MicDeck at the
      // left end fills the frame's corner, so there is one curve, not two.
      const atLeft = active.offsetLeft <= FRAME_PAD + 2;
      const left = atLeft ? 0 : active.offsetLeft;
      const tabTop = atLeft ? -FRAME_PAD : active.offsetTop;
      const tabBottom = panel ? panel.offsetTop - header.offsetTop + TAB_OVERLAP : active.offsetTop + active.offsetHeight;
      tab = {
        left,
        top: tabTop,
        width: active.offsetLeft + active.offsetWidth - left,
        height: tabBottom - tabTop,
        radius: atLeft ? "22px 14px 0 0" : "14px 14px 0 0",
      };
    } else {
      tab = null;
    }
    observer?.disconnect();
    observer = new ResizeObserver(() => measureFrame());
    observer.observe(header);
    if (panel) observer.observe(panel);
  }
  $effect(() => {
    void page;
    void snapshot?.simulated;
    void failure;
    measureFrame();
  });

  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const parts = () => ({
    panel: mainEl?.querySelector<HTMLElement>(".page > :first-child") ?? null,
    rest: [...(mainEl?.querySelectorAll<HTMLElement>(".page > :not(:first-child)") ?? [])],
  });
  let switching = false;
  let queued: Page | null = null;

  /** Page change: the lower panels fade out while the first one folds up
   *  with the frame; the tab moves over; the new panel folds open, then
   *  its lower panels fade in. */
  async function switchTo(next: Page) {
    switching = true;
    const old = parts();
    for (const el of old.rest) el.animate([{ opacity: 1 }, { opacity: 0 }], { duration: FADE_MS, fill: "forwards" });
    old.panel?.animate([{ clipPath: "inset(0 0 0 0)" }, { clipPath: "inset(0 0 100% 0)" }], {
      duration: FOLD_MS,
      delay: 60,
      easing: FOLD_EASING,
      fill: "forwards",
    });
    await wait(60);
    folded = true;
    await measureFrame();
    await wait(FOLD_MS);

    page = next;
    remember(next);
    await tick();
    const fresh = parts();
    if (fresh.panel) fresh.panel.style.clipPath = "inset(0 0 100% 0)";
    for (const el of fresh.rest) el.style.opacity = "0";
    await measureFrame();
    await wait(MOVE_MS);

    folded = false;
    await measureFrame();
    if (fresh.panel) {
      const panel = fresh.panel;
      panel.style.clipPath = "";
      panel.animate([{ clipPath: "inset(0 0 100% 0)" }, { clipPath: "inset(0 0 0 0)" }], { duration: FOLD_MS, easing: FOLD_EASING });
    }
    await wait(FOLD_MS - 60);
    for (const el of fresh.rest) {
      el.style.opacity = "";
      el.animate([{ opacity: 0 }, { opacity: 1 }], { duration: FADE_MS + 40 });
    }
    switching = false;
    if (queued && queued !== page) {
      const again = queued;
      queued = null;
      void switchTo(again);
    }
    queued = null;
  }

  function storedPage(): Page {
    try {
      const p = localStorage.getItem("micdeck.page") as Page | null;
      return p && PAGES.includes(p) ? p : "overview";
    } catch {
      return "overview";
    }
  }

  function go(next: Page) {
    if (switching) {
      queued = next;
    } else if (next !== page) {
      void switchTo(next);
    }
  }

  function remember(next: Page) {
    try {
      localStorage.setItem("micdeck.page", next);
    } catch {
      // Remembering the page is a convenience only.
    }
  }

  // In stereo mode the receiver puts transmitter 1 left and 2 right.
  const channelLabels = $derived.by(() => {
    const stereo = snapshot?.receiver?.settings["stereo"] === "stereo";
    return stereo ? ["Sender 1 · L", "Sender 2 · R"] : ["Links", "Rechts"];
  });

  const worst = $derived.by((): CheckLevel => {
    const levels = snapshot?.checks.map((c) => c.level) ?? [];
    return levels.includes("error") ? "error" : levels.includes("warn") ? "warn" : "ok";
  });

  const actions = {
    toggleMute: () => void backend?.setMuted(!snapshot?.audio.source?.muted),
    startMicTest: () => void backend?.startMicTest(),
    stopMicTest: () => void backend?.stopMicTest(),
    setSetting: (id: string, value: string) => void backend?.setDeviceSetting(id, value),
  };

  async function setAppSetting(setting: AppSettingName, on: boolean) {
    if (backend) appSettings = await backend.setAppSetting(setting, on);
  }

  onMount(() => {
    let closed = false;
    const unlisten: Array<() => void> = [];
    const reportVisibility = () => {
      void backend?.setMeterWanted(document.visibilityState === "visible");
    };
    const clock = window.setInterval(() => (now = Date.now()), 1000);
    // Below the designed width everything shrinks together instead of
    // cutting text off.
    const fit = () => {
      document.documentElement.style.setProperty("zoom", String(Math.min(1, Math.max(0.6, window.innerWidth / DESIGN_WIDTH))));
    };
    fit();
    window.addEventListener("resize", fit);
    window.addEventListener("resize", measureFrame);

    (async () => {
      try {
        backend = await connect();
        unlisten.push(await backend.onSnapshot((s) => (snapshot = s)));
        unlisten.push(await backend.onLevel((f) => (level = f)));
        unlisten.push(await backend.onAppSettings((s) => (appSettings = s)));
        snapshot = await backend.initial();
        appSettings = await backend.appSettings();
        images = await backend.deviceImages();
        const asked = (await backend.startPage()) as Page | null;
        if (asked && PAGES.includes(asked)) page = asked;
        if (closed) return;
        reportVisibility();
        document.addEventListener("visibilitychange", reportVisibility);
      } catch (e) {
        failure = e instanceof Error ? e.message : String(e);
      }
    })();

    return () => {
      closed = true;
      window.clearInterval(clock);
      window.removeEventListener("resize", fit);
      window.removeEventListener("resize", measureFrame);
      observer?.disconnect();
      unlisten.forEach((u) => u());
      document.removeEventListener("visibilitychange", reportVisibility);
      void backend?.setMeterWanted(false);
    };
  });
</script>

<div class="shell">
  <main bind:this={mainEl} style:--sheet={page === "overview" ? "var(--surface)" : "var(--sheet-sub)"}>
    {#if frame}
      <div class="frame" style:top="{frame.top}px" style:height="{frame.height}px"></div>
    {/if}
    <header class="top" bind:this={header}>
      {#if tab}
        <div
          class="tab"
          style:left="{tab.left}px"
          style:top="{tab.top}px"
          style:width="{tab.width}px"
          style:height="{tab.height}px"
          style:--neck="{tab.height - TAB_OVERLAP}px"
          style:border-radius={tab.radius}
          class:at-left={tab.left === 0}
          class:open={!folded}
        ></div>
      {/if}
      <Nav {page} diagnose={worst} muted={snapshot?.audio.source?.muted === true} onselect={go} />
      {#if snapshot?.simulated}
        <div class="demo" role="status" title="{snapshot.demoLabel ?? 'Simulierte Daten'} – keine echten Messwerte">
          <strong>Demo</strong>
          <span>{snapshot.demoLabel ?? "Simulierte Daten"}</span>
        </div>
      {/if}
    </header>

    {#if failure}
      <section class="card">
        <h3 class="label-caps">Fehler</h3>
        <p class="muted">{failure}</p>
      </section>
    {:else if snapshot}
      {#if page === "overview"}
        <Overview {snapshot} {level} {images} labels={channelLabels} {now} {actions} {go} />
      {:else if page === "device"}
        <Device {snapshot} {images} {now} onchange={actions.setSetting} />
      {:else if page === "diagnose"}
        <Diagnose {snapshot} />
      {:else}
        <AppPage settings={appSettings} onchange={setAppSetting} />
      {/if}
    {:else}
      <p class="muted">Starte …</p>
    {/if}
  </main>
</div>

<style>
  .shell {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
  }
  main {
    position: relative;
    z-index: 0;
    min-width: 0;
    overflow-y: auto;
    padding: 16px 26px 22px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .frame {
    position: absolute;
    z-index: -1;
    /* As wide as the panels: no strips beside or below them. */
    left: 26px;
    right: 26px;
    border-radius: 22px;
    background: #1c1e21;
    transition:
      height 240ms cubic-bezier(0.3, 0.7, 0.2, 1),
      top 240ms ease;
  }
  /* Index-card tab: exactly the panel colour and drawn behind the panel, so
     tab and panel read as one sheet. Small curves join it to the panel top. */
  .tab {
    position: absolute;
    z-index: -1;
    background: var(--sheet);
    pointer-events: none;
    transition:
      left 220ms cubic-bezier(0.3, 0.7, 0.2, 1),
      width 220ms cubic-bezier(0.3, 0.7, 0.2, 1),
      top 220ms cubic-bezier(0.3, 0.7, 0.2, 1),
      height 240ms cubic-bezier(0.3, 0.7, 0.2, 1),
      border-radius 220ms,
      background-color 220ms;
  }
  .tab:not(.open)::before,
  .tab:not(.open)::after {
    display: none;
  }
  .tab::before,
  .tab::after {
    content: "";
    position: absolute;
    top: calc(var(--neck) - 12px);
    width: 12px;
    height: 12px;
  }
  .tab::before {
    left: -12px;
    background: radial-gradient(circle at 0 0, transparent 11.5px, var(--sheet) 12px);
  }
  .tab::after {
    right: -12px;
    background: radial-gradient(circle at 100% 0, transparent 11.5px, var(--sheet) 12px);
  }
  .tab.at-left::before {
    display: none;
  }
  /* The first panel continues the tab: flat panel colour, no top edge. */
  main :global(.page > :first-child) {
    background: var(--sheet);
    border-top-color: transparent;
    /* The frame gives the outer shape; the panel's own top corners would
       show as extra curves. */
    border-top-left-radius: 0;
    border-top-right-radius: 0;
  }
  .top {
    position: relative;
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 44px;
    padding: 0 6px;
    /* Closer to the panel below than the page's usual spacing. */
    margin-bottom: -10px;
  }
  .demo {
    margin-left: auto;
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 13px;
    color: var(--text-2);
    border: 1px solid color-mix(in srgb, var(--warn) 45%, transparent);
    background: color-mix(in srgb, var(--warn) 8%, transparent);
    border-radius: 999px;
    padding: 4px 12px 4px 6px;
    min-width: 0;
  }
  .demo strong {
    color: #1d1600;
    background: var(--warn);
    border-radius: 999px;
    padding: 1px 8px;
    font-size: 12px;
  }
  .demo span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .muted {
    color: var(--text-2);
    margin: 0;
  }
  @media (max-width: 820px) {
    main {
      padding: 16px 18px 24px;
    }
  }
</style>
