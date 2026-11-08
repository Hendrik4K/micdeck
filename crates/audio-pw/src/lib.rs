//! PipeWire side of MicDeck: which audio inputs exist, which is the default,
//! whether the DJI input is muted, and a level meter on demand.
//!
//! Runs its own PipeWire main loop on a background thread and reconnects if
//! PipeWire restarts. MicDeck observes here; the one exception is muting the
//! DJI input when the user presses its mute button. It never changes the
//! default device, volumes or links.

mod meter;
mod mictest;
mod props;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use micdeck_core::{AudioEvent, AudioGraph, AudioService, AudioSource, MeterState};
use pipewire as pw;
use pw::spa::param::ParamType;
use pw::types::ObjectType;

use meter::Meter;
use props::{DeviceProps, NodeProps};

/// Receives everything the monitor observes. Called from the PipeWire thread.
pub type Sink = Arc<dyn Fn(AudioEvent) + Send + Sync>;

const RECONNECT_DELAY: Duration = Duration::from_secs(2);
const EPIPE: i32 = 32;
const ENOENT: i32 = 2;
const METER_RETRY_DELAY: Duration = Duration::from_secs(5);
/// Give PipeWire a moment to describe all objects before the first report.
const FIRST_REPORT_DELAY: Duration = Duration::from_millis(300);

enum Command {
    Meter(Option<String>),
    Mute { node_name: String, mute: bool },
    MicTest { node_name: String },
    MicTestStop,
    Quit,
}

/// Handle to the PipeWire thread; dropping it stops the thread.
pub struct AudioMonitor {
    tx: pw::channel::Sender<Command>,
}

impl AudioMonitor {
    /// Measure levels on the input with this `node.name`, or stop measuring.
    pub fn set_meter(&self, target: Option<String>) {
        let _ = self.tx.send(Command::Meter(target));
    }

    /// Mute or unmute the input with this `node.name` – the only change
    /// MicDeck ever makes to PipeWire, and only when the user asks for it.
    pub fn set_mute(&self, node_name: String, mute: bool) {
        let _ = self.tx.send(Command::Mute { node_name, mute });
    }

    /// Record a few seconds from this input into memory, play them back once
    /// and drop them. Only on the user's explicit request.
    pub fn start_mic_test(&self, node_name: String) {
        let _ = self.tx.send(Command::MicTest { node_name });
    }

    /// End a running microphone session at once.
    pub fn stop_mic_test(&self) {
        let _ = self.tx.send(Command::MicTestStop);
    }
}

impl Drop for AudioMonitor {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Quit);
    }
}

/// Start the PipeWire thread. Events go to `sink`.
pub fn spawn(sink: Sink) -> AudioMonitor {
    let (tx, rx) = pw::channel::channel();
    std::thread::Builder::new()
        .name("pipewire".into())
        .spawn(move || run(rx, sink))
        .expect("failed to start PipeWire thread");
    AudioMonitor { tx }
}

enum End {
    Quit,
    Lost(String),
}

fn run(mut rx: pw::channel::Receiver<Command>, sink: Sink) {
    pw::init();
    let mut meter_wanted = None;
    loop {
        let (back, end) = session(rx, &sink, &mut meter_wanted);
        rx = back;
        match end {
            End::Quit => return,
            End::Lost(reason) => {
                log::warn!("PipeWire unavailable: {reason}");
                sink(AudioEvent::Service(AudioService::Unavailable { reason }));
                std::thread::sleep(RECONNECT_DELAY);
            }
        }
    }
}

/// One connection to PipeWire, until it is lost or MicDeck quits.
fn session(
    rx: pw::channel::Receiver<Command>,
    sink: &Sink,
    meter_wanted: &mut Option<String>,
) -> (pw::channel::Receiver<Command>, End) {
    let mainloop = match pw::main_loop::MainLoopRc::new(None) {
        Ok(m) => m,
        Err(e) => return (rx, End::Lost(format!("PipeWire-Schleife nicht verfügbar ({e})"))),
    };
    let context = match pw::context::ContextRc::new(&mainloop, None) {
        Ok(c) => c,
        Err(e) => return (rx, End::Lost(format!("PipeWire-Kontext nicht verfügbar ({e})"))),
    };
    let core = match context.connect_rc(None) {
        Ok(c) => c,
        Err(_) => return (rx, End::Lost("Keine Verbindung zum PipeWire-Dienst".into())),
    };
    let registry = match core.get_registry_rc() {
        Ok(r) => r,
        Err(e) => return (rx, End::Lost(format!("PipeWire-Registry nicht verfügbar ({e})"))),
    };
    log::info!("connected to PipeWire");

    let shared = Rc::new(RefCell::new(Shared::new(sink.clone(), meter_wanted.clone())));
    let end: Rc<RefCell<Option<End>>> = Rc::new(RefCell::new(None));

    let _core_listener = core
        .add_listener_local()
        .error({
            let mainloop = mainloop.downgrade();
            let end = end.clone();
            move |id, _seq, res, message| {
                // A lost connection is reported on the core (object 0) as
                // -EPIPE. Other core errors, such as "unknown resource" for an
                // object that was just removed on unplug, are harmless.
                let lost = id == 0 && res == -EPIPE;
                if lost || res != -ENOENT {
                    log::warn!("PipeWire error on object {id} ({res}): {message}");
                } else {
                    log::debug!("PipeWire error on object {id} ({res}): {message}");
                }
                if lost {
                    end.borrow_mut()
                        .get_or_insert(End::Lost(format!("Verbindung zu PipeWire unterbrochen ({message})")));
                    if let Some(mainloop) = mainloop.upgrade() {
                        mainloop.quit();
                    }
                }
            }
        })
        .register();

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let shared = shared.clone();
            let registry = registry.downgrade();
            move |object| {
                if let Some(registry) = registry.upgrade() {
                    Shared::on_global(&shared, &registry, object);
                }
            }
        })
        .global_remove({
            let shared = shared.clone();
            move |id| shared.borrow_mut().on_remove(id)
        })
        .register();

    let attached = rx.attach(mainloop.loop_(), {
        let shared = shared.clone();
        let mainloop = mainloop.downgrade();
        let end = end.clone();
        move |command| match command {
            Command::Meter(target) => shared.borrow_mut().meter_wanted = target,
            Command::Mute { node_name, mute } => shared.borrow().set_mute(&node_name, mute),
            Command::MicTest { node_name } => shared.borrow_mut().mic_test_wanted = Some(node_name),
            Command::MicTestStop => {
                let mut s = shared.borrow_mut();
                s.mic_test_wanted = None;
                s.mic_test = None;
            }
            Command::Quit => {
                end.borrow_mut().replace(End::Quit);
                if let Some(mainloop) = mainloop.upgrade() {
                    mainloop.quit();
                }
            }
        }
    });

    let timer = mainloop.loop_().add_timer({
        let shared = shared.clone();
        let core = core.clone();
        move |_| Shared::tick(&shared, &core)
    });
    let _ = timer.update_timer(Some(Duration::from_millis(50)), Some(Duration::from_millis(100)));

    mainloop.run();

    *meter_wanted = shared.borrow().meter_wanted.clone();
    // Tear down streams and proxies while the connection still exists.
    shared.borrow_mut().shutdown();
    drop(timer);
    let rx = attached.deattach();
    let end = end.borrow_mut().take().unwrap_or(End::Lost("PipeWire-Verbindung beendet".into()));
    (rx, end)
}

struct DeviceEntry {
    _proxy: pw::device::Device,
    _listener: pw::device::DeviceListener,
    props: DeviceProps,
}

struct NodeEntry {
    _proxy: pw::node::Node,
    _listener: pw::node::NodeListener,
    props: NodeProps,
}

struct MetadataEntry {
    id: u32,
    _proxy: pw::metadata::Metadata,
    _listener: pw::metadata::MetadataListener,
}

struct Shared {
    sink: Sink,
    started: Instant,
    devices: HashMap<u32, DeviceEntry>,
    nodes: HashMap<u32, NodeEntry>,
    /// Other programs' recording streams: node id → program name.
    capture_streams: HashMap<u32, String>,
    /// Links: link id → (output node, input node).
    links: HashMap<u32, (u32, u32)>,
    metadata: Option<MetadataEntry>,
    default_source: Option<String>,
    configured_default_source: Option<String>,
    dirty: bool,
    reported: Option<AudioGraph>,
    meter_wanted: Option<String>,
    meter: Option<Meter>,
    meter_retry_at: Option<Instant>,
    mic_test_wanted: Option<String>,
    mic_test: Option<mictest::MicSession>,
}

impl Shared {
    fn new(sink: Sink, meter_wanted: Option<String>) -> Self {
        Self {
            sink,
            started: Instant::now(),
            devices: HashMap::new(),
            nodes: HashMap::new(),
            capture_streams: HashMap::new(),
            links: HashMap::new(),
            metadata: None,
            default_source: None,
            configured_default_source: None,
            dirty: true,
            reported: None,
            meter_wanted,
            meter: None,
            meter_retry_at: None,
            mic_test_wanted: None,
            mic_test: None,
        }
    }

    fn on_global(
        this: &Rc<RefCell<Self>>,
        registry: &pw::registry::RegistryRc,
        object: &pw::registry::GlobalObject<&pw::spa::utils::dict::DictRef>,
    ) {
        let props = object.props;
        let class = props.and_then(|p| p.get("media.class"));
        let id = object.id;
        match object.type_ {
            ObjectType::Device if class == Some("Audio/Device") => {
                let Ok(device) = registry.bind::<pw::device::Device, _>(object) else { return };
                let weak = Rc::downgrade(this);
                let listener = device
                    .add_listener_local()
                    .info(move |info| {
                        let Some(shared) = weak.upgrade() else { return };
                        let mut s = shared.borrow_mut();
                        // Later updates that only change params carry incomplete
                        // props; reading them would wipe the USB IDs.
                        let props_changed = info.change_mask().contains(pw::device::DeviceChangeMask::PROPS);
                        if let (true, Some(entry), Some(p)) = (props_changed, s.devices.get_mut(&id), info.props()) {
                            entry.props = props::device_props(|k| p.get(k));
                        }
                        log::debug!(
                            "device {id} info: change mask {:?}, {:?}",
                            info.change_mask(),
                            s.devices.get(&id).map(|e| &e.props),
                        );
                        s.dirty = true;
                    })
                    .register();
                this.borrow_mut().devices.insert(
                    id,
                    DeviceEntry { _proxy: device, _listener: listener, props: DeviceProps::default() },
                );
            }
            ObjectType::Node if class.is_some_and(|c| c.starts_with("Audio/Source")) => {
                let Ok(node) = registry.bind::<pw::node::Node, _>(object) else { return };
                let info_weak = Rc::downgrade(this);
                let param_weak = Rc::downgrade(this);
                let listener = node
                    .add_listener_local()
                    .info(move |info| {
                        let Some(shared) = info_weak.upgrade() else { return };
                        let mut s = shared.borrow_mut();
                        if let Some(entry) = s.nodes.get_mut(&id) {
                            if let Some(p) = info.props() {
                                props::apply_node_props(&mut entry.props, |k| p.get(k));
                            }
                            entry.props.state = Some(props::node_state(&info.state()));
                        }
                        s.dirty = true;
                    })
                    .param(move |_seq, param_type, _index, _next, param| {
                        if param_type != ParamType::Props {
                            return;
                        }
                        let Some(pod) = param else { return };
                        let (muted, volume) = props::mute_and_volume(pod.as_bytes());
                        let Some(shared) = param_weak.upgrade() else { return };
                        let mut s = shared.borrow_mut();
                        if let Some(entry) = s.nodes.get_mut(&id) {
                            // A node can report several Props objects; keep known values.
                            entry.props.muted = muted.or(entry.props.muted);
                            entry.props.volume = volume.or(entry.props.volume);
                        }
                        s.dirty = true;
                    })
                    .register();
                node.subscribe_params(&[ParamType::Props]);
                let mut initial = NodeProps::default();
                if let Some(p) = props {
                    props::apply_node_props(&mut initial, |k| p.get(k));
                }
                this.borrow_mut()
                    .nodes
                    .insert(id, NodeEntry { _proxy: node, _listener: listener, props: initial });
            }
            ObjectType::Node if class == Some("Stream/Input/Audio") => {
                let get = |k: &str| props.and_then(|p| p.get(k));
                // MicDeck's own meter and test, and level monitors of mixers, don't count.
                let own = get("node.name").is_some_and(|n| n.starts_with("micdeck"));
                if own || get("stream.monitor") == Some("true") {
                    return;
                }
                let name = get("application.name").or(get("node.name")).unwrap_or("Ein Programm");
                let mut s = this.borrow_mut();
                s.capture_streams.insert(id, name.to_string());
                s.dirty = true;
            }
            ObjectType::Link => {
                let node = |k: &str| props.and_then(|p| p.get(k)).and_then(|v| v.parse::<u32>().ok());
                if let (Some(out), Some(input)) = (node("link.output.node"), node("link.input.node")) {
                    let mut s = this.borrow_mut();
                    s.links.insert(id, (out, input));
                    s.dirty = true;
                }
            }
            ObjectType::Metadata if props.and_then(|p| p.get("metadata.name")) == Some("default") => {
                let Ok(metadata) = registry.bind::<pw::metadata::Metadata, _>(object) else { return };
                let weak = Rc::downgrade(this);
                let listener = metadata
                    .add_listener_local()
                    .property(move |subject, key, _type, value| {
                        if subject != 0 {
                            return 0;
                        }
                        let Some(shared) = weak.upgrade() else { return 0 };
                        let mut s = shared.borrow_mut();
                        let parsed = value.and_then(props::default_node_name);
                        match key {
                            Some("default.audio.source") => s.default_source = parsed,
                            Some("default.configured.audio.source") => s.configured_default_source = parsed,
                            // No key: everything was cleared.
                            None => {
                                s.default_source = None;
                                s.configured_default_source = None;
                            }
                            Some(_) => return 0,
                        }
                        s.dirty = true;
                        0
                    })
                    .register();
                this.borrow_mut().metadata = Some(MetadataEntry { id, _proxy: metadata, _listener: listener });
            }
            _ => {}
        }
    }

    /// Mute through WirePlumber, like the desktop's sound settings do: that
    /// path sets the device route, which on the DJI receiver flips its own
    /// USB mute switch. Setting the node's `Props` directly would only mute
    /// in software (checked on real hardware).
    fn set_mute(&self, node_name: &str, mute: bool) {
        let Some(&id) = self.nodes.iter().find(|(_, n)| n.props.node_name.as_deref() == Some(node_name)).map(|(id, _)| id)
        else {
            log::warn!("cannot mute {node_name}: input not found");
            return;
        };
        log::info!("{} {node_name}", if mute { "muting" } else { "unmuting" });
        std::thread::spawn(move || {
            let result = std::process::Command::new("wpctl")
                .args(["set-mute", &id.to_string(), if mute { "1" } else { "0" }])
                .status();
            match result {
                Ok(status) if status.success() => {}
                Ok(status) => log::warn!("wpctl set-mute failed: {status}"),
                Err(e) => log::warn!("wpctl not available: {e}"),
            }
        });
    }

    fn on_remove(&mut self, id: u32) {
        let removed = self.devices.remove(&id).is_some()
            || self.nodes.remove(&id).is_some()
            || self.capture_streams.remove(&id).is_some()
            || self.links.remove(&id).is_some();
        if self.metadata.as_ref().is_some_and(|m| m.id == id) {
            self.metadata = None;
            self.default_source = None;
            self.configured_default_source = None;
            self.dirty = true;
        }
        if removed {
            self.dirty = true;
        }
    }

    fn graph(&self) -> AudioGraph {
        let mut sources: Vec<AudioSource> = self
            .nodes
            .iter()
            .filter_map(|(&id, n)| {
                let p = &n.props;
                let device = p.device_id.and_then(|d| self.devices.get(&d)).map(|d| &d.props);
                Some(AudioSource {
                    id,
                    node_name: p.node_name.clone()?,
                    description: p.description.clone().or_else(|| p.node_name.clone())?,
                    usb_vendor_id: device.and_then(|d| d.usb_vendor_id),
                    usb_product_id: device.and_then(|d| d.usb_product_id),
                    alsa_card: p.alsa_card.or_else(|| device.and_then(|d| d.alsa_card)),
                    state: p.state,
                    muted: p.muted,
                    volume: p.volume,
                    channels: p.channels,
                    capture_apps: self.capture_apps(id),
                })
            })
            // MicDeck's own meter is a stream, not a source, but be safe.
            .filter(|s| s.node_name != "micdeck-meter")
            .collect();
        sources.sort_by_key(|s| s.id);
        AudioGraph {
            sources,
            default_source: self.default_source.clone(),
            configured_default_source: self.configured_default_source.clone(),
        }
    }

    /// Names of the other programs recording from source `id`.
    fn capture_apps(&self, id: u32) -> Vec<String> {
        let mut apps: Vec<String> = self
            .links
            .values()
            .filter(|(out, _)| *out == id)
            .filter_map(|(_, input)| self.capture_streams.get(input).cloned())
            .collect();
        apps.sort();
        apps.dedup();
        apps
    }

    fn tick(this: &Rc<RefCell<Self>>, core: &pw::core::CoreRc) {
        let mut s = this.borrow_mut();
        if s.dirty && s.started.elapsed() >= FIRST_REPORT_DELAY {
            s.dirty = false;
            let graph = s.graph();
            if s.reported.as_ref() != Some(&graph) {
                log::debug!(
                    "audio inputs {:?}, default {:?}, devices {:?}",
                    graph
                        .sources
                        .iter()
                        .map(|s| (s.node_name.as_str(), s.usb_vendor_id, s.alsa_card))
                        .collect::<Vec<_>>(),
                    graph.default_source,
                    s.devices.iter().map(|(id, d)| (*id, &d.props)).collect::<Vec<_>>(),
                );
                (s.sink)(AudioEvent::Graph(graph.clone()));
                s.reported = Some(graph);
            }
        }
        s.reconcile_meter(core);
        s.run_mic_test(core);
    }

    fn run_mic_test(&mut self, core: &pw::core::CoreRc) {
        if let Some(target) = self.mic_test_wanted.take() {
            if self.mic_test.is_some() {
                log::info!("microphone test already running");
            } else {
                match mictest::MicSession::start(core, &target, self.sink.clone()) {
                    Ok(test) => {
                        log::info!("microphone test started on {target}");
                        self.mic_test = Some(test);
                    }
                    Err(message) => {
                        log::warn!("microphone test failed: {message}");
                        (self.sink)(AudioEvent::MicTest(micdeck_core::MicTestState::Failed { message }));
                    }
                }
            }
        }
        if let Some(test) = &mut self.mic_test
            && !test.poll(core)
        {
            self.mic_test = None;
        }
    }

    /// Start, stop or restart the meter so it matches what is wanted.
    fn reconcile_meter(&mut self, core: &pw::core::CoreRc) {
        let wanted = self.meter_wanted.clone();
        let available = wanted.as_ref().is_some_and(|name| {
            self.nodes.values().any(|n| n.props.node_name.as_deref() == Some(name.as_str()))
        });

        if let Some(meter) = &self.meter {
            let keep = available && !meter.is_dead() && wanted.as_deref() == Some(meter.target());
            if !keep {
                let died = meter.is_dead();
                self.meter = None;
                if died {
                    // Try again later, in case the input comes back.
                    self.meter_retry_at = Some(Instant::now() + METER_RETRY_DELAY);
                }
                (self.sink)(AudioEvent::Meter(MeterState::Off));
            }
        }

        if self.meter.is_some() || !available {
            return;
        }
        if self.meter_retry_at.is_some_and(|t| Instant::now() < t) {
            return;
        }
        let Some(target) = wanted else { return };
        match Meter::start(core, &target, self.sink.clone()) {
            Ok(meter) => {
                log::info!("level meter started on {target}");
                self.meter = Some(meter);
                self.meter_retry_at = None;
                (self.sink)(AudioEvent::Meter(MeterState::Starting { node_name: target }));
            }
            Err(message) => {
                log::warn!("level meter failed: {message}");
                self.meter_retry_at = Some(Instant::now() + METER_RETRY_DELAY);
                (self.sink)(AudioEvent::Meter(MeterState::Failed { message }));
            }
        }
    }

    fn shutdown(&mut self) {
        self.mic_test = None;
        self.meter = None;
        self.nodes.clear();
        self.devices.clear();
        self.metadata = None;
    }
}
