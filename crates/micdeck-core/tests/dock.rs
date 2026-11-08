//! Take-out / put-back events from the transmitter's charging-case flag.

use micdeck_core::dock::{DockEvent, DockWatcher};
use micdeck_core::mock::Scenario;
use micdeck_core::{Input, State, UsbEvent};

/// Real (not simulated) state of a ready receiver whose transmitter 1 is
/// `docked` at `at_ms`.
fn telemetry(state: &mut State, docked: bool, at_ms: u64) {
    for input in Scenario::Ready.periodic_inputs() {
        let Input::Usb(UsbEvent::Telemetry { key, mut status }) = input else { continue };
        if let Some(Some(tx)) = status.tx.get_mut(0) {
            tx.charging = Some(docked);
        }
        state.apply(Input::Usb(UsbEvent::Telemetry { key, status }), at_ms);
    }
}

fn ready(docked: bool) -> State {
    let mut state = State::new(false);
    for input in Scenario::Ready.inputs() {
        state.apply(input, 0);
    }
    telemetry(&mut state, docked, 0);
    state
}

/// Feed `docked` from `from_ms` to `to_ms` like a receiver does (10 Hz).
fn run(state: &mut State, w: &mut DockWatcher, docked: bool, from_ms: u64, to_ms: u64) -> Vec<DockEvent> {
    let mut events = Vec::new();
    for t in (from_ms..=to_ms).step_by(100) {
        telemetry(state, docked, t);
        events.extend(w.update(&state.snapshot(t), t));
    }
    events
}

#[test]
fn starting_state_fires_nothing() {
    let mut state = ready(false);
    let mut w = DockWatcher::default();
    assert!(run(&mut state, &mut w, false, 0, 3_000).is_empty());
}

#[test]
fn take_out_and_put_back_fire_once_each() {
    let mut state = ready(true);
    let mut w = DockWatcher::default();
    assert!(run(&mut state, &mut w, true, 0, 1_000).is_empty());
    assert_eq!(run(&mut state, &mut w, false, 1_100, 3_000), [DockEvent::TakenOut { unit: 1 }]);
    assert_eq!(run(&mut state, &mut w, true, 3_100, 5_000), [DockEvent::PutBack { unit: 1 }]);
}

#[test]
fn a_short_wobble_is_ignored() {
    let mut state = ready(true);
    let mut w = DockWatcher::default();
    run(&mut state, &mut w, true, 0, 1_000);
    // Out for 300 ms only, then back in.
    assert!(run(&mut state, &mut w, false, 1_100, 1_400).is_empty());
    assert!(run(&mut state, &mut w, true, 1_500, 3_000).is_empty());
}

#[test]
fn demo_data_fires_nothing() {
    let mut state = State::new(true);
    for input in Scenario::Ready.inputs() {
        state.apply(input, 0);
    }
    let mut w = DockWatcher::default();
    assert!(run(&mut state, &mut w, true, 0, 1_000).is_empty());
    assert!(run(&mut state, &mut w, false, 1_100, 3_000).is_empty());
}
