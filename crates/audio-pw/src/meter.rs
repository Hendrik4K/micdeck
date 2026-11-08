//! MicDeck's level meter: a capture stream on one input that turns samples
//! into peak/RMS levels in memory. Nothing is stored or sent anywhere.
//!
//! The stream is not exclusive (other programs keep recording), never moves
//! to another input (`node.dont-reconnect`), and does not touch the default
//! device or any volume.

use std::cell::Cell;
use std::io::Cursor;
use std::rc::Rc;

use micdeck_core::level::LevelAccumulator;
use micdeck_core::{AudioEvent, MeterState};
use pipewire as pw;
use pw::properties::properties;
use pw::spa::param::audio::{AudioFormat, AudioInfoRaw};
use pw::spa::param::ParamType;
use pw::spa::pod::serialize::PodSerializer;
use pw::spa::pod::{Object, Pod, Value};
use pw::spa::utils::{Direction, SpaTypes};
use pw::stream::{StreamFlags, StreamListener, StreamRc, StreamState};

use crate::Sink;

/// Level frames per second sent to the UI.
const FRAMES_PER_SECOND: u32 = 30;

struct MeterData {
    target: String,
    sink: Sink,
    acc: LevelAccumulator,
    channels: u32,
    rate: u32,
    window: u32,
    dead: Rc<Cell<bool>>,
}

pub(crate) struct Meter {
    target: String,
    stream: StreamRc,
    _listener: StreamListener<MeterData>,
    dead: Rc<Cell<bool>>,
}

impl Meter {
    pub fn start(core: &pw::core::CoreRc, target: &str, sink: Sink) -> Result<Self, String> {
        let props = properties! {
            "media.type" => "Audio",
            "media.category" => "Capture",
            "media.role" => "Production",
            "media.name" => "Pegelanzeige",
            "application.name" => "MicDeck",
            "node.name" => "micdeck-meter",
            "node.description" => "MicDeck Pegelanzeige",
            "target.object" => target,
            "node.dont-reconnect" => "true",
            "stream.dont-remix" => "true",
            "node.latency" => "1024/48000",
        };
        let stream = StreamRc::new(core.clone(), "micdeck-meter", props)
            .map_err(|e| format!("Mess-Stream konnte nicht erstellt werden ({e})"))?;
        let dead = Rc::new(Cell::new(false));
        let data = MeterData {
            target: target.to_string(),
            sink,
            acc: LevelAccumulator::new(1),
            channels: 0,
            rate: 0,
            window: 48_000 / FRAMES_PER_SECOND,
            dead: dead.clone(),
        };

        let listener = stream
            .add_local_listener_with_user_data(data)
            .state_changed(|_, data, _old, new| match new {
                StreamState::Streaming => (data.sink)(AudioEvent::Meter(MeterState::Running {
                    node_name: data.target.clone(),
                    channels: data.channels,
                    rate: data.rate,
                })),
                StreamState::Error(message) => {
                    data.dead.set(true);
                    (data.sink)(AudioEvent::Meter(MeterState::Failed { message }));
                }
                StreamState::Unconnected => data.dead.set(true),
                _ => {}
            })
            .param_changed(|_, data, id, param| {
                let Some(param) = param else { return };
                if id != ParamType::Format.as_raw() {
                    return;
                }
                let mut info = AudioInfoRaw::new();
                if info.parse(param).is_ok() && info.channels() > 0 && info.rate() > 0 {
                    data.channels = info.channels();
                    data.rate = info.rate();
                    data.window = (info.rate() / FRAMES_PER_SECOND).max(1);
                    data.acc = LevelAccumulator::new(info.channels() as usize);
                }
            })
            .process(|stream, data| {
                let Some(mut buffer) = stream.dequeue_buffer() else { return };
                let Some(d) = buffer.datas_mut().first_mut() else { return };
                let (offset, size) = {
                    let chunk = d.chunk();
                    (chunk.offset() as usize, chunk.size() as usize)
                };
                if data.channels == 0 {
                    return;
                }
                if let Some(bytes) = d.data() {
                    let end = offset.saturating_add(size).min(bytes.len());
                    data.acc.push_f32le_bytes(&bytes[offset.min(end)..end]);
                }
                if data.acc.frames() >= data.window {
                    let frame = data.acc.finish(&data.target, false);
                    (data.sink)(AudioEvent::Level(frame));
                }
            })
            .register()
            .map_err(|e| format!("Mess-Stream konnte nicht eingerichtet werden ({e})"))?;

        // 32-bit float; rate and channel count follow the input.
        let mut format = AudioInfoRaw::new();
        format.set_format(AudioFormat::F32LE);
        let object = Object {
            type_: SpaTypes::ObjectParamFormat.as_raw(),
            id: ParamType::EnumFormat.as_raw(),
            properties: format.into(),
        };
        let bytes = PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(object))
            .map_err(|e| format!("Format konnte nicht beschrieben werden ({e:?})"))?
            .0
            .into_inner();
        let pod = Pod::from_bytes(&bytes).ok_or("Ungültige Formatbeschreibung")?;
        stream
            .connect(
                Direction::Input,
                None,
                StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
                &mut [pod],
            )
            .map_err(|e| format!("Mess-Stream konnte nicht verbunden werden ({e})"))?;

        Ok(Self { target: target.to_string(), stream, _listener: listener, dead })
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    /// The stream ended (its input disappeared or an error occurred).
    pub fn is_dead(&self) -> bool {
        self.dead.get()
    }
}

impl Drop for Meter {
    fn drop(&mut self) {
        let _ = self.stream.disconnect();
    }
}
