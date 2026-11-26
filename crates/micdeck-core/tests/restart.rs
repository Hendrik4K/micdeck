//! Restarting the receiver from MicDeck.

use micdeck_core::mock::Scenario;
use micdeck_core::{Input, Level, State, UsbEvent};

fn ready(at_ms: u64) -> State {
    let mut state = State::new(true);
    for input in Scenario::Ready.inputs() {
        state.apply(input, at_ms);
    }
    state
}

fn telemetry(state: &mut State, at_ms: u64) {
    for input in Scenario::Ready.periodic_inputs() {
        state.apply(input, at_ms);
    }
}

#[test]
fn restart_needs_live_status_and_shows_until_the_microphone_is_back() {
    let mut state = ready(0);
    let (key, value) = state.restart_target(100).expect("ready receiver can restart");
    assert!(!key.is_empty() && !value.is_empty());

    state.apply(Input::RestartRequested, 1_000);
    let snap = state.snapshot(1_500);
    assert!(snap.restarting);
    assert_eq!(snap.verdict.level, Level::Pending);
    assert_eq!(snap.verdict.title, "Startet neu …");

    // It drops off USB while booting: still "restarting", not "Nicht verbunden".
    state.apply(Input::Usb(UsbEvent::Removed { key }), 1_600);
    assert_eq!(state.snapshot(3_000).verdict.title, "Startet neu …");

    // Back with a connected microphone after the minimum time: done.
    for input in Scenario::Ready.inputs() {
        state.apply(input, 8_500);
    }
    telemetry(&mut state, 8_500);
    assert!(!state.snapshot(8_600).restarting);
}

#[test]
fn restart_gives_up_showing_after_twenty_seconds() {
    let mut state = ready(0);
    state.apply(Input::RestartRequested, 1_000);
    assert!(state.snapshot(20_000).restarting);
    assert!(!state.snapshot(21_100).restarting);
}

#[test]
fn no_restart_without_status_data() {
    let state = ready(0);
    assert!(state.restart_target(60_000).is_err(), "status data is stale by then");
}
