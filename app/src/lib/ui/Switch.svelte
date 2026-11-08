<script lang="ts">
  let {
    checked,
    disabled = false,
    label,
    onchange,
  }: { checked: boolean; disabled?: boolean; label: string; onchange?: (on: boolean) => void } = $props();
</script>

<button
  class="switch"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange?.(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    flex: none;
    position: relative;
    width: 50px;
    height: 28px;
    padding: 0;
    border-radius: 999px;
    border: 0;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    cursor: pointer;
    transition: background 160ms;
  }
  .switch.on {
    background: var(--accent);
    box-shadow: 0 0 14px rgba(47, 227, 138, 0.25);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #d9dee4;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
    transition: transform 160ms ease;
  }
  .on .knob {
    transform: translateX(22px);
    background: #fff;
  }
  .switch:disabled {
    cursor: default;
    opacity: 0.4;
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .switch.on:disabled {
    background: color-mix(in srgb, var(--accent) 45%, var(--surface-3));
  }
</style>
