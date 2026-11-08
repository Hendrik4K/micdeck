<script lang="ts">
  import type { Check } from "./types";

  let { checks }: { checks: Check[] } = $props();
  /** Two columns, filled top to bottom. */
  const rows = $derived(Math.ceil(checks.length / 2));
</script>

<section class="card">
  <h3 class="label-caps">Prüfungen</h3>
  <ul style:grid-template-rows="repeat({rows}, auto)">
    {#each checks as check, i (check.id)}
      <li class="level-{check.level}" class:column-top={i === rows}>
        <span class="mark" aria-hidden="true">
          {#if check.level === "ok"}
            <svg viewBox="0 0 16 16"><path d="M3.5 8.5l3 3 6-7" /></svg>
          {:else if check.level === "warn"}
            <svg viewBox="0 0 16 16"><path d="M8 4v5M8 11.6v.1" /></svg>
          {:else if check.level === "error"}
            <svg viewBox="0 0 16 16"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7" /></svg>
          {:else if check.level === "pending"}
            <svg viewBox="0 0 16 16"><path d="M4 8h.1M8 8h.1M12 8h.1" /></svg>
          {:else}
            <svg viewBox="0 0 16 16"><path d="M5 8h6" /></svg>
          {/if}
        </span>
        <div class="body">
          <div class="line">
            <span class="label">{check.label}</span>
            <span class="value">{check.value}</span>
          </div>
          {#if check.hint && (check.level === "warn" || check.level === "error")}
            <p class="hint">{check.hint}</p>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
</section>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-auto-flow: column;
    column-gap: 40px;
    row-gap: 2px;
  }
  li {
    display: flex;
    gap: 10px;
    padding: 6px 0;
    font-size: 13.5px;
    border-top: 1px solid var(--line);
    --tone: var(--text-3);
  }
  li:first-child,
  li.column-top {
    border-top: 0;
  }
  .level-ok { --tone: var(--ok); }
  .level-warn { --tone: var(--warn); }
  .level-error { --tone: var(--error); }
  .level-pending { --tone: var(--pending); }

  .mark {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--tone) 16%, transparent);
    margin-top: 1px;
  }
  svg {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: var(--tone);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: baseline;
  }
  .label {
    color: var(--text-2);
    white-space: nowrap;
  }
  .value {
    text-align: right;
    font-weight: 550;
  }
  .level-inactive .value {
    color: var(--text-3);
    font-weight: 400;
  }
  .hint {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--text-2);
    user-select: text;
  }
</style>
