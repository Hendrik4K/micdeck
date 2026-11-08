//! Hardware check for the write path: switches every setting MicDeck may
//! send to its other value, waits for the receiver's acknowledgement and
//! read-back, then restores the original value the same way. Also checks
//! that a setting MicDeck must never send is refused without USB traffic.
//!
//! Needs a connected receiver with transmitter 1 switched on, and no other
//! program (including MicDeck itself) holding its control interface:
//!
//! ```sh
//! cargo run -p usb-link --example settings_roundtrip
//! ```

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use micdeck_core::device_settings::{Access, SPECS};
use micdeck_core::{UsbEvent, WritePhase, WriteReport};
use usb_link::WriteRequest;

struct Rig {
    events: mpsc::Receiver<UsbEvent>,
    watcher: usb_link::UsbWatcher,
    key: String,
    settings: BTreeMap<String, String>,
    next: u64,
}

impl Rig {
    /// Waits for a receiver that streams status frames with settings.
    fn connect() -> Result<Self, String> {
        let (tx, events) = mpsc::channel();
        let watcher = usb_link::spawn(Arc::new(move |e| {
            let _ = tx.send(e);
        }));
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match events.recv_timeout(Duration::from_millis(200)) {
                Ok(UsbEvent::Telemetry { key, status }) if !status.settings.is_empty() => {
                    println!("Empfänger {key}: Protokoll v{:?}, {} Einstellungen", status.protocol_version, status.settings.len());
                    return Ok(Self { events, watcher, key, settings: status.settings, next: 0 });
                }
                Ok(UsbEvent::Receiver(r)) => println!("Empfänger {} ({:?})", r.key, r.link),
                _ => {}
            }
        }
        Err("Kein Empfänger mit Statusdaten gefunden (angeschlossen? MicDeck beendet?)".into())
    }

    /// Sends one change and waits for its last step.
    fn write(&mut self, setting: &str, value: &str) -> (WriteReport, Duration) {
        self.next += 1;
        let request = self.next;
        let started = Instant::now();
        self.watcher.write_setting(
            self.key.clone(),
            WriteRequest { request, setting: setting.into(), value: value.into() },
        );
        loop {
            match self.events.recv_timeout(Duration::from_secs(8)) {
                Ok(UsbEvent::Telemetry { status, .. }) => self.settings = status.settings,
                Ok(UsbEvent::Write { report, .. }) if report.request == request && report.phase.is_final() => {
                    return (report, started.elapsed());
                }
                Ok(_) => {}
                Err(_) => {
                    let report = WriteReport {
                        request,
                        setting: setting.into(),
                        value: value.into(),
                        phase: WritePhase::Failed { message: "kein Ergebnis in 8 s".into() },
                        unix_ms: 0,
                        seq: None,
                        packet: None,
                    };
                    return (report, started.elapsed());
                }
            }
        }
    }
}

fn verdict(report: &WriteReport) -> String {
    match &report.phase {
        WritePhase::Confirmed => "bestätigt + zurückgelesen".into(),
        other => format!("{other:?}"),
    }
}

fn main() {
    let mut rig = match Rig::connect() {
        Ok(rig) => rig,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let mut failures = 0;

    for spec in SPECS.iter().filter(|s| s.access == Access::Writable) {
        let Some(original) = rig.settings.get(spec.id).cloned() else {
            println!("{:<20} übersprungen: Wert wird nicht gemeldet (Sender aus?)", spec.id);
            continue;
        };
        let is = |rig: &Rig, id: &str, v: &str| rig.settings.get(id).is_some_and(|x| x == v);
        // The level can only be chosen while noise cancelling is on: switch
        // it on for the test and off again afterwards.
        let nc_was_off = spec.id == "noise-cancel" && is(&rig, "noise-cancel-power", "off");
        if nc_was_off {
            let (on, _) = rig.write("noise-cancel-power", "on");
            println!("{:<20} Rauschminderung für den Test an: {}", "", verdict(&on));
        }
        let locked = match spec.id {
            "noise-cancel" if is(&rig, "noise-cancel-power", "off") => Some("Rauschminderung ist aus"),
            "safety-track" if is(&rig, "stereo", "stereo") => Some("Stereo ist an"),
            "stereo" if is(&rig, "safety-track", "on") => Some("Backup-Aufnahme ist an"),
            _ => None,
        };
        if let Some(why) = locked {
            println!("{:<20} übersprungen: {why}", spec.id);
            continue;
        }
        let Some(&(target, _)) = spec.options.iter().find(|(v, _)| *v != original) else { continue };

        let (there, t1) = rig.write(spec.id, target);
        let (back, t2) = rig.write(spec.id, &original);
        let ok = there.phase == WritePhase::Confirmed && back.phase == WritePhase::Confirmed;
        let restored = rig.settings.get(spec.id) == Some(&original);
        if !ok || !restored {
            failures += 1;
        }
        println!(
            "{:<20} {original:>8} -> {target:<8} {} ({} ms) | zurück: {} ({} ms){}",
            spec.id,
            verdict(&there),
            t1.as_millis(),
            verdict(&back),
            t2.as_millis(),
            if restored { "" } else { "  !! NICHT WIEDERHERGESTELLT" },
        );
        if let Some(packet) = &there.packet {
            println!("{:<20} gesendet: {packet}", "");
        }
        if nc_was_off {
            let (off, _) = rig.write("noise-cancel-power", "off");
            let restored = off.phase == WritePhase::Confirmed;
            if !restored {
                failures += 1;
            }
            println!("{:<20} Rauschminderung wieder aus: {}", "", verdict(&off));
        }
    }

    // Must be refused before anything reaches the USB endpoint.
    let (refused, _) = rig.write("plug-free", "on");
    let blocked = matches!(refused.phase, WritePhase::Rejected { .. }) && refused.seq.is_none();
    if !blocked {
        failures += 1;
    }
    println!("{:<20} gesperrt: {}", "plug-free", if blocked { "ja, nichts gesendet" } else { "NEIN" });

    println!("{}", if failures == 0 { "Alles in Ordnung." } else { "Es gab Fehler." });
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
