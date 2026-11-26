//! Hardware check: does writing the plug-free speaker setting restart the
//! receiver, even with its current value? Prints every USB event with the
//! time since sending. Needs MicDeck closed.
//!
//! ```sh
//! cargo run -p usb-link --example restart_receiver [on|off]
//! ```

use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use micdeck_core::UsbEvent;
use usb_link::{WriteRequest, RESTART};

fn main() {
    let (tx, events) = mpsc::channel();
    let watcher = usb_link::spawn(Arc::new(move |e| {
        let _ = tx.send(e);
    }));
    let deadline = Instant::now() + Duration::from_secs(10);
    let (key, current) = loop {
        if Instant::now() > deadline {
            eprintln!("Kein Empfänger mit Statusdaten (angeschlossen? MicDeck beendet?)");
            std::process::exit(1);
        }
        if let Ok(UsbEvent::Telemetry { key, status }) = events.recv_timeout(Duration::from_millis(200)) {
            if let Some(v) = status.settings.get("plug-free") {
                let tx = status.tx.iter().flatten().count();
                println!("Empfänger {key}: plug-free = {v}, Sender verbunden: {tx}");
                break (key, v.clone());
            }
        }
    };
    let value = std::env::args().nth(1).unwrap_or(current);
    println!("Sende plug-free = {value} …");
    let start = Instant::now();
    watcher.write_setting(key, WriteRequest { request: 1, setting: RESTART.into(), value });
    let mut last_tx = None;
    while start.elapsed() < Duration::from_secs(40) {
        let Ok(e) = events.recv_timeout(Duration::from_millis(200)) else { continue };
        let t = start.elapsed().as_secs_f32();
        match e {
            UsbEvent::Telemetry { status, .. } => {
                let tx = status.tx.iter().flatten().count();
                if last_tx != Some(tx) {
                    println!("{t:5.1} s  Status: Sender verbunden {tx}, plug-free {:?}", status.settings.get("plug-free"));
                    last_tx = Some(tx);
                }
            }
            UsbEvent::Write { report, .. } => println!("{t:5.1} s  Schreiben: {:?}", report.phase),
            other => println!("{t:5.1} s  {other:?}"),
        }
    }
}
