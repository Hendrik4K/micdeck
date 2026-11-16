<script lang="ts">
  // The button names what a click does, not the current state: "Muten" in
  // red while the microphone is live, "Mikrofon an" in green while muted.
  let { muted, onclick }: { muted: boolean; onclick: () => void } = $props();
</script>

<button
  class="mute"
  class:unmute={muted}
  {onclick}
  title={muted ? "Stummschaltung aufheben" : "Stummschalten – auch am Empfänger selbst"}
>
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <rect x="8.5" y="3" width="7" height="11.5" rx="3.5" />
    <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
    {#if !muted}<path d="M3.5 3.5l17 17" />{/if}
  </svg>
  <span>{muted ? "Mikrofon an" : "Muten"}</span>
</button>

<style>
  .mute {
    --c: var(--error);
    flex: none;
    width: 86px;
    height: 86px;
    border-radius: 50%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 1px;
    font: inherit;
    font-size: 11px;
    letter-spacing: -0.01em;
    font-weight: 650;
    white-space: nowrap;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 8%, transparent);
    border: 2px solid var(--c);
    box-shadow: 0 0 18px color-mix(in srgb, var(--c) 22%, transparent);
    cursor: pointer;
    transition: background 140ms, color 300ms, border-color 300ms, box-shadow 300ms;
  }
  .mute.unmute {
    --c: var(--accent);
  }
  .mute:hover {
    background: color-mix(in srgb, var(--c) 16%, transparent);
  }
  svg {
    width: 28px;
    height: 28px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
  }
</style>
