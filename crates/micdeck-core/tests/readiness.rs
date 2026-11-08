//! Readiness rules for the situations MicDeck must tell apart.

use micdeck_core::mock::{demo_level, DemoSignal, Scenario, DEMO_SOURCE};
use micdeck_core::{
    AudioEvent, AudioService, CheckId, CheckLevel, Input, Level, LinkState, MeterState,
    SignalStatus, Snapshot, State, UsbEvent,
};

fn state_for(scenario: Scenario, at_ms: u64) -> State {
    let mut state = State::new(true);
    for input in scenario.inputs() {
        state.apply(input, at_ms);
    }
    state
}

/// Start the meter on the demo input and feed `duration_ms` of `signal`,
/// with fresh telemetry along the way like a real receiver would send.
fn measure(state: &mut State, scenario: Scenario, signal: DemoSignal, from_ms: u64, duration_ms: u64) {
    state.apply(Input::MeterWanted(true), from_ms);
    state.apply(
        Input::Audio(AudioEvent::Meter(MeterState::Running {
            node_name: DEMO_SOURCE.into(),
            channels: 2,
            rate: 48_000,
        })),
        from_ms,
    );
    let mut t = from_ms;
    let mut next_telemetry = from_ms;
    while t <= from_ms + duration_ms {
        state.apply(
            Input::Audio(AudioEvent::Level(demo_level(signal, t - from_ms, DEMO_SOURCE, 2))),
            t,
        );
        if t >= next_telemetry {
            for input in scenario.periodic_inputs() {
                state.apply(input, t);
            }
            next_telemetry += 500;
        }
        t += 33;
    }
}

fn check(snapshot: &Snapshot, id: CheckId) -> &micdeck_core::Check {
    snapshot.checks.iter().find(|c| c.id == id).expect("check present")
}

#[test]
fn fresh_state_is_pending_not_disconnected() {
    let snap = State::new(false).snapshot(0);
    assert_eq!(snap.verdict.level, Level::Pending);
}

#[test]
fn ready_with_both_transmitters_and_sound_is_green() {
    let mut state = state_for(Scenario::Ready, 0);
    measure(&mut state, Scenario::Ready, DemoSignal::Speech, 100, 3_000);
    let snap = state.snapshot(3_100);
    assert_eq!(snap.verdict.level, Level::Ok, "{:?}", snap.verdict);
    assert_eq!(snap.verdict.title, "Einsatzbereit");
    assert!(snap.verdict.detail.contains("Beide Sender verbunden"));
    assert!(snap.verdict.detail.contains("Ton kommt an"));
    assert_eq!(snap.audio.signal.status, SignalStatus::Present);
    assert!(snap.checks.iter().all(|c| c.level == CheckLevel::Ok), "{:#?}", snap.checks);
    assert!(snap.simulated);
}

#[test]
fn ready_without_meter_relies_on_radio_status() {
    let state = state_for(Scenario::Ready, 0);
    let snap = state.snapshot(500);
    assert_eq!(snap.verdict.level, Level::Ok);
    assert_eq!(check(&snap, CheckId::Signal).level, CheckLevel::Inactive);
}

#[test]
fn receiver_without_transmitter_is_yellow_not_red() {
    let snap = state_for(Scenario::NoTransmitter, 0).snapshot(500);
    assert_eq!(snap.verdict.level, Level::Warn);
    assert_eq!(snap.verdict.title, "Verbunden – kein Sender aktiv");
    assert_eq!(check(&snap, CheckId::Usb).level, CheckLevel::Ok);
    assert_eq!(check(&snap, CheckId::Radio).level, CheckLevel::Warn);
    assert!(snap.transmitters.iter().all(|t| t.connected == Some(false)));
}

#[test]
fn transmitter_connected_but_only_digital_silence() {
    let mut state = state_for(Scenario::Silent, 0);
    measure(&mut state, Scenario::Silent, DemoSignal::Silence, 0, 2_500);
    let snap = state.snapshot(2_500);
    assert_eq!(snap.verdict.level, Level::Warn);
    assert_eq!(snap.verdict.title, "Verbunden – aber kein Ton");
    assert_eq!(snap.audio.signal.status, SignalStatus::Silent);
}

#[test]
fn short_silence_is_not_reported_yet() {
    let mut state = state_for(Scenario::Ready, 0);
    measure(&mut state, Scenario::Ready, DemoSignal::Speech, 0, 1_000);
    measure(&mut state, Scenario::Ready, DemoSignal::Silence, 1_033, 1_000);
    let snap = state.snapshot(2_033);
    assert_eq!(snap.audio.signal.status, SignalStatus::Present);
    assert_eq!(snap.verdict.level, Level::Ok);
}

#[test]
fn not_plugged_in_is_red() {
    let snap = state_for(Scenario::Unplugged, 0).snapshot(10_000);
    assert_eq!(snap.verdict.level, Level::Error);
    assert_eq!(snap.verdict.title, "Nicht verbunden");
    assert_eq!(check(&snap, CheckId::Usb).level, CheckLevel::Error);
    assert!(snap.receiver.is_none());
}

#[test]
fn cable_pulled_while_running_turns_red() {
    let mut state = state_for(Scenario::Ready, 0);
    state.apply(Input::Usb(UsbEvent::Removed { key: "demo-1".into() }), 1_000);
    // PipeWire drops the input shortly after the USB device disappears.
    let mut graph = match Scenario::Unplugged.inputs().pop() {
        Some(Input::Audio(AudioEvent::Graph(g))) => g,
        other => panic!("unexpected {other:?}"),
    };
    graph.default_source = None;
    state.apply(Input::Audio(AudioEvent::Graph(graph)), 1_100);
    let snap = state.snapshot(1_200);
    assert_eq!(snap.verdict.level, Level::Error);
    assert_eq!(snap.verdict.title, "Nicht verbunden");
}

#[test]
fn input_is_found_by_alsa_card_before_pipewire_knows_the_usb_ids() {
    // Seen with a real receiver plugged in while MicDeck runs: PipeWire lists
    // the input before its device reports vendor and product IDs.
    let mut state = state_for(Scenario::Ready, 0);
    let mut graph = match Scenario::Ready.inputs().pop() {
        Some(Input::Audio(AudioEvent::Graph(g))) => g,
        other => panic!("unexpected {other:?}"),
    };
    for source in &mut graph.sources {
        source.usb_vendor_id = None;
        source.usb_product_id = None;
    }
    state.apply(Input::Audio(AudioEvent::Graph(graph)), 100);
    let snap = state.snapshot(6_000);
    assert_eq!(check(&snap, CheckId::AudioInput).level, CheckLevel::Ok, "{:#?}", snap.checks);
    assert_ne!(snap.verdict.title, "Kein Audioeingang");
}

#[test]
fn usb_without_audio_input_waits_then_turns_red() {
    let state = state_for(Scenario::NoAudioInput, 0);
    let early = state.snapshot(1_000);
    assert_eq!(early.verdict.level, Level::Pending);
    let late = state.snapshot(6_000);
    assert_eq!(late.verdict.level, Level::Error);
    assert_eq!(late.verdict.title, "Kein Audioeingang");
    let audio = check(&late, CheckId::AudioInput);
    assert_eq!(audio.value, "Kein Audiotreiber geladen");
    assert!(audio.hint.is_some());
}

#[test]
fn missing_permission_with_sound_is_still_green() {
    let mut state = state_for(Scenario::NoPermission, 0);
    measure(&mut state, Scenario::NoPermission, DemoSignal::Speech, 0, 1_000);
    let snap = state.snapshot(1_000);
    assert_eq!(snap.verdict.level, Level::Ok, "{:?}", snap.verdict);
    let radio = check(&snap, CheckId::Radio);
    assert_eq!(radio.level, CheckLevel::Warn);
    assert!(radio.hint.as_deref().unwrap().contains("udev"));
    assert!(snap.transmitters.iter().all(|t| t.connected.is_none()), "radio state is unknown");
}

#[test]
fn missing_permission_without_sound_check_is_yellow() {
    let snap = state_for(Scenario::NoPermission, 0).snapshot(1_000);
    assert_eq!(snap.verdict.level, Level::Warn);
    assert_eq!(snap.verdict.title, "Verbunden – Sender-Status unbekannt");
}

#[test]
fn interface_busy_is_explained() {
    let snap = state_for(Scenario::Busy, 0).snapshot(1_000);
    let radio = check(&snap, CheckId::Radio);
    assert_eq!(radio.value, "Von anderem Programm belegt");
    assert_eq!(snap.receiver.as_ref().unwrap().link, LinkState::Busy);
}

#[test]
fn pipewire_down_is_red() {
    let snap = state_for(Scenario::PipeWireDown, 0).snapshot(1_000);
    assert_eq!(snap.verdict.level, Level::Error);
    assert_eq!(snap.verdict.title, "Audiosystem nicht erreichbar");
    assert_eq!(check(&snap, CheckId::AudioInput).level, CheckLevel::Error);
}

#[test]
fn other_default_microphone_is_yellow_and_named() {
    let snap = state_for(Scenario::NotDefault, 0).snapshot(500);
    assert_eq!(snap.verdict.level, Level::Warn);
    assert_eq!(snap.verdict.title, "Nicht als Standard ausgewählt");
    assert_eq!(check(&snap, CheckId::DefaultInput).value, "Standard ist „Eingebautes Mikrofon (Demo)“");
    assert_eq!(snap.audio.inputs.len(), 2);
}

#[test]
fn muted_input_is_yellow() {
    let snap = state_for(Scenario::Muted, 0).snapshot(500);
    assert_eq!(snap.verdict.level, Level::Warn);
    assert_eq!(snap.verdict.title, "Stummgeschaltet");
}

#[test]
fn clipping_is_a_warning_check_but_not_a_readiness_problem() {
    let mut state = state_for(Scenario::Clipping, 0);
    measure(&mut state, Scenario::Clipping, DemoSignal::Clipping, 0, 2_100);
    let snap = state.snapshot(2_100);
    assert!(snap.audio.signal.clipped_recently);
    assert_eq!(check(&snap, CheckId::Signal).value, "Übersteuert");
    assert_eq!(snap.verdict.level, Level::Ok);
}

#[test]
fn empty_battery_is_yellow() {
    let snap = state_for(Scenario::LowBattery, 0).snapshot(500);
    assert_eq!(snap.verdict.level, Level::Warn);
    assert_eq!(snap.verdict.title, "Akku fast leer");
    let battery = snap.transmitters[0].battery.as_ref().unwrap();
    assert!(battery.critical);
    assert_eq!(battery.bars, 0);
}

#[test]
fn stale_status_data_is_reported_after_a_grace_period() {
    let state = state_for(Scenario::Ready, 0);
    // Last frame at 0: still live at 1 s, a gap at 2 s, missing at 4 s.
    assert_eq!(check(&state.snapshot(1_000), CheckId::Radio).level, CheckLevel::Ok);
    assert_eq!(check(&state.snapshot(2_000), CheckId::Radio).level, CheckLevel::Pending);
    let late = state.snapshot(4_000);
    assert_eq!(check(&late, CheckId::Radio).value, "Keine Statusdaten");
    assert!(late.transmitters.iter().all(|t| t.connected.is_none()));
}

#[test]
fn level_frames_from_another_input_are_ignored() {
    let mut state = state_for(Scenario::Ready, 0);
    state.apply(Input::MeterWanted(true), 0);
    state.apply(
        Input::Audio(AudioEvent::Meter(MeterState::Running {
            node_name: DEMO_SOURCE.into(),
            channels: 2,
            rate: 48_000,
        })),
        0,
    );
    state.apply(
        Input::Audio(AudioEvent::Level(demo_level(DemoSignal::Speech, 0, "other.input", 2))),
        10,
    );
    assert_eq!(state.snapshot(20).audio.signal.status, SignalStatus::Unknown);
}

#[test]
fn meter_follows_visibility_and_audio_service() {
    let mut state = state_for(Scenario::Ready, 0);
    assert_eq!(state.meter_target(), None, "no meter while nothing shows it");
    state.apply(Input::MeterWanted(true), 0);
    assert_eq!(state.meter_target().as_deref(), Some(DEMO_SOURCE));
    state.apply(
        Input::Audio(AudioEvent::Service(AudioService::Unavailable { reason: "weg".into() })),
        10,
    );
    assert_eq!(state.meter_target(), None);
}

#[test]
fn every_scenario_serializes_to_json() {
    for scenario in Scenario::ALL {
        let mut state = state_for(scenario, 0);
        if let Some(signal) = scenario.signal() {
            measure(&mut state, scenario, signal, 0, 300);
        }
        let json = serde_json::to_string(&state.snapshot(300)).expect("serializes");
        assert!(json.contains("\"simulated\":true"), "{}", scenario.slug());
    }
}
