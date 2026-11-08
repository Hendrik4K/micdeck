//! When MicDeck shows a desktop notification.

use micdeck_core::mock::Scenario;
use micdeck_core::notify::{Notice, Notifier};
use micdeck_core::{AudioEvent, Input, State, UsbEvent};

/// Real (not simulated) state fed with a scenario's inputs at `at_ms`.
fn live(state: &mut State, scenario: Scenario, at_ms: u64) {
    for input in scenario.inputs() {
        state.apply(input, at_ms);
    }
}

fn step(state: &State, n: &mut Notifier, now_ms: u64) -> Vec<String> {
    n.update(&state.snapshot(now_ms), now_ms).into_iter().map(|Notice { title, .. }| title).collect()
}

#[test]
fn nothing_is_announced_before_micdeck_was_ever_ready() {
    let mut state = State::new(false);
    live(&mut state, Scenario::Unplugged, 0);
    let mut n = Notifier::default();
    for t in (0..10_000).step_by(100) {
        assert!(step(&state, &mut n, t).is_empty());
    }
}

#[test]
fn unplugging_is_announced_once_and_the_comeback_too() {
    let mut state = State::new(false);
    let mut n = Notifier::default();
    live(&mut state, Scenario::Ready, 0);
    assert!(step(&state, &mut n, 500).is_empty());

    state.apply(Input::Usb(UsbEvent::Removed { key: "demo-1".into() }), 1_000);
    let graph = match Scenario::Unplugged.inputs().pop() {
        Some(Input::Audio(AudioEvent::Graph(g))) => g,
        other => panic!("unexpected {other:?}"),
    };
    state.apply(Input::Audio(AudioEvent::Graph(graph)), 1_050);
    assert_eq!(step(&state, &mut n, 1_100), ["Empfänger getrennt"]);
    // The red verdict that follows is the same problem: no second notice.
    for t in (1_200..8_000).step_by(100) {
        assert!(step(&state, &mut n, t).is_empty(), "at {t}");
    }

    live(&mut state, Scenario::Ready, 8_000);
    assert_eq!(step(&state, &mut n, 8_100), ["Wieder einsatzbereit"]);
    assert!(step(&state, &mut n, 8_200).is_empty());
}

#[test]
fn a_short_radio_dropout_is_silent_a_long_one_is_announced() {
    let mut state = State::new(false);
    let mut n = Notifier::default();
    live(&mut state, Scenario::Ready, 0);
    assert!(step(&state, &mut n, 500).is_empty());

    let lost = Scenario::NoTransmitter.periodic_inputs();
    let mut notices = Vec::new();
    for t in (1_000..=4_500).step_by(100) {
        for input in lost.clone() {
            state.apply(input, t);
        }
        let got = step(&state, &mut n, t);
        if t < 3_900 {
            assert!(got.is_empty(), "too early at {t}: {got:?}");
        }
        notices.extend(got);
    }
    assert_eq!(notices, ["Sender 1 getrennt", "Sender 2 getrennt"]);

    for input in Scenario::Ready.periodic_inputs() {
        state.apply(input, 5_000);
    }
    assert_eq!(step(&state, &mut n, 5_000), ["Wieder einsatzbereit"]);
}

#[test]
fn demo_data_never_notifies() {
    let mut state = State::new(true);
    let mut n = Notifier::default();
    live(&mut state, Scenario::Ready, 0);
    step(&state, &mut n, 100);
    state.apply(Input::Usb(UsbEvent::Removed { key: "demo-1".into() }), 200);
    assert!(step(&state, &mut n, 5_000).is_empty());
}
