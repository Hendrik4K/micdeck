<script lang="ts">
  // Grey frosted glass over things that need the USB-C connection (status data,
  // device settings) while the receiver hangs on the 3.5 mm jack. A lock is
  // laser-engraved into the glass; hovering swaps it for "USB-C exklusiv".
  // `chip`: a small pill in a row instead of a cover over a whole area.
  let { chip = false }: { chip?: boolean } = $props();
</script>

<div class="glass" class:chip role="img" aria-label="Nur über USB-C verfügbar">
  <svg class="lock" viewBox="0 0 24 24" aria-hidden="true">
    <!-- light edge below, dark cut above: reads as engraved -->
    <g class="lit">
      <path d="M7.5 10.5V8a4.5 4.5 0 0 1 9 0v2.5" />
      <rect x="5" y="10.5" width="14" height="10" rx="2.2" />
      <path d="M12 14.4v2.6" />
    </g>
    <g class="cut">
      <path d="M7.5 10.5V8a4.5 4.5 0 0 1 9 0v2.5" />
      <rect x="5" y="10.5" width="14" height="10" rx="2.2" />
      <path d="M12 14.4v2.6" />
    </g>
  </svg>
  <span class="text">USB-C exklusiv</span>
</div>

<style>
  .glass {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: grid;
    place-items: center;
    border-radius: inherit;
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.07), rgba(255, 255, 255, 0.015) 55%),
      rgba(120, 126, 136, 0.16);
    -webkit-backdrop-filter: blur(7px) saturate(0.35);
    backdrop-filter: blur(7px) saturate(0.35);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.1),
      inset 0 0 0 1px rgba(255, 255, 255, 0.05);
    cursor: default;
    overflow: hidden;
  }
  .glass > * {
    grid-area: 1 / 1;
    transition: opacity 220ms ease, transform 220ms ease;
  }
  .lock {
    width: 46px;
    height: 46px;
    fill: none;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .lock g {
    stroke-width: 1.7;
  }
  .lock .lit {
    stroke: rgba(255, 255, 255, 0.2);
    transform: translateY(0.9px);
  }
  .lock .cut {
    stroke: rgba(8, 10, 13, 0.62);
  }
  .text {
    opacity: 0;
    transform: scale(0.96);
    font-size: 17px;
    font-weight: 800;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    white-space: nowrap;
    /* engraved: dark fill, light edge below, dark edge above */
    color: rgba(10, 12, 16, 0.6);
    text-shadow:
      0 1px 0 rgba(255, 255, 255, 0.2),
      0 -1px 0 rgba(0, 0, 0, 0.3);
  }
  .glass:hover .lock {
    opacity: 0;
    transform: scale(0.9);
  }
  .glass:hover .text {
    opacity: 1;
    transform: none;
  }

  /* Small pill in a list row: lighter glass (nothing behind it to blur), the
     text sits over the lock so the pill only widens on hover. */
  .chip {
    position: relative;
    inset: auto;
    display: inline-grid;
    width: 50px;
    height: 24px;
    border-radius: 999px;
    background:
      linear-gradient(170deg, rgba(255, 255, 255, 0.16), rgba(255, 255, 255, 0.05) 60%),
      rgba(150, 156, 166, 0.2);
    transition: width 220ms ease;
  }
  .chip:hover {
    width: 124px;
  }
  .chip .lock {
    width: 16px;
    height: 16px;
  }
  .chip .lock g {
    stroke-width: 2.2;
  }
  .chip .lock .lit {
    stroke: rgba(255, 255, 255, 0.28);
  }
  .chip .lock .cut {
    stroke: rgba(6, 8, 11, 0.78);
  }
  .chip .text {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: 10.5px;
    letter-spacing: 0.1em;
    color: rgba(6, 8, 11, 0.72);
    text-shadow: 0 1px 0 rgba(255, 255, 255, 0.26);
  }
</style>
