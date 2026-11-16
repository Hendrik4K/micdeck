<script lang="ts">
  import type { SettingOption } from "../types";

  let {
    options,
    value,
    disabled = false,
    tone = "plain",
    stretch = false,
    label,
    onchange,
  }: {
    options: SettingOption[];
    value: string | null;
    disabled?: boolean;
    /** `nc`: the selected option in DJI-style yellow with a waveform. */
    tone?: "plain" | "nc";
    /** Fill the available width, options equally wide. */
    stretch?: boolean;
    label: string;
    onchange?: (value: string) => void;
  } = $props();

  // One highlight that slides to the chosen option instead of jumping.
  const index = $derived(options.findIndex((o) => o.value === value));
</script>

<div
  class="seg tone-{tone}"
  class:stretch
  role="radiogroup"
  aria-label={label}
  class:disabled
  style:--count={options.length}
  style:--index={Math.max(0, index)}
>
  {#if index >= 0}<span class="thumb" aria-hidden="true"></span>{/if}
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
    --pad: 4px;
    --gap: 4px;
    position: relative;
    flex: none;
    display: inline-grid;
    grid-template-columns: repeat(var(--count), minmax(74px, 1fr));
    gap: var(--gap);
    padding: var(--pad);
    border-radius: 12px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .stretch {
    display: grid;
    width: 100%;
  }
  .thumb {
    position: absolute;
    top: var(--pad);
    bottom: var(--pad);
    left: var(--pad);
    width: calc((100% - 2 * var(--pad) - (var(--count) - 1) * var(--gap)) / var(--count));
    transform: translateX(calc(var(--index) * (100% + var(--gap))));
    border-radius: 9px;
    background: #f1f3f5;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
    transition:
      transform 320ms cubic-bezier(0.3, 0.7, 0.2, 1),
      background 200ms;
  }
  .tone-nc .thumb {
    background: var(--nc);
    box-shadow: 0 1px 10px rgba(245, 210, 58, 0.25);
  }
  button {
    position: relative;
    font: inherit;
    font-size: 14px;
    font-weight: 600;
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
    transition: color 240ms;
  }
  button:hover:not(:disabled):not(.selected) {
    color: var(--text);
  }
  button.selected {
    color: #0c0e10;
  }
  .tone-nc button.selected {
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
