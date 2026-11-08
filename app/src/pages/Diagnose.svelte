<script lang="ts">
  import type { ReceiverView, Snapshot, WriteLogEntry } from "../lib/types";
  import CheckList from "../lib/CheckList.svelte";

  let { snapshot }: { snapshot: Snapshot } = $props();

  const linkText = (r: ReceiverView): string => {
    switch (r.link.kind) {
      case "open":
        return r.telemetryLive ? "Offen, Statusdaten kommen an" : "Offen, wartet auf Statusdaten";
      case "opening":
        return "Wird geöffnet …";
      case "permissionDenied":
        return "Keine Berechtigung (udev-Regel fehlt)";
      case "busy":
        return "Von einem anderen Programm belegt";
      case "failed":
        return `Gestört: ${r.link.message}`;
      case "unsupported":
        return "Modell unbekannt, nur USB und Audio prüfbar";
    }
  };
  const time = (ms: number) => new Date(ms).toLocaleTimeString("de-DE");
  const tone = (e: WriteLogEntry) => (e.ok === null ? "busy" : e.ok ? "ok" : "err");
  const receiver = $derived(snapshot.receiver);
</script>

<div class="page">
  <CheckList checks={snapshot.checks} />
  <div class="columns">
    <div class="left">
      <section class="card">
        <h3 class="label-caps">Technik</h3>
        <dl>
          <dt>Empfänger</dt>
          <dd>{receiver ? `${receiver.name} · USB ${receiver.usbId} · Port ${receiver.key}` : "Nicht angeschlossen"}</dd>
          {#if receiver}
            <dt>Steuerschnittstelle</dt>
            <dd>{linkText(receiver)}</dd>
            <dt>Soundkarte</dt>
            <dd class="num">{receiver.alsaCards.length ? receiver.alsaCards.map((c) => `ALSA-Karte ${c}`).join(", ") : "Noch keine"}</dd>
          {/if}
          <dt>Audiosystem</dt>
          <dd>
            {snapshot.audio.service.kind === "available"
              ? "PipeWire verbunden"
              : snapshot.audio.service.kind === "connecting"
                ? "Verbinde mit PipeWire …"
                : `PipeWire nicht erreichbar: ${snapshot.audio.service.reason}`}
          </dd>
          <dt>Standard-Mikrofon</dt>
          <dd>{snapshot.audio.defaultInput ? `${snapshot.audio.defaultInput.description}${snapshot.audio.defaultInput.isDji ? "" : " (anderes Gerät)"}` : "Keins festgelegt"}</dd>
          <dt>Nimmt gerade auf</dt>
          <dd>{snapshot.audio.source?.captureApps.length ? snapshot.audio.source.captureApps.join(", ") : "Kein anderes Programm"}</dd>
          <dt>DJI-Eingang</dt>
          <dd class="mono">{snapshot.audio.source?.nodeName ?? "–"}</dd>
          <dt>Pegelmessung</dt>
          <dd>
            {snapshot.meter.kind === "running"
              ? `Läuft · ${snapshot.meter.channels} Kanäle · ${snapshot.meter.rate / 1000} kHz`
              : snapshot.meter.kind === "failed"
                ? `Fehlgeschlagen: ${snapshot.meter.message}`
                : snapshot.meter.kind === "starting"
                  ? "Startet …"
                  : "Aus (nur während das Fenster sichtbar ist)"}
          </dd>
          {#if snapshot.usbWatchError}
            <dt>USB-Überwachung</dt>
            <dd>{snapshot.usbWatchError}</dd>
          {/if}
          {#if snapshot.otherReceivers > 0}
            <dt>Weitere Empfänger</dt>
            <dd>{snapshot.otherReceivers} weitere angeschlossen, angezeigt wird der erste</dd>
          {/if}
        </dl>
      </section>
    </div>

    <section class="card protocol">
      <h3 class="label-caps">Geräte-Protokoll</h3>
      {#if snapshot.deviceLog.length}
        <ol class="log">
          {#each snapshot.deviceLog as entry (entry.request)}
            <li class={tone(entry)}>
              <span class="dot" aria-hidden="true"></span>
              <div class="what">
                <div class="line">
                  <b>{entry.label}</b>
                  <span class="change">{entry.from ?? "?"} → {entry.to}</span>
                  <span class="time num">{time(entry.unixMs)}</span>
                </div>
                <span class="summary">{entry.summary}</span>
                {#if entry.packet}
                  <code title="Befehl wie gesendet">#{entry.seq} · {entry.packet}</code>
                {/if}
              </div>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="empty">Noch keine Änderung am Gerät. MicDeck sendet nur, wenn du eine Einstellung umschaltest.</p>
      {/if}
      <p class="file">Die letzten 30 Minuten, auch in <code>~/.local/state/micdeck/device-writes.log</code></p>
    </section>
  </div>

</div>

<style>
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 16px;
    align-items: stretch;
  }
  .left {
    display: grid;
    gap: 16px;
    align-content: start;
  }
  /* As tall as the left column: the list scrolls instead of growing the row. */
  .protocol {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .protocol .log {
    flex: 1 1 0;
    height: 0;
    min-height: 120px;
  }
  .log {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
  }
  .log li {
    display: flex;
    gap: 12px;
    padding: 10px 0;
    border-top: 1px solid var(--line);
    --tone: var(--text-3);
  }
  .log li:first-child {
    border-top: 0;
    padding-top: 0;
  }
  .log li.ok {
    --tone: var(--ok);
  }
  .log li.err {
    --tone: var(--error);
  }
  .log li.busy {
    --tone: var(--warn);
  }
  .dot {
    flex: none;
    margin-top: 6px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--tone);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
    font-size: 14px;
  }
  .line {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .change {
    color: var(--text-2);
  }
  .time {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-3);
  }
  .summary {
    font-size: 13px;
    color: var(--tone);
  }
  code {
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 11.5px;
    color: var(--text-3);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .empty,
  .file {
    margin: 0;
    font-size: 13px;
    color: var(--text-3);
  }
  .file {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: 150px 1fr;
    gap: 8px 18px;
    font-size: 14px;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    color: var(--text-2);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 12.5px;
    user-select: text;
  }
  @media (max-width: 820px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
