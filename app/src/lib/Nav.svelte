<script lang="ts" module>
  export type Page = "overview" | "device" | "diagnose" | "app";
</script>

<script lang="ts">
  import type { CheckLevel } from "./types";

  let {
    page,
    diagnose,
    muted = false,
    onselect,
  }: {
    page: Page;
    /** The DJI input is muted: MicDeck turns yellow. */
    muted?: boolean;
    /** Worst check level, marked on "Diagnose". */
    diagnose: CheckLevel;
    onselect: (page: Page) => void;
  } = $props();

  // MicDeck itself is the main view; the others open to its right like a submenu.
  const items: { id: Page; label: string; icon: string }[] = [
    { id: "device", label: "Gerät", icon: "M5 7h9M18 7h1M5 17h3M12 17h7M16 4.5v5M10 14.5v5" },
    { id: "diagnose", label: "Diagnose", icon: "M3 12h4l2.5-6 4 12 2.5-6H21" },
    { id: "app", label: "App", icon: "M12 9.2a2.8 2.8 0 1 0 0 5.6 2.8 2.8 0 0 0 0-5.6zM12 3v2.2M12 18.8V21M5.6 5.6l1.6 1.6M16.8 16.8l1.6 1.6M3 12h2.2M18.8 12H21M5.6 18.4l1.6-1.6M16.8 7.2l1.6-1.6" },
  ];
</script>

<nav aria-label="Seiten">
  <button class="main" class:muted class:active={page === "overview"} aria-current={page === "overview" ? "page" : undefined} onclick={() => onselect("overview")}>
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="8.5" y="2.5" width="7" height="12.5" rx="3.5" />
      <path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3.5" />
    </svg>
    <span>MicDeck</span>
  </button>
  <div class="sub">
    {#each items as item, i (item.id)}
      {#if i > 0}<span class="divider" aria-hidden="true"></span>{/if}
      <button class:active={page === item.id} aria-current={page === item.id ? "page" : undefined} onclick={() => onselect(item.id)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d={item.icon} /></svg>
        <span>{item.label}</span>
        {#if item.id === "diagnose" && (diagnose === "warn" || diagnose === "error")}
          <i class="badge {diagnose}" title="Es gibt Hinweise"></i>
        {/if}
      </button>
    {/each}
  </div>
</nav>

<style>
  /* Sits on the shared dark frame (App.svelte). The active page gets an
     index-card tab drawn behind it (App.svelte); MicDeck keeps the card
     colour even when another page is open. */
  nav {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 52px;
  }
  button {
    position: relative;
    z-index: 2;
  }
  .main {
    flex: none;
    align-self: stretch;
    display: inline-flex;
    align-items: center;
    gap: 9px;
    padding: 0 28px 0 20px;
    font: inherit;
    font-size: 16px;
    font-weight: 750;
    color: var(--text);
    border: 0;
    /* Concentric with the frame's 22 px corner, 6 px inside it. */
    border-radius: 16px;
    background: var(--surface);
    cursor: pointer;
    transition: color 160ms;
  }
  .main svg {
    width: 20px;
    height: 20px;
    stroke: var(--accent);
  }
  .main.muted,
  .main.muted.active,
  .main.muted svg {
    color: var(--warn);
    stroke: var(--warn);
  }
  /* Active: the tab behind it takes over. */
  .main.active {
    color: var(--accent);
    background: transparent;
    box-shadow: none;
  }
  /* Evenly spread, thin dividers, the chosen page as a glowing pill. */
  .sub {
    flex: 1;
    display: flex;
    align-items: center;
    padding-right: 6px;
  }
  .sub {
    align-self: stretch;
  }
  .sub button {
    flex: 1;
    align-self: stretch;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    font: inherit;
    font-size: 14.5px;
    font-weight: 600;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    transition: background 160ms, color 160ms, box-shadow 160ms;
  }
  .sub button:hover:not(.active) {
    color: var(--text);
  }
  .sub button.active {
    color: var(--accent);
  }
  .divider {
    flex: none;
    width: 1px;
    height: 18px;
    margin: 0 4px;
    background: var(--line-2);
  }
  svg {
    flex: none;
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .badge {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .badge.warn {
    background: var(--warn);
  }
  .badge.error {
    background: var(--error);
  }
  @media (max-width: 820px) {
    .sub span {
      display: none;
    }
  }
</style>
