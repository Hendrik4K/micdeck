//! USB side of MicDeck: finds DJI receivers, follows hotplug, streams the
//! receiver's decoded status frames and, when the user asks, changes a
//! setting.
//!
//! Only the receiver's vendor interface (6 on the Mic Mini) is ever claimed.
//! The audio interfaces stay with the kernel's `snd-usb-audio` driver, so
//! recording in OBS, Discord and other programs keeps working. A command is
//! only sent for a setting MicDeck lists as writable
//! ([`micdeck_core::device_settings::writable`]), one at a time, and counts
//! as done once the receiver has acknowledged it and reports the new value.

mod sysfs;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dji_protocol::packet::{self, Dialect};
use dji_protocol::{DeviceModel, DeviceStatus};
use micdeck_core::device_settings::writable;
use micdeck_core::{LinkState, UsbEvent, UsbReceiver, WritePhase, WriteReport, DJI_VENDOR_ID};
use nusb::hotplug::HotplugEvent;
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{DeviceId, DeviceInfo, Endpoint, ErrorKind, MaybeFuture};

pub use sysfs::read_device as read_sysfs_device;

/// Receives everything the watcher observes. Called from background threads.
pub type Sink = Arc<dyn Fn(UsbEvent) + Send + Sync>;

/// How often a refused or failed open is retried. Covers a udev rule being
/// installed or another program releasing the interface while MicDeck runs.
const RETRY_BLOCKED: Duration = Duration::from_secs(3);
const RETRY_FAILED_MAX: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(500);
/// How long the receiver gets to acknowledge a command.
const ACK_TIMEOUT: Duration = Duration::from_millis(1_500);
/// How long it gets to report the new value after acknowledging it.
const READ_BACK_TIMEOUT: Duration = Duration::from_secs(3);
const OUT_TIMEOUT: Duration = Duration::from_secs(1);

/// A request to change one setting. Its steps come back as `UsbEvent::Write`.
#[derive(Debug, Clone)]
pub struct WriteRequest {
    pub request: u64,
    pub setting: String,
    pub value: String,
}

/// Keeps the watcher alive; dropping it stops all device threads.
pub struct UsbWatcher {
    stop: Arc<AtomicBool>,
    tx: Sender<Msg>,
}

impl UsbWatcher {
    /// Ask the receiver at `key` to change a setting.
    pub fn write_setting(&self, key: String, request: WriteRequest) {
        let _ = self.tx.send(Msg::Write { key, request });
    }
}

impl Drop for UsbWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Start watching the bus. Events go to `sink` from background threads.
pub fn spawn(sink: Sink) -> UsbWatcher {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    {
        let (tx, sink, stop) = (tx.clone(), sink.clone(), stop.clone());
        thread::Builder::new()
            .name("usb-hotplug".into())
            .spawn(move || watch_bus(&tx, &sink, &stop))
            .expect("failed to start USB hotplug thread");
    }
    {
        let (tx, stop) = (tx.clone(), stop.clone());
        thread::Builder::new()
            .name("usb-supervisor".into())
            .spawn(move || Supervisor::new(sink, tx, stop).run(rx))
            .expect("failed to start USB supervisor thread");
    }
    UsbWatcher { stop, tx }
}

enum Msg {
    Connected(DeviceInfo),
    Disconnected(DeviceId),
    ScanDone,
    Link { id: DeviceId, generation: u64, link: LinkState },
    Ended { id: DeviceId, generation: u64, reason: EndReason },
    Write { key: String, request: WriteRequest },
}

#[derive(Debug)]
enum EndReason {
    Stopped,
    Gone,
    PermissionDenied,
    Busy,
    Failed(String),
}

fn is_dji(info: &DeviceInfo) -> bool {
    info.vendor_id() == DJI_VENDOR_ID
}

fn unix_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

/// A step of `request` without a command behind it.
fn report(request: &WriteRequest, phase: WritePhase) -> WriteReport {
    WriteReport {
        request: request.request,
        setting: request.setting.clone(),
        value: request.value.clone(),
        phase,
        unix_ms: unix_ms(),
        seq: None,
        packet: None,
    }
}

// ---------------------------------------------------------------------------
// Bus watching

fn watch_bus(tx: &Sender<Msg>, sink: &Sink, stop: &AtomicBool) {
    match nusb::watch_devices() {
        Ok(watch) => {
            // List after creating the watch so a device plugged in between the
            // two is not missed; the supervisor ignores duplicates.
            scan(tx);
            for event in futures_lite::stream::block_on(watch) {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let msg = match event {
                    HotplugEvent::Connected(info) if is_dji(&info) => Msg::Connected(info),
                    HotplugEvent::Connected(_) => continue,
                    HotplugEvent::Disconnected(id) => Msg::Disconnected(id),
                };
                if tx.send(msg).is_err() {
                    return;
                }
            }
            sink(UsbEvent::WatchFailed { message: "Die Hotplug-Überwachung wurde beendet".into() });
        }
        Err(e) => {
            log::warn!("USB hotplug unavailable: {e}");
            sink(UsbEvent::WatchFailed { message: format!("Hotplug nicht verfügbar ({e})") });
        }
    }
    poll_bus(tx, stop);
}

fn scan(tx: &Sender<Msg>) {
    match nusb::list_devices().wait() {
        Ok(devices) => {
            for info in devices.filter(is_dji) {
                let _ = tx.send(Msg::Connected(info));
            }
        }
        Err(e) => log::warn!("listing USB devices failed: {e}"),
    }
    let _ = tx.send(Msg::ScanDone);
}

/// Fallback when hotplug notifications are unavailable.
fn poll_bus(tx: &Sender<Msg>, stop: &AtomicBool) {
    let mut known: Vec<DeviceId> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        if let Ok(devices) = nusb::list_devices().wait() {
            let current: Vec<DeviceInfo> = devices.filter(is_dji).collect();
            for info in &current {
                if !known.contains(&info.id()) && tx.send(Msg::Connected(info.clone())).is_err() {
                    return;
                }
            }
            for id in &known {
                if !current.iter().any(|i| i.id() == *id) && tx.send(Msg::Disconnected(*id)).is_err() {
                    return;
                }
            }
            known = current.iter().map(DeviceInfo::id).collect();
        }
        let _ = tx.send(Msg::ScanDone);
        thread::sleep(POLL_INTERVAL);
    }
}

// ---------------------------------------------------------------------------
// Supervisor: one per process, owns the list of receivers and their links.

struct Tracked {
    info: DeviceInfo,
    model: Option<&'static dyn DeviceModel>,
    receiver: UsbReceiver,
    generation: u64,
    actor_stop: Option<Arc<AtomicBool>>,
    /// Setting changes for the running actor.
    actor_writes: Option<Sender<WriteRequest>>,
    retry_at: Option<Instant>,
    failures: u32,
}

struct Supervisor {
    sink: Sink,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
    devices: HashMap<DeviceId, Tracked>,
    generation: u64,
    ready: bool,
}

impl Supervisor {
    fn new(sink: Sink, tx: Sender<Msg>, stop: Arc<AtomicBool>) -> Self {
        Self { sink, tx, stop, devices: HashMap::new(), generation: 0, ready: false }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        loop {
            if self.stop.load(Ordering::Relaxed) {
                for t in self.devices.values() {
                    if let Some(stop) = &t.actor_stop {
                        stop.store(true, Ordering::Relaxed);
                    }
                }
                return;
            }
            match rx.recv_timeout(TICK) {
                Ok(msg) => self.handle(msg),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.tick();
        }
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Connected(info) => {
                if !self.devices.contains_key(&info.id()) {
                    self.attach(info);
                }
            }
            Msg::Disconnected(id) => {
                if let Some(t) = self.devices.remove(&id) {
                    if let Some(stop) = t.actor_stop {
                        stop.store(true, Ordering::Relaxed);
                    }
                    log::info!("receiver {} unplugged", t.receiver.key);
                    (self.sink)(UsbEvent::Removed { key: t.receiver.key });
                }
            }
            Msg::ScanDone => {
                if !self.ready {
                    self.ready = true;
                    (self.sink)(UsbEvent::Ready);
                }
            }
            Msg::Link { id, generation, link } => {
                if let Some(t) = self.devices.get_mut(&id).filter(|t| t.generation == generation) {
                    if link == LinkState::Open {
                        t.failures = 0;
                    }
                    t.receiver.link = link;
                    (self.sink)(UsbEvent::Receiver(t.receiver.clone()));
                }
            }
            Msg::Write { key, request } => {
                let actor = self.devices.values().find(|t| t.receiver.key == key).and_then(|t| t.actor_writes.as_ref());
                if !actor.is_some_and(|writes| writes.send(request.clone()).is_ok()) {
                    let phase = WritePhase::Rejected { reason: "Keine Verbindung zum Empfänger.".into() };
                    (self.sink)(UsbEvent::Write { key, report: report(&request, phase) });
                }
            }
            Msg::Ended { id, generation, reason } => {
                let Some(t) = self.devices.get_mut(&id).filter(|t| t.generation == generation) else {
                    return;
                };
                t.actor_stop = None;
                t.actor_writes = None;
                log::info!("link to {} ended: {reason:?}", t.receiver.key);
                let link = match reason {
                    EndReason::Stopped => return,
                    // The hotplug event that removes the device follows shortly.
                    EndReason::Gone => return,
                    EndReason::PermissionDenied => {
                        t.retry_at = Some(Instant::now() + RETRY_BLOCKED);
                        LinkState::PermissionDenied
                    }
                    EndReason::Busy => {
                        t.retry_at = Some(Instant::now() + RETRY_BLOCKED);
                        LinkState::Busy
                    }
                    EndReason::Failed(message) => {
                        t.failures += 1;
                        let backoff = Duration::from_secs(1 << t.failures.min(5)).min(RETRY_FAILED_MAX);
                        t.retry_at = Some(Instant::now() + backoff);
                        LinkState::Failed { message }
                    }
                };
                if t.receiver.link != link {
                    t.receiver.link = link;
                    (self.sink)(UsbEvent::Receiver(t.receiver.clone()));
                }
            }
        }
    }

    fn attach(&mut self, info: DeviceInfo) {
        let model = dji_protocol::model_for_usb(info.vendor_id(), info.product_id());
        let key = info
            .sysfs_path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("usb-{}-{}", info.busnum(), info.device_address()));
        let (interfaces, alsa_cards) = sysfs::read_device(info.sysfs_path());
        let receiver = UsbReceiver {
            key,
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            model_name: model.map(|m| m.name().to_string()),
            product_string: info.product_string().map(str::to_string),
            serial: info.serial_number().map(str::to_string),
            interfaces,
            alsa_cards,
            link: if model.is_some() { LinkState::Opening } else { LinkState::Unsupported },
        };
        log::info!(
            "DJI device {:04x}:{:04x} at {} ({})",
            receiver.vendor_id,
            receiver.product_id,
            receiver.key,
            receiver.model_name.as_deref().unwrap_or("unknown model"),
        );
        (self.sink)(UsbEvent::Receiver(receiver.clone()));
        self.devices.insert(
            info.id(),
            Tracked {
                info,
                model,
                receiver,
                generation: 0,
                actor_stop: None,
                actor_writes: None,
                retry_at: model.map(|_| Instant::now()),
                failures: 0,
            },
        );
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let ids: Vec<DeviceId> = self.devices.keys().copied().collect();
        for id in ids {
            let Some(t) = self.devices.get_mut(&id) else { continue };

            // The kernel binds the audio driver a moment after the device
            // appears, and may rebind it later; keep the picture current.
            let (interfaces, alsa_cards) = sysfs::read_device(t.info.sysfs_path());
            if interfaces != t.receiver.interfaces || alsa_cards != t.receiver.alsa_cards {
                t.receiver.interfaces = interfaces;
                t.receiver.alsa_cards = alsa_cards;
                (self.sink)(UsbEvent::Receiver(t.receiver.clone()));
            }

            let due = t.retry_at.is_some_and(|at| now >= at);
            let idle = t.actor_stop.is_none();
            let Some(model) = t.model.filter(|_| due && idle) else { continue };
            self.generation += 1;
            t.generation = self.generation;
            t.retry_at = None;
            let stop = Arc::new(AtomicBool::new(false));
            t.actor_stop = Some(stop.clone());
            let (writes_tx, writes) = mpsc::channel();
            t.actor_writes = Some(writes_tx);
            let actor = Actor {
                id,
                generation: t.generation,
                info: t.info.clone(),
                model,
                key: t.receiver.key.clone(),
                sink: self.sink.clone(),
                tx: self.tx.clone(),
                stop,
                writes,
            };
            let spawned = thread::Builder::new()
                .name(format!("usb-{}", actor.key))
                .spawn(move || actor.run());
            if let Err(e) = spawned {
                log::error!("cannot start device thread: {e}");
                t.actor_stop = None;
                t.actor_writes = None;
                t.retry_at = Some(now + RETRY_FAILED_MAX);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Actor: one thread per opened receiver, reading status frames and writing
// the setting changes the user asks for.

struct Actor {
    id: DeviceId,
    generation: u64,
    info: DeviceInfo,
    model: &'static dyn DeviceModel,
    key: String,
    sink: Sink,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
    writes: Receiver<WriteRequest>,
}

/// The setting change on its way, and what it waits for.
struct InFlight {
    request: WriteRequest,
    seq: u16,
    packet: String,
    /// `false` until the receiver acknowledged the command.
    acknowledged: bool,
    deadline: Instant,
}

impl InFlight {
    fn report(&self, phase: WritePhase) -> WriteReport {
        WriteReport { seq: Some(self.seq), packet: Some(self.packet.clone()), ..report(&self.request, phase) }
    }
}

impl Actor {
    fn run(self) {
        let reason = self.session();
        let _ = self.tx.send(Msg::Ended { id: self.id, generation: self.generation, reason });
    }

    fn open_failed(e: &nusb::Error) -> EndReason {
        match e.kind() {
            ErrorKind::PermissionDenied => EndReason::PermissionDenied,
            ErrorKind::Busy => EndReason::Busy,
            ErrorKind::Disconnected => EndReason::Gone,
            _ => EndReason::Failed(format!("Steuerschnittstelle nicht verfügbar ({e})")),
        }
    }

    fn emit(&self, report: WriteReport) {
        log::info!(
            "{} write {}={} (seq {:?}): {:?}",
            self.key,
            report.setting,
            report.value,
            report.seq,
            report.phase
        );
        (self.sink)(UsbEvent::Write { key: self.key.clone(), report });
    }

    /// Sends `request` if it is allowed and possible; returns what it now waits for.
    fn start_write(
        &self,
        request: WriteRequest,
        out: Option<&mut Endpoint<Bulk, Out>>,
        dialect: Option<Dialect>,
        seq: &mut u16,
    ) -> Option<InFlight> {
        let reject = |reason: &str| report(&request, WritePhase::Rejected { reason: reason.into() });
        if !writable(&request.setting, &request.value) {
            self.emit(reject("MicDeck sendet diese Einstellung nicht."));
            return None;
        }
        let Some(dialect) = dialect else {
            self.emit(reject("Der Empfänger hat sich noch nicht gemeldet."));
            return None;
        };
        let Some(command) = self.model.build_command(*seq, dialect, &request.setting, &request.value, None) else {
            self.emit(reject("Mit dieser Firmware nicht möglich."));
            return None;
        };
        let Some(out) = out else {
            self.emit(report(&request, WritePhase::Failed { message: "Kein Schreibzugriff auf den Empfänger.".into() }));
            return None;
        };
        let flight = InFlight {
            request,
            seq: *seq,
            packet: hex(&command),
            acknowledged: false,
            deadline: Instant::now() + ACK_TIMEOUT,
        };
        *seq = seq.wrapping_add(1).max(1);
        let done = out.transfer_blocking(Buffer::from(command), OUT_TIMEOUT);
        if let Err(e) = done.status {
            self.emit(flight.report(WritePhase::Failed { message: format!("Senden fehlgeschlagen ({e})") }));
            return None;
        }
        self.emit(flight.report(WritePhase::Sent));
        Some(flight)
    }

    /// Moves `flight` on after `frame`; returns it while it is still waiting.
    fn advance_write(&self, flight: InFlight, frame: &[u8], status: Option<&DeviceStatus>) -> Option<InFlight> {
        if !flight.acknowledged {
            let Some(ack) = packet::parse_ack(frame).filter(|a| a.seq == flight.seq) else {
                return Some(flight);
            };
            if !ack.accepted() {
                log::warn!("{} refused seq {} with code {:#04x}", self.key, ack.seq, ack.code);
                self.emit(flight.report(WritePhase::Refused));
                return None;
            }
            self.emit(flight.report(WritePhase::Acknowledged));
            return Some(InFlight { acknowledged: true, deadline: Instant::now() + READ_BACK_TIMEOUT, ..flight });
        }
        let reported = status.and_then(|s| s.settings.get(&flight.request.setting));
        if reported == Some(&flight.request.value) {
            self.emit(flight.report(WritePhase::Confirmed));
            return None;
        }
        Some(flight)
    }

    /// Ends `flight` if it waited too long.
    fn expire_write(&self, flight: InFlight, status: &DeviceStatus) -> Option<InFlight> {
        if Instant::now() < flight.deadline {
            return Some(flight);
        }
        let phase = if flight.acknowledged {
            WritePhase::NotApplied { reported: status.settings.get(&flight.request.setting).cloned() }
        } else {
            WritePhase::NoAnswer
        };
        self.emit(flight.report(phase));
        None
    }

    fn session(&self) -> EndReason {
        let device = match self.info.open().wait() {
            Ok(d) => d,
            Err(e) => return Self::open_failed(&e),
        };
        // Detaches a kernel driver from this one interface only, and never
        // takes it from another program (nusb refuses with "busy" then).
        let iface = match device.detach_and_claim_interface(self.model.interface()).wait() {
            Ok(i) => i,
            Err(e) => return Self::open_failed(&e),
        };
        let mut ep = match iface.endpoint::<Bulk, In>(self.model.bulk_in()) {
            Ok(ep) => ep,
            Err(e) => return Self::open_failed(&e),
        };
        // Without it MicDeck still shows everything; it just cannot change settings.
        let mut out = iface
            .endpoint::<Bulk, Out>(self.model.bulk_out())
            .inspect_err(|e| log::warn!("{}: no write access ({e})", self.key))
            .ok();
        let _ = self.tx.send(Msg::Link { id: self.id, generation: self.generation, link: LinkState::Open });

        let mps = ep.max_packet_size().max(1);
        let request_len = 512usize.div_ceil(mps) * mps;
        for _ in 0..4 {
            let buffer = ep.allocate(request_len);
            ep.submit(buffer);
        }

        let mut pending = Vec::with_capacity(1024);
        let mut status = DeviceStatus::disconnected(self.model.id());
        let mut dialect = None;
        let mut seq: u16 = 1;
        let mut flight: Option<InFlight> = None;
        let reason = loop {
            if self.stop.load(Ordering::Relaxed) {
                break EndReason::Stopped;
            }
            if flight.is_none()
                && let Ok(request) = self.writes.try_recv()
            {
                flight = self.start_write(request, out.as_mut(), dialect, &mut seq);
            }
            let wait = if flight.is_some() { Duration::from_millis(50) } else { Duration::from_millis(250) };
            let Some(done) = ep.wait_next_complete(wait) else {
                flight = flight.and_then(|f| self.expire_write(f, &status));
                continue;
            };
            match done.status {
                Ok(()) => {
                    let n = done.actual_len.min(done.buffer.len());
                    pending.extend_from_slice(&done.buffer[..n]);
                    while let Some(frame) = packet::take_frame(&mut pending) {
                        log::trace!("{} frame {:02x?}", self.key, frame);
                        if let Some(d) = packet::heartbeat_dialect(&frame) {
                            dialect = Some(d);
                        }
                        let decoded = self.model.decode(&status, &frame);
                        if let Some(next) = &decoded {
                            status = next.clone();
                            (self.sink)(UsbEvent::Telemetry { key: self.key.clone(), status: status.clone() });
                        }
                        flight = flight.and_then(|f| self.advance_write(f, &frame, decoded.as_ref()));
                    }
                    flight = flight.and_then(|f| self.expire_write(f, &status));
                    ep.submit(done.buffer);
                }
                Err(TransferError::Disconnected) => break EndReason::Gone,
                Err(e) => break EndReason::Failed(format!("Lesefehler ({e})")),
            }
        };
        if let Some(f) = flight {
            self.emit(f.report(WritePhase::Failed { message: "Verbindung zum Empfänger beendet.".into() }));
        }
        drop(out);

        ep.cancel_all();
        while ep.pending() > 0 && ep.wait_next_complete(Duration::from_millis(200)).is_some() {}
        drop(ep);
        drop(iface);
        if !matches!(reason, EndReason::Gone) {
            // Give the interface back to whatever kernel driver had it (if any).
            let _ = device.attach_kernel_driver(self.model.interface());
        }
        reason
    }
}
