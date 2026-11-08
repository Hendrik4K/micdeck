# MicDeck

[English version: README.md](README.md)

Linux-Begleit-App für das DJI Mic Mini. Beantwortet auf einen Blick: Ist der Empfänger
angeschlossen, sind die Sender an, und kommt tatsächlich Ton am PC an?

> **Inoffiziell und auf eigene Gefahr.** MicDeck ist ein unabhängiges Hobbyprojekt, nicht von DJI oder
> Anthropic. Es kann Einstellungen an Empfänger und Sender ändern. Gesendet werden nur Befehle, die
> am echten Gerät getestet wurden, eine Gewährleistung gibt es aber nicht (siehe Lizenzen).

Stand: **0.1.10 – neue Oberfläche („Precision Desk“) und Geräteeinstellungen wie in der DJI-App.**
Was genau geprüft ist und was noch offen ist, steht im Abschnitt „Teststand“.

## Aufbau

```
crates/dji-protocol   DJI-Protokoll (aus DJI-Mic-Control übernommen, siehe THIRD_PARTY.md)
crates/micdeck-core   Zustand, Ampel-Logik, Pegel-Mathematik, Demo-Szenarien – ohne Hardwarezugriff
crates/usb-link       USB: Hotplug, Rechte/Belegt-Erkennung, Statusdaten lesen, Einstellungen schreiben
crates/audio-pw       PipeWire: Eingänge, Standard-Mikrofon, stumm, Pegelmessung im Arbeitsspeicher
app/src-tauri         Tauri-App, verbindet alles
app/src               Oberfläche (Svelte 5 + TypeScript)
packaging/            udev-Regel (noch nicht installiert)
docs/                 Protokoll, Screenshots
```

## Entwickeln

Voraussetzungen (Fedora 44, einmalig, mit Passwort):

```bash
sudo dnf install --repo=fedora --repo=updates webkit2gtk4.1-devel librsvg2-devel libxdo-devel pipewire-devel rust cargo rustfmt clippy
```

Rust ab 1.90 (Tauri 2.12 verlangt das; Fedora 44 hat 1.99). Die PipeWire-Bibliothek erzeugt ihre
Rust-Anbindung beim Bauen mit libclang. Ohne das Paket `clang` findet libclang seine eigenen Header
(`stdbool.h`) nicht. Deshalb trägt `.cargo/config.toml` den Pfad `/usr/lib/clang/22/include` ein.
Nach einem clang-Versionssprung muss die Zahl dort angepasst werden. Mit `sudo dnf install clang`
sollte die Zeile überflüssig werden (noch nicht getestet).

Dann im Ordner `MicDeck`:

```bash
pnpm -C app install
cargo test --workspace
pnpm -C app tauri dev
```

Demo-Modus (ohne Empfänger, deutlich als Demo markiert): `pnpm -C app tauri dev -- -- --demo`
oder ein einzelnes Szenario mit `--demo=no-transmitter` (Namen: `ready`, `no-transmitter`,
`silent`, `muted`, `not-default`, `clipping`, `low-battery`, `no-permission`, `busy`,
`unplugged`, `no-audio-input`, `pipewire-down`).

Nur die Oberfläche im Browser ansehen: `pnpm -C app dev` und `http://127.0.0.1:1420` öffnen
(Vorschaudaten, Zustand per `#ready`, `#no-tx`, `#unplugged`, `#no-permission`).

Fehlersuche: `RUST_LOG=info,usb_link=debug,audio_pw=debug` vor den Startbefehl setzen. Das zeigt
die gefundenen Empfänger, die PipeWire-Eingänge und das Standard-Mikrofon. `usb_link=trace` zeigt
zusätzlich jedes Statuspaket des Empfängers als Hex-Bytes.

## Bedienung

- **Fenster:** oben eine ovale Navigationsleiste über die ganze Breite. *MicDeck* ist die
  Hauptansicht (Hardware-Bühne, Audio-Monitor mit großem Stumm-Knopf und Mikrofontest,
  RX-Schnelleinstellungen), daneben *Gerät*, *Diagnose* und *App*. `micdeck --page=device` öffnet
  eine Seite direkt.
- **Eigene Gerätebilder:** MicDeck bringt nur Silhouetten mit. Lege `receiver.png` und
  `transmitter.png` (oder `.webp`/`.jpg`, am besten freigestellt mit transparentem Hintergrund,
  höchstens 10 MB) nach `~/.config/micdeck/devices/` und starte MicDeck neu. Dann erscheinen deine
  Bilder. Sie bleiben auf deinem Rechner.
- **Geräteeinstellungen:** Rauschminderung an/aus und Einfach/Stark, Audio-Kanal Mono/Stereo,
  Backup-Aufnahme, Hochpassfilter, Clipping-Kontrolle, automatische Abschaltung (Empfänger und
  Sender), Empfänger An/Aus mit Kamera, Einschalttaste für Rauschminderung, Mikrofon-LED. MicDeck
  sendet nur, wenn du umschaltest, immer einen Befehl nach dem anderen. Eine Änderung gilt erst als
  übernommen, wenn der Empfänger sie bestätigt **und** den neuen Wert zurückmeldet. Jeder Schritt
  steht 30 Minuten lang unter *Diagnose* im Geräte-Protokoll und in
  `~/.local/state/micdeck/device-writes.log`.
- **Gesperrt und gekennzeichnet:** Externer Lautsprecher ohne Stecker (startet den Empfänger neu und
  unterbricht den Ton, bitte in der DJI-App ändern), Klangfarbe (nur DJI Mic Mini 2), Verstärkung
  (nur am Drehrad, wird angezeigt), Firmware-Update (nur DJI-App). Dazu dieselben Regeln wie in der
  DJI-App: Backup-Aufnahme nur in Mono, Einfach/Stark nur bei eingeschalteter Rauschminderung,
  Sender-Einstellungen nur mit eingeschaltetem Sender, v2-Funktionen nur mit Firmware 02.
- **Panel-Symbol** (Tray): Ein farbiger Punkt zeigt den Status (grün, gelb, rot, grau = prüfe). Der
  Tooltip zeigt die Kurzbeschreibung. Ein Linksklick öffnet oder versteckt das Fenster.
- **Menü** (Rechtsklick): MicDeck öffnen · Stummschalten · Benachrichtigungen · Diktat über Ladecase ·
  Im Ladecase stumm · Beim Anmelden starten (nur in der installierten App) · Beenden. Dieselben
  Schalter stehen im Fenster auf der Seite *App*, beide Stellen bleiben synchron.
- **Fenster schließen** versteckt MicDeck ins Panel. Ohne Panel-Symbol beendet es die App wie gewohnt.
- **Pegelmessung** läuft nur, solange das Fenster offen ist.
- **Mikrofontest** (Link „Test starten“ unter der Pegelanzeige; solange er läuft, überdeckt er den
  Stumm-Knopf): Die Aufnahme beginnt, sobald du sprichst, und endet nach einer kurzen Pause
  (höchstens 15 s). Ein roter, blinkender Kreis zeigt die Aufnahme, sein Ring füllt sich mit der
  Sprechzeit. Danach erscheint ein grünes Play-Symbol, du hörst dich selbst, und der Ring läuft
  zurück. Sprichst du dazwischen, beginnt eine neue Aufnahme; ein Echo-Schutz verhindert, dass sich
  die Wiedergabe über Lautsprecher selbst unterbricht. Rechts beendet „Beenden“ bzw. „Zurück“ den
  Test, nach ein paar Sekunden Ruhe endet er von selbst. Aufnahmen liegen nur im Arbeitsspeicher
  und werden nie gespeichert.
- **Im Ladecase stumm** (Panel-Menü, standardmäßig aus): Sender 1 zurück ins Ladecase schaltet das
  Mikro stumm, Herausnehmen hebt die Stummschaltung auf. Läuft ein Diktat, wird erst abgeschickt und
  dann stummgeschaltet. Will währenddessen ein anderes Programm aufnehmen (etwa der Browser), gibt
  MicDeck das Mikro nur für diese Aufnahme frei, meldet das per Benachrichtigung und schaltet 2 s
  nach dem Ende wieder stumm. Eine Stummschaltung per Knopf hebt MicDeck dafür nie auf.
- **Stumm-Knopf** (großer runder Knopf unter der Pegelanzeige und im Panel-Menü): schaltet über WirePlumber stumm. Beim
  Mic Mini legt das den USB-Stummschalter im Empfänger selbst um („Mic Capture Switch“). Dann kommt bei
  keinem Programm mehr Ton an, auch nicht bei Programmen, die PipeWire umgehen. Hebt ein anderes
  Programm die Stummschaltung auf, warnt MicDeck. Das Panel-Symbol bekommt einen Schrägstrich.
- **Benachrichtigungen:** Wenn der Empfänger abgezogen wird, ein Sender länger als 3 s die Funkverbindung
  verliert oder die Ampel länger als 3 s rot ist, und zwar nur, wenn vorher schon einmal alles bereit
  war. Wenn danach alles wieder bereit ist, kommt einmal „Wieder einsatzbereit“.
- **Diktat über Ladecase (Claude)**, im Panel-Menü, standardmäßig aus: Nimmst du Sender 1 aus dem
  Ladecase, holt MicDeck das Claude-Fenster nach vorn und drückt Claudes Diktat-Kürzel Strg+D.
  Legst du ihn zurück, wartet MicDeck 1 s Nachlauf (für das Satzende), drückt Strg+D erneut, wartet
  2 s auf den letzten Text und schickt mit Enter ab. Das Panel-Symbol zeigt dabei: leuchtender
  Lime-Sonar-Ball (Diktat läuft) → Fortschrittsbalken (wird gesendet) → Häkchen (gesendet). Tasten gehen nur an das Claude-Fenster: Lässt es sich nicht nach vorn holen, passiert
  nichts, und eine Benachrichtigung nennt den Grund. Strg+D ist ein Umschalter. Wer zwischendurch von
  Hand stoppt, vertauscht Start und Stopp, und einmal Strg+D von Hand stellt es zurück. Hat Claude ein
  anderes Diktat-Kürzel eingestellt, funktioniert es nicht.
- **Einstellungen** liegen in `~/.config/micdeck/settings.json`. Der Autostart-Eintrag
  `~/.config/autostart/micdeck.desktop` existiert nur, solange er eingeschaltet ist. Er startet
  MicDeck mit `--hidden`, also nur im Panel.

## USB-Berechtigung (für Funkstatus, Akku, Einstellungen)

Ton und Pegel funktionieren ohne. Für die Statusdaten des Empfängers braucht MicDeck Zugriff auf
genau dieses USB-Gerät. `packaging/70-micdeck.rules` gibt ihn nur dem gerade am PC angemeldeten
Benutzer (`uaccess`), ohne `0666` und ohne zusätzliche Gruppe.

```bash
sudo install -m 0644 packaging/70-micdeck.rules /etc/udev/rules.d/70-micdeck.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=usb --attr-match=idVendor=2ca3
```

Rückgängig: `sudo rm /etc/udev/rules.d/70-micdeck.rules` und `sudo udevadm control --reload-rules`.

## Datenschutz

Keine Telemetrie, keine Cloud, keine Hintergrunddienste. Die Pegelmessung wertet Samples nur im
Arbeitsspeicher aus und läuft nur, solange eine Pegelanzeige sichtbar ist. Es wird nichts
aufgenommen oder gespeichert. Auf das Gerät schreibt MicDeck nur Einstellungen, die du selbst
umschaltest, und nie Firmware.

## Teststand

Stand 06.11.2026, Fedora 44, Rust 1.99, Tauri 2.12.5:

| Teil | Stand |
|---|---|
| Oberfläche: TypeScript-Prüfung, Build, Browser-Vorschau | ✅ geprüft |
| Rust-Code kompiliert, clippy ohne Fehler (4 Stilhinweise offen) | ✅ |
| Automatische Tests: 65 (Protokoll 31, Ampel-Szenarien 22, Pegel 5, PipeWire-Auswertung 4, sysfs 3) | ✅ alle bestanden |
| App im Demo-Modus: Fenster, Ereignisse, Pegel, alle 12 Szenarien, `--demo=<name>` | ✅ per Bildschirmfoto geprüft |
| App im Live-Modus ohne Empfänger: echte PipeWire-Eingänge und Standard-Mikrofon, „Nicht verbunden“ | ✅ geprüft |

Mit echtem DJI Mic Mini (Empfänger `2ca3:4011`, Firmware 01.01.00.56, Protokoll v1, ein Sender an):

| Teil | Stand |
|---|---|
| USB-Erkennung; udev-Regel gibt dem angemeldeten Benutzer Zugriff ohne root | ✅ |
| Nur Schnittstelle 6 belegt; Audio bleibt bei `snd-usb-audio`, HID bei `usbhid` | ✅ |
| Statusdaten: Firmware, Sender 1 verbunden, Sender 2 nicht verbunden, Funkpegel | ✅ |
| Akkustand | ❔ Protokoll v1 meldet keinen bekannten Wert; die Zeile wird dann ausgeblendet |
| Laden (Sender im Ladecase) | ✅ Bit `0x02` im Flag-Byte des Senders: Dock raus → `0x24`, rein → `0x26`; angezeigt als „Lädt (im Ladecase)“ |
| Lautstärke-Einordnung (geglättet, „Stille“ … „Sehr laut“) | ✅ angezeigt; Grenzen grob, mit Sprache noch nicht abgeglichen |
| DJI-Eingang in PipeWire erkannt; WirePlumber macht ihn beim Einstecken zum Standard | ✅ |
| Echter Pegel in dBFS, Ampel grün „Einsatzbereit“ | ✅ |
| Parallele Aufnahme über PipeWire (`pw-record`) und Pulse-API (`parec`) während MicDeck misst | ✅ (nur Bytes gezählt, nichts gespeichert) |
| Abziehen → sofort rot; Einstecken → nach 3 s grün, ohne Neustart | ✅ |
| Zwei Sender gleichzeitig, Sender verliert Funk, Stummschaltung am Gerät | ⏳ noch nicht getestet |
| Echte Programme (OBS, Discord) parallel | ⏳ noch nicht getestet |

Phase 4 (Panel, Benachrichtigungen, Autostart, Paket), Stand 06.11.2026 abends:

| Teil | Stand |
|---|---|
| Tray-Symbol bei Cinnamon angemeldet, Tooltip mit Status | ✅ per D-Bus geprüft |
| Klick aufs Symbol versteckt/zeigt das Fenster | ✅ per D-Bus-Aufruf `Activate` geprüft; echter Mausklick noch nicht von Hand bestätigt |
| Fenster schließen → ins Panel; Pegelmessung stoppt dabei, startet beim Öffnen wieder | ✅ |
| Logik der Benachrichtigungen (Abziehen, Funkverlust > 3 s, wieder bereit, nichts im Demo-Modus) | ✅ 4 Tests |
| Benachrichtigung tatsächlich auf dem Desktop | ⏳ braucht ein echtes Abziehen |
| Autostart-Eintrag | ✅ Pfad-Quoting getestet; Ein-/Ausschalten nur in der installierten App, noch nicht von Hand geprüft |
| Diktat über Ladecase: Sender raus → Diktat in Claude gestartet, zurück → gestoppt und abgeschickt, Text vollständig angekommen | ✅ 06.11.2026 am echten Gerät (Testversion) |
| Dock-Erkennung (kein Auslösen beim Start, beim Wackeln unter 0,5 s, im Demo-Modus) | ✅ 4 Tests |
| Panel-Symbol während des Diktats: Sonar → Balken → Häkchen → Mikrofon | ✅ am Gerät, Pixmaps über D-Bus mitgeschnitten |
| Stumm-Knopf: Hardware-Schalter im Empfänger geht auf „off“, Warnung bei fremdem Entstummen | ✅ am Gerät (Menü per D-Bus ausgelöst, Schalter per `amixer` gelesen) |
| Firmware 02.03.17.00 (Protokoll v2): Akkustand, „lädt“, Gain-Regler werden gelesen. Das Dock-Bit gilt auch bei **vollem** Akku (anders als v1), Ladecase-Diktat ausgelöst | ✅ am Gerät (06.11.2026, 23:28) |
| Mikrofon-Session: drei Aufnahmen hintereinander, Wiedergabe, automatisches Ende; Spracherkennung mit 4 Tests | ✅ am Gerät (06.11.2026, über Lautsprecher, Echo −40…−52 dBFS gemessen) |
| RPM-Paket (`target/release/bundle/rpm/`) installiert, App aus `/usr/bin/micdeck` läuft mit Empfänger, Tray und udev-Zugriff | ✅ 06.11.2026 |

Version 0.1.10 (neue Oberfläche, Geräteeinstellungen), Stand 07.11.2026, Firmware 02.03.17.00:

| Teil | Stand |
|---|---|
| Automatische Tests: 98, davon 13 für Sperrregeln und Änderungsverlauf, 2 für die Protokolldatei | ✅ alle bestanden |
| Alle 11 schaltbaren Einstellungen am Gerät umgeschaltet und zurückgestellt (`settings_roundtrip`): jede bestätigt und nach 0,1–0,2 s zurückgelesen, Originalwerte wiederhergestellt | ✅ am Gerät |
| Gesperrte Einstellung (Externer Lautsprecher) wird abgelehnt, ohne dass etwas gesendet wird | ✅ am Gerät |
| Oberfläche: alle fünf Seiten, Schalten mit Warte-Anzeige und Rückmeldung, Protokoll | ✅ in der Browser-Vorschau (simuliertes Gerät) |
| Echte App (Release-Build) mit Gerät: Übersicht mit echten Werten (Akku, Sender 2 schlank, Pegel, RX-Werte), Diagnose, App-Seite | ✅ per Fenster-Bildschirmfoto |
| Schalter auf der App-Seite → gespeichert und im Panel-Menü abgehakt („Im Ladecase stumm“, von Hand geschaltet) | ✅ |
| Stummschalten über das Panel-Menü: Hardwareschalter im Empfänger off/on, Tooltip | ✅ per D-Bus |
| Autostart war in allen Paketen bis 0.1.9 fälschlich gesperrt (falsche Erkennung des Release-Builds) | ✅ in 0.1.10 behoben |
| Geräteeinstellungen über die Oberfläche der echten App geschaltet (37-mal: Rauschminderung, Einfach/Stark, Mono/Stereo, Hochpassfilter, Backup-Aufnahme, LED, Sendertaste) | ✅ von Hand, jede bestätigt und zurückgelesen (Protokolldatei) |
| Stereo bei nur einem Sender gesperrt (Mono fest), zurück auf Mono bleibt möglich | ✅ 2 Tests; in der App bei einem Sender ausgeblendet |
| Zweiter Start (Menü, Desktop-Symbol) holt das laufende Fenster nach vorn, nur eine Instanz | ✅ 0.1.12 geprüft |
| Eigene Gerätebilder aus `~/.config/micdeck/devices/`, sonst Silhouetten | ✅ 1 Test; in der App mit eigenen Bildern und mit leerem Ordner geprüft |
| Mikrofontest, Diktat und „Im Ladecase stumm“ mit der neuen Oberfläche | ⏳ Logik unverändert, von Hand noch nicht geprüft |

Beim Test gefunden und behoben: Beim Einstecken im laufenden Betrieb blieb die Anzeige auf „Kein
Audioeingang“. PipeWire schickt nach der ersten Geräte-Info Folge-Infos ohne vollständige
Eigenschaften, die die USB-IDs überschrieben. Jetzt werden nur echte Eigenschafts-Updates
übernommen. Zusätzlich wird der Eingang zuerst über die ALSA-Kartennummer aus dem Kernel zugeordnet.
Außerdem lösten harmlose PipeWire-Meldungen beim Abziehen einen Neuaufbau der Verbindung aus.
