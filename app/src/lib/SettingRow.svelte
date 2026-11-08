<script lang="ts">
  import type { SettingView } from "./types";
  import Switch from "./ui/Switch.svelte";
  import Segmented from "./ui/Segmented.svelte";

  let {
    setting,
    busy,
    error = null,
    compact = false,
    hideReason = false,
    onchange,
  }: {
    setting: SettingView;
    /** Another change is running; MicDeck sends one at a time. */
    busy: boolean;
    /** Why the last change of this setting failed, shown for a few seconds. */
    error?: string | null;
    /** Overview card: no description. */
    compact?: boolean;
    /** The page already names the reason (one reason for every row). */
    hideReason?: boolean;
    onchange: (value: string) => void;
  } = $props();

  // The control moves at once; the receiver confirms within ~0.2 s. Should it
  // refuse, the control falls back to what the receiver reports.
  const shown = $derived(setting.pending ?? setting.value);
  const disabled = $derived(setting.locked !== null);
  const fixedLabel = $derived(setting.options.find((o) => o.value === setting.value)?.label ?? setting.value);
  // One change at a time; a click during the short wait is dropped, without
  // dimming every control for a blink.
  const change = (value: string) => {
    if (!busy) onchange(value);
  };
</script>

<div class="row" class:locked={setting.locked !== null} class:compact>
  <div class="text">
    <span class="label">{setting.label}</span>
    {#if !compact}<span class="desc">{setting.description}</span>{/if}
    {#if setting.locked && !hideReason}
      <span class="why">{setting.locked}</span>
    {/if}
    {#if error}
      <span class="state err">{error}</span>
    {/if}
  </div>
  <div class="control">
    {#if setting.fixed}
      <span class="fixed">{fixedLabel}</span>
    {:else if setting.switch}
      <Switch checked={shown === "on"} {disabled} label={setting.label} onchange={(on) => change(on ? "on" : "off")} />
    {:else}
      <Segmented
        options={setting.options}
        value={shown}
        {disabled}
        tone={setting.id === "noise-cancel" ? "nc" : "plain"}
        label={setting.label}
        onchange={change}
      />
    {/if}
    {#if setting.unsupported}<span class="tag">nicht steuerbar</span>{/if}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 14px 0;
    border-top: 1px solid var(--line);
  }
  .row:first-child {
    border-top: 0;
    padding-top: 2px;
  }
  .compact {
    padding: 11px 0;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .label {
    font-size: 15.5px;
    font-weight: 600;
  }
  .desc {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .locked .label {
    color: var(--text-2);
  }
  .locked .desc {
    color: var(--text-3);
  }
  .why {
    font-size: 12px;
    color: var(--text-3);
  }
  .state {
    font-size: 12px;
    font-weight: 600;
  }
  .state.err {
    color: var(--error);
  }
  .fixed {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    padding: 5px 14px;
    border-radius: 9px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .control {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
  }
</style>
