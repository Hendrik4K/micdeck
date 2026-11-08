<script lang="ts">
  import type { AppSettingName, AppSettings } from "../lib/types";
  import Switch from "../lib/ui/Switch.svelte";

  let {
    settings,
    onchange,
  }: { settings: AppSettings | null; onchange: (setting: AppSettingName, on: boolean) => void } = $props();

  const rows: { id: AppSettingName; label: string; description: string }[] = [
    {
      id: "notifications",
      label: "Benachrichtigungen",
      description: "Meldet sich, wenn das Mikrofon nicht mehr bereit ist, und wenn es wieder geht.",
    },
    {
      id: "dockDictation",
      label: "Diktat über das Ladecase (Claude)",
      description:
        "Sender 1 aus dem Ladecase nehmen startet das Diktat in der Claude-App. Zurücklegen beendet es und schickt die Nachricht ab.",
    },
    {
      id: "dockMute",
      label: "Im Ladecase stumm",
      description:
        "Schaltet den DJI-Eingang stumm, solange Sender 1 im Ladecase liegt. Will ein Programm aufnehmen, etwa der Browser, gibt MicDeck das Mikrofon dafür kurz frei, meldet das und schaltet danach wieder stumm.",
    },
    {
      id: "autostart",
      label: "Beim Anmelden starten",
      description: "Startet MicDeck nach dem Anmelden im Hintergrund, nur mit dem Symbol im Panel.",
    },
  ];
</script>

<div class="page">
  <section class="card">
    <h3 class="label-caps">MicDeck</h3>
    {#if settings}
      {#each rows as row (row.id)}
        {@const locked = row.id === "autostart" && !settings.autostartAvailable}
        <div class="row" class:locked>
          <div class="text">
            <span class="label">{row.label}</span>
            <span class="desc">{row.description}</span>
            {#if locked}<span class="why">Nur in der installierten App, nicht im Entwicklungsmodus.</span>{/if}
          </div>
          <Switch checked={settings[row.id]} disabled={locked} label={row.label} onchange={(on) => onchange(row.id, on)} />
        </div>
      {/each}
      <p class="note">Dieselben Schalter findest du im Menü des Panel-Symbols.</p>
    {:else}
      <p class="note">Lade …</p>
    {/if}
  </section>

  <section class="card about">
    <h3 class="label-caps">Über</h3>
    <p><b>MicDeck {__APP_VERSION__}</b> · Begleit-App für das DJI Mic Mini unter Linux</p>
    <p>
      Läuft komplett lokal: keine Konten, keine Cloud, keine Telemetrie. Ton wird nur für die Pegelanzeige und den
      Mikrofontest im Arbeitsspeicher verarbeitet und nie gespeichert.
    </p>
    <p class="small">Kein offizielles DJI-Produkt. Lizenz: MIT oder Apache-2.0.</p>
  </section>
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 14px 0;
    border-top: 1px solid var(--line);
  }
  .row:first-of-type {
    border-top: 0;
    padding-top: 2px;
  }
  .text {
    flex: 1;
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
  .why {
    font-size: 12px;
    color: var(--text-3);
  }
  .note {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .about p {
    margin: 0 0 8px;
    font-size: 14px;
    color: var(--text-2);
  }
  .about b {
    color: var(--text);
  }
  .about .small {
    font-size: 12.5px;
    color: var(--text-3);
    margin: 0;
  }
</style>
