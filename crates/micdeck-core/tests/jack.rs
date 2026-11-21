//! Jack mode: the receiver connected by its 3.5 mm output instead of USB-C.
//! Sound arrives on an analog socket; there is no status data.

use micdeck_core::mock::Scenario;
use micdeck_core::{AudioEvent, AudioSource, CheckId, CheckLevel, Connection, Input, Level, State};

const FRONT: &str = "alsa_input.klinke-vorne";
const BACK: &str = "alsa_input.usb-Generic_USB_Audio-00.HiFi__Mic__source";

fn jack(id: u32, node_name: &str, plugged: bool) -> AudioSource {
    AudioSource {
        id,
        node_name: node_name.into(),
        description: node_name.into(),
        usb_vendor_id: None,
        usb_product_id: None,
        alsa_card: Some(4),
        state: None,
        muted: Some(false),
        volume: Some(1.0),
        channels: Some(2),
        capture_apps: Vec::new(),
        jack_plugged: Some(plugged),
    }
}

/// No receiver on USB, with these analog inputs; `default` is the default input.
fn state_with(jacks: Vec<AudioSource>, default: &str) -> State {
    let mut state = State::new(true);
    for input in Scenario::Unplugged.inputs() {
        let input = match input {
            Input::Audio(AudioEvent::Graph(mut graph)) => {
                graph.sources.extend(jacks.iter().cloned());
                graph.default_source = Some(default.into());
                Input::Audio(AudioEvent::Graph(graph))
            }
            other => other,
        };
        state.apply(input, 0);
    }
    state
}

#[test]
fn plugged_socket_without_usb_receiver_is_jack_mode() {
    let state = state_with(vec![jack(201, FRONT, true)], FRONT);
    let snap = state.snapshot(10_000);
    assert_eq!(snap.connection, Connection::Jack);
    assert_eq!(snap.verdict.level, Level::Ok, "{:?}", snap.verdict);
    assert_eq!(snap.jack.as_ref().map(|j| j.label.as_str()), Some("Front"));
    assert_eq!(snap.audio.source.as_ref().map(|s| s.node_name.as_str()), Some(FRONT));
    let usb = snap.checks.iter().find(|c| c.id == CheckId::Usb).unwrap();
    assert_eq!(usb.level, CheckLevel::Ok);
    let radio = snap.checks.iter().find(|c| c.id == CheckId::Radio).unwrap();
    assert_eq!(radio.level, CheckLevel::Inactive);
}

#[test]
fn empty_sockets_do_not_count() {
    let state = state_with(vec![jack(201, FRONT, false), jack(202, BACK, false)], BACK);
    let snap = state.snapshot(10_000);
    assert_eq!(snap.connection, Connection::None);
    assert!(snap.jack.is_none());
    assert_eq!(snap.verdict.title, "Nicht verbunden");
}

#[test]
fn the_pick_wins_and_falls_back_when_unplugged() {
    let mut state = state_with(vec![jack(201, FRONT, true), jack(202, BACK, true)], FRONT);
    let snap = state.snapshot(10_000);
    let j = snap.jack.unwrap();
    assert_eq!(j.label, "Front", "the default input comes first");
    assert_eq!(j.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["Front", "Back"]);

    state.apply(Input::JackChoice(Some(BACK.into())), 11_000);
    assert_eq!(state.snapshot(11_000).jack.unwrap().label, "Back");
    state.apply(Input::MeterWanted(true), 11_000);
    assert_eq!(state.meter_target().as_deref(), Some(BACK));

    // The picked socket loses its plug: back to the other one.
    let state = {
        let mut s = state_with(vec![jack(201, FRONT, true), jack(202, BACK, false)], FRONT);
        s.apply(Input::JackChoice(Some(BACK.into())), 0);
        s
    };
    assert_eq!(state.snapshot(12_000).jack.unwrap().label, "Front");
}

#[test]
fn usb_receiver_beats_the_jack() {
    let mut state = State::new(true);
    for input in Scenario::Ready.inputs() {
        let input = match input {
            Input::Audio(AudioEvent::Graph(mut graph)) => {
                graph.sources.push(jack(201, FRONT, true));
                Input::Audio(AudioEvent::Graph(graph))
            }
            other => other,
        };
        state.apply(input, 0);
    }
    let snap = state.snapshot(1_000);
    assert_eq!(snap.connection, Connection::Usb);
    assert!(snap.jack.is_none());
}
