//! MicDeck's core, free of any hardware or OS access.
//!
//! The USB and PipeWire adapters report what they observe as [`Input`]s;
//! [`State`] folds them into one picture and [`State::snapshot`] turns that
//! into what the UI shows, including the green/yellow/red [`Verdict`]. Keeping
//! this crate pure makes every readiness rule unit-testable without a device.

pub mod device_settings;
pub mod dock;
pub mod level;
pub mod mock;
pub mod model;
pub mod notify;
pub mod voice;
mod state;
mod verdict;

pub use model::*;
pub use state::{Input, State};
pub use verdict::{
    AudioView, BatteryView, Check, CheckId, CheckLevel, Level, ReceiverView, SignalStatus,
    SignalView, Snapshot, SourceRef, TransmitterView, Verdict,
};
