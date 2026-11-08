<script lang="ts">
  import type { SettingOption } from "../types";

  let {
    options,
    value,
    disabled = false,
    tone = "plain",
    label,
    onchange,
  }: {
    options: SettingOption[];
    value: string | null;
    disabled?: boolean;
    /** `nc`: the selected option in DJI-style yellow with a waveform. */
    tone?: "plain" | "nc";
    label: string;
    onchange?: (value: string) => void;
  } = $props();
</script>

<div class="seg tone-{tone}" role="radiogroup" aria-label={label} class:disabled>
  {#each options as option (option.value)}
    <button
      role="radio"
      aria-checked={value === option.value}
      class:selected={value === option.value}
      {disabled}
      onclick={() => value !== option.value && onchange?.(option.value)}
    >
      {#if tone === "nc"}
        <svg viewBox="0 0 28 14" aria-hidden="true">
          {#if option.value === "strong"}
            <path d="M1 7h3l1.5-3 2 6 2-8 2 10 1.5-5H15" /><rect x="16" y="2.5" width="4" height="9" rx="2" /><path d="M21 7h6" />
          {:else}
            <path d="M1 7h3l1.5-3 2 6 2-8 2 10 1.5-5H15" /><rect x="16" y="2.5" width="4" height="9" rx="2" /><path d="M21 7c1-2 2-2 3 0s2 2 3 0" />
          {/if}
        </svg>
      {/if}
      {option.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    flex: none;
    display: inline-flex;
    gap: 4px;
    padding: 4px;
    border-radius: 12px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  button {
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    min-width: 74px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 6px 14px;
    border-radius: 9px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    transition: background 140ms, color 140ms;
  }
  button:hover:not(:disabled):not(.selected) {
    color: var(--text);
    background: rgba(255, 255, 255, 0.04);
  }
  button.selected {
    background: #f1f3f5;
    color: #0c0e10;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
  }
  .tone-nc button.selected {
    background: var(--nc);
    color: var(--nc-ink);
  }
  button:disabled {
    cursor: default;
  }
  .disabled {
    opacity: 0.45;
  }
  svg {
    width: 26px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
