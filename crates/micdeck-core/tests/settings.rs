//! Device settings: what may be changed when, and how a change is tracked.

use dji_protocol::DeviceStatus;
use micdeck_core::device_settings::{writable, SettingView};
use micdeck_core::mock::Scenario;
use micdeck_core::{Input, State, UsbEvent, WritePhase, WriteReport};

/// Real (not simulated) state fed with a scenario's receiver data.
fn live(scenario: Scenario) -> (State, String, DeviceStatus) {
    let mut state = State::new(false);
    let mut telemetry = None;
    for input in scenario.inputs() {
        if let Input::Usb(UsbEvent::Telemetry { key, status }) = &input {
            telemetry = Some((key.clone(), status.clone()));
        }
        state.apply(input, 0);
    }
    let (key, status) = telemetry.expect("scenario has telemetry");
    (state, key, status)
}

fn with_settings(state: &mut State, key: &str, status: &DeviceStatus, changes: &[(&str, &str)], at_ms: u64) {
    let mut status = status.clone();
    for (id, value) in changes {
        status.settings.insert(id.to_string(), value.to_string());
    }
    state.apply(Input::Usb(UsbEvent::Telemetry { key: key.into(), status }), at_ms);
}

fn view(state: &State, id: &str, now_ms: u64) -> SettingView {
    state.snapshot(now_ms).device_settings.into_iter().find(|s| s.id == id).expect("setting shown")
}

fn report_for(key: &str, request: u64, setting: &str, value: &str, phase: WritePhase) -> Input {
    Input::Usb(UsbEvent::Write {
        key: key.into(),
        report: WriteReport {
            request,
            setting: setting.into(),
            value: value.into(),
            phase,
            unix_ms: 1_700_000_000_000,
            seq: Some(1),
            packet: None,
        },
    })
}

#[test]
fn only_settings_marked_writable_are_ever_sent() {
    assert!(writable("low-cut", "on"));
    assert!(writable("stereo", "stereo"));
    assert!(!writable("low-cut", "loud"), "unknown values are refused");
    assert!(!writable("plug-free", "on"), "reboots the receiver");
    assert!(!writable("voice-tone", "rich"), "DJI Mic Mini 2 only");
    assert!(!writable("firmware", "on"));
}

#[test]
fn live_receiver_offers_its_settings_with_current_values() {
    let (state, _, _) = live(Scenario::Ready);
    let low_cut = view(&state, "low-cut", 100);
    assert_eq!(low_cut.value.as_deref(), Some("off"));
    assert_eq!(low_cut.locked, None);
    assert!(low_cut.switch);
    let channel = view(&state, "stereo", 100);
    assert!(!channel.switch);
    assert_eq!(channel.options.iter().map(|o| o.label).collect::<Vec<_>>(), ["Mono", "Stereo"]);
}

#[test]
fn unsupported_settings_stay_locked_and_say_why() {
    let (state, _, _) = live(Scenario::Ready);
    for id in ["plug-free", "voice-tone"] {
        let v = view(&state, id, 100);
        assert!(v.unsupported, "{id}");
        assert!(v.locked.is_some(), "{id}");
    }
    assert!(view(&state, "plug-free", 100).locked.unwrap().contains("neu"));
}

#[test]
fn stereo_and_backup_track_lock_each_other() {
    let (mut state, key, status) = live(Scenario::Ready);
    with_settings(&mut state, &key, &status, &[("stereo", "stereo")], 200);
    assert_eq!(view(&state, "safety-track", 300).locked.as_deref(), Some("Nur im Mono-Modus verfügbar."));
    assert_eq!(view(&state, "stereo", 300).locked, None);

    with_settings(&mut state, &key, &status, &[("stereo", "mono"), ("safety-track", "on")], 400);
    assert!(view(&state, "stereo", 500).locked.unwrap().contains("Backup-Aufnahme"));
}

#[test]
fn one_transmitter_stays_mono() {
    // Scenario::Silent has transmitter 1 only.
    let (mut state, key, status) = live(Scenario::Silent);
    let channel = view(&state, "stereo", 100);
    assert!(channel.fixed);
    assert!(channel.locked.unwrap().contains("zwei Sender"));
    assert!(state.setting_target("stereo", "stereo", 100).is_err());

    // Set to stereo elsewhere (e.g. DJI's app): switching back to mono stays possible.
    with_settings(&mut state, &key, &status, &[("stereo", "stereo")], 200);
    let channel = view(&state, "stereo", 300);
    assert!(!channel.fixed);
    assert_eq!(channel.locked, None);
    assert_eq!(state.setting_target("stereo", "mono", 300), Ok(key));
}

#[test]
fn two_transmitters_may_use_stereo() {
    let (state, _, _) = live(Scenario::Ready);
    let channel = view(&state, "stereo", 100);
    assert!(!channel.fixed);
    assert_eq!(channel.locked, None);
}

#[test]
fn noise_cancel_level_needs_noise_cancel_on() {
    let (mut state, key, status) = live(Scenario::Ready);
    with_settings(&mut state, &key, &status, &[("noise-cancel-power", "off")], 200);
    assert!(view(&state, "noise-cancel", 300).locked.unwrap().contains("Rauschminderung"));
}

#[test]
fn v2_only_settings_are_locked_on_v1_firmware() {
    let (mut state, key, mut status) = live(Scenario::Ready);
    status.protocol_version = Some(1);
    state.apply(Input::Usb(UsbEvent::Telemetry { key, status }), 200);
    assert!(view(&state, "noise-cancel-power", 300).locked.unwrap().contains("Firmware 02"));
    assert_eq!(view(&state, "low-cut", 300).locked, None, "exists on v1 too");
}

#[test]
fn per_sender_settings_need_a_sender() {
    let (state, _, _) = live(Scenario::NoTransmitter);
    let v = view(&state, "mic-leds", 100);
    assert_eq!(v.value, None);
    assert!(v.locked.unwrap().contains("Sender einschalten"));
    assert_eq!(view(&state, "clip-limiter", 100).locked, None, "a receiver setting");
}

#[test]
fn nothing_is_sent_in_demo_mode_or_without_a_receiver() {
    let mut demo = State::new(true);
    for input in Scenario::Ready.inputs() {
        demo.apply(input, 0);
    }
    assert!(demo.setting_target("low-cut", "on", 100).unwrap_err().contains("Demo"));
    assert!(view(&demo, "low-cut", 100).locked.unwrap().contains("Demo"));

    let empty = State::new(false);
    assert!(empty.setting_target("low-cut", "on", 100).is_err());
}

#[test]
fn requests_are_checked_before_sending() {
    let (state, key, _) = live(Scenario::Ready);
    assert_eq!(state.setting_target("low-cut", "on", 100), Ok(key));
    assert!(state.setting_target("low-cut", "off", 100).unwrap_err().contains("bereits"));
    assert!(state.setting_target("plug-free", "on", 100).is_err());
    assert!(state.setting_target("low-cut", "loud", 100).is_err());
}

#[test]
fn a_change_is_tracked_until_read_back() {
    let (mut state, key, status) = live(Scenario::Ready);
    state.apply(report_for(&key, 1, "low-cut", "on", WritePhase::Sent), 200);
    assert_eq!(view(&state, "low-cut", 250).pending.as_deref(), Some("on"));
    assert!(state.setting_target("mic-leds", "off", 250).unwrap_err().contains("läuft noch"));

    state.apply(report_for(&key, 1, "low-cut", "on", WritePhase::Acknowledged), 300);
    with_settings(&mut state, &key, &status, &[("low-cut", "on")], 350);
    state.apply(report_for(&key, 1, "low-cut", "on", WritePhase::Confirmed), 360);

    let snap = state.snapshot(400);
    let entry = &snap.device_log[0];
    assert_eq!(entry.label, "Hochpassfilter");
    assert_eq!((entry.from.as_deref(), entry.to.as_str()), (Some("Aus"), "An"));
    assert_eq!(entry.ok, Some(true));
    assert_eq!(entry.summary, "Übernommen und zurückgelesen");
    assert_eq!(view(&state, "low-cut", 400).pending, None);
    assert_eq!(view(&state, "low-cut", 400).value.as_deref(), Some("on"));
}

#[test]
fn a_change_without_answer_does_not_block_forever() {
    let (mut state, key, _) = live(Scenario::Ready);
    state.apply(report_for(&key, 7, "mic-leds", "off", WritePhase::Sent), 200);
    // Telemetry keeps arriving; the write never gets an answer.
    let (_, _, status) = live(Scenario::Ready);
    state.apply(Input::Usb(UsbEvent::Telemetry { key: key.clone(), status }), 11_000);
    let snap = state.snapshot(11_000);
    assert_eq!(snap.device_log[0].phase, WritePhase::NoAnswer);
    assert_eq!(snap.device_log[0].ok, Some(false));
    assert!(state.setting_target("low-cut", "on", 11_000).is_ok());
}

#[test]
fn unplugging_ends_a_running_change() {
    let (mut state, key, _) = live(Scenario::Ready);
    state.apply(report_for(&key, 3, "stereo", "stereo", WritePhase::Sent), 200);
    state.apply(Input::Usb(UsbEvent::Removed { key }), 300);
    let snap = state.snapshot(400);
    assert!(matches!(&snap.device_log[0].phase, WritePhase::Failed { message } if message.contains("abgezogen")));
}

#[test]
fn refused_and_rejected_changes_are_logged_as_failed() {
    let (mut state, key, _) = live(Scenario::Ready);
    state.apply(report_for(&key, 1, "low-cut", "on", WritePhase::Refused), 200);
    state.apply(
        report_for(&key, 2, "stereo", "stereo", WritePhase::Rejected { reason: "Erst die Backup-Aufnahme ausschalten.".into() }),
        300,
    );
    let log = state.snapshot(400).device_log;
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].summary, "Nicht gesendet: Erst die Backup-Aufnahme ausschalten.");
    assert_eq!(log[1].summary, "Vom Empfänger abgelehnt");
    assert!(log.iter().all(|e| e.ok == Some(false)));
}
