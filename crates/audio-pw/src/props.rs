//! Parsing of PipeWire properties and params into MicDeck's terms. Pure
//! functions, so they are testable without a running PipeWire.

use micdeck_core::NodeRunState;
use pipewire as pw;
use pw::spa::pod::deserialize::PodDeserializer;
use pw::spa::pod::{Value, ValueArray};

/// The parts of an `Audio/Device` MicDeck needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceProps {
    pub usb_vendor_id: Option<u16>,
    pub usb_product_id: Option<u16>,
    pub alsa_card: Option<u32>,
}

/// The parts of an `Audio/Source` node MicDeck needs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeProps {
    pub node_name: Option<String>,
    pub description: Option<String>,
    pub device_id: Option<u32>,
    pub alsa_card: Option<u32>,
    pub channels: Option<u32>,
    pub state: Option<NodeRunState>,
    pub muted: Option<bool>,
    pub volume: Option<f32>,
}

fn parse_id(s: &str) -> Option<u16> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u16::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

pub fn device_props<'a>(get: impl Fn(&str) -> Option<&'a str>) -> DeviceProps {
    DeviceProps {
        usb_vendor_id: get("device.vendor.id").and_then(parse_id),
        usb_product_id: get("device.product.id").and_then(parse_id),
        alsa_card: get("api.alsa.card").and_then(|c| c.trim().parse().ok()),
    }
}

/// Update `node` from an info event's properties, keeping fields the event
/// does not mention.
pub fn apply_node_props<'a>(node: &mut NodeProps, get: impl Fn(&str) -> Option<&'a str>) {
    if let Some(name) = get("node.name") {
        node.node_name = Some(name.to_string());
    }
    if let Some(desc) = get("node.description").or_else(|| get("node.nick")) {
        node.description = Some(desc.to_string());
    }
    if let Some(id) = get("device.id").and_then(|d| d.trim().parse().ok()) {
        node.device_id = Some(id);
    }
    // ALSA nodes made by WirePlumber carry `alsa.card`, older setups `api.alsa.card`.
    if let Some(card) = get("api.alsa.card").or_else(|| get("alsa.card")).and_then(|c| c.trim().parse().ok()) {
        node.alsa_card = Some(card);
    }
    if let Some(ch) = get("audio.channels").and_then(|c| c.trim().parse().ok()) {
        node.channels = Some(ch);
    }
}

pub fn node_state(state: &pw::node::NodeState<'_>) -> NodeRunState {
    use pw::node::NodeState;
    match state {
        NodeState::Error(_) => NodeRunState::Error,
        NodeState::Creating => NodeRunState::Creating,
        NodeState::Suspended => NodeRunState::Suspended,
        NodeState::Idle => NodeRunState::Idle,
        NodeState::Running => NodeRunState::Running,
    }
}

/// Mute flag and desktop-style volume (cubic, 1.0 = 100 %) from a node's
/// `Props` param.
pub fn mute_and_volume(pod_bytes: &[u8]) -> (Option<bool>, Option<f32>) {
    let Ok((_, Value::Object(object))) = PodDeserializer::deserialize_any_from(pod_bytes) else {
        return (None, None);
    };
    let mut muted = None;
    let mut volume = None;
    for prop in object.properties {
        if prop.key == pw::spa::sys::SPA_PROP_mute
            && let Value::Bool(m) = prop.value
        {
            muted = Some(m);
        } else if prop.key == pw::spa::sys::SPA_PROP_channelVolumes
            && let Value::ValueArray(ValueArray::Float(v)) = prop.value
        {
            volume = cubic_volume(&v);
        }
    }
    (muted, volume)
}

/// PipeWire stores linear gains; desktop mixers show their cube root.
fn cubic_volume(linear: &[f32]) -> Option<f32> {
    if linear.is_empty() {
        return None;
    }
    let mean = linear.iter().sum::<f32>() / linear.len() as f32;
    Some(mean.max(0.0).cbrt())
}

/// `node.name` from a `default.audio.source`-style metadata value such as
/// `{"name":"alsa_input.usb-…"}`.
pub fn default_node_name(value: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(value).ok()?;
    json.get("name")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw::spa::pod::serialize::PodSerializer;
    use pw::spa::pod::{Object, Property, PropertyFlags};
    use std::collections::HashMap;

    fn lookup<'m>(map: &'m HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<&'static str> + 'm {
        move |k| map.get(k).copied()
    }

    #[test]
    fn reads_usb_ids_and_card_of_a_device() {
        let map = HashMap::from([
            ("device.vendor.id", "0x2ca3"),
            ("device.product.id", "0x4011"),
            ("api.alsa.card", "3"),
        ]);
        let p = device_props(lookup(&map));
        assert_eq!(p.usb_vendor_id, Some(0x2ca3));
        assert_eq!(p.usb_product_id, Some(0x4011));
        assert_eq!(p.alsa_card, Some(3));
        assert_eq!(device_props(|_| None), DeviceProps::default());
    }

    #[test]
    fn node_info_updates_keep_unmentioned_fields() {
        let mut node = NodeProps::default();
        let first = HashMap::from([
            ("node.name", "alsa_input.usb-DJI"),
            ("node.description", "DJI Mic Mini Analog Stereo"),
            ("device.id", "57"),
            ("audio.channels", "2"),
        ]);
        apply_node_props(&mut node, lookup(&first));
        let later = HashMap::from([("node.name", "alsa_input.usb-DJI.renamed")]);
        apply_node_props(&mut node, lookup(&later));
        assert_eq!(node.node_name.as_deref(), Some("alsa_input.usb-DJI.renamed"));
        assert_eq!(node.description.as_deref(), Some("DJI Mic Mini Analog Stereo"));
        assert_eq!(node.device_id, Some(57));
        assert_eq!(node.channels, Some(2));

        // Without a description, the nick is used.
        let mut nick_only = NodeProps::default();
        apply_node_props(&mut nick_only, lookup(&HashMap::from([("node.nick", "USB Mic")])));
        assert_eq!(nick_only.description.as_deref(), Some("USB Mic"));
    }

    #[test]
    fn parses_mute_and_volume_from_props_pod() {
        let object = Object {
            type_: pw::spa::utils::SpaTypes::ObjectParamProps.as_raw(),
            id: pw::spa::param::ParamType::Props.as_raw(),
            properties: vec![
                Property {
                    key: pw::spa::sys::SPA_PROP_mute,
                    flags: PropertyFlags::empty(),
                    value: Value::Bool(true),
                },
                Property {
                    key: pw::spa::sys::SPA_PROP_channelVolumes,
                    flags: PropertyFlags::empty(),
                    value: Value::ValueArray(ValueArray::Float(vec![0.125, 0.125])),
                },
            ],
        };
        let bytes = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object))
            .unwrap()
            .0
            .into_inner();
        let (muted, volume) = mute_and_volume(&bytes);
        assert_eq!(muted, Some(true));
        assert!((volume.unwrap() - 0.5).abs() < 1e-6, "cube root of 0.125 is 0.5");
        assert_eq!(mute_and_volume(&[1, 2, 3]), (None, None));
    }

    #[test]
    fn reads_default_node_name_from_metadata_json() {
        assert_eq!(
            default_node_name(r#"{"name":"alsa_input.usb-DJI_Mic-00.analog-stereo"}"#).as_deref(),
            Some("alsa_input.usb-DJI_Mic-00.analog-stereo")
        );
        assert_eq!(default_node_name("not json"), None);
        assert_eq!(default_node_name(r#"{"other":1}"#), None);
    }
}
