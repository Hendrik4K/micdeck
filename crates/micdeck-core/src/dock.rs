//! Turns the "transmitter sits in the charging case" flag into clean
//! take-out / put-back events, for triggers such as starting dictation.

use crate::verdict::Snapshot;

/// A change must hold this long before it counts, so a wobble in the case
/// doesn't fire twice.
pub const STABLE_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockEvent {
    /// Transmitter `unit` was taken out of the charging case.
    TakenOut { unit: u8 },
    /// Transmitter `unit` was put back into the charging case.
    PutBack { unit: u8 },
}

#[derive(Debug, Default, Clone)]
struct Track {
    /// Last state that held for `STABLE_MS`; `None` until one has.
    settled: Option<bool>,
    candidate: Option<(bool, u64)>,
}

#[derive(Debug, Default)]
pub struct DockWatcher {
    tracks: Vec<Track>,
}

impl DockWatcher {
    /// Feed the current snapshot; returns the events that happened.
    pub fn update(&mut self, snap: &Snapshot, now_ms: u64) -> Vec<DockEvent> {
        let mut out = Vec::new();
        if snap.simulated {
            return out;
        }
        self.tracks.resize(snap.transmitters.len(), Track::default());
        for (tx, track) in snap.transmitters.iter().zip(&mut self.tracks) {
            // Only a connected transmitter that reports the flag counts; an
            // unknown state never fires anything.
            let docked = match (tx.connected, tx.charging) {
                (Some(true), Some(d)) => d,
                _ => {
                    track.candidate = None;
                    continue;
                }
            };
            if track.settled == Some(docked) {
                track.candidate = None;
                continue;
            }
            let since = match track.candidate {
                Some((d, t)) if d == docked => t,
                _ => {
                    track.candidate = Some((docked, now_ms));
                    now_ms
                }
            };
            if now_ms.saturating_sub(since) < STABLE_MS {
                continue;
            }
            // The first settled state is only the starting point.
            if track.settled.is_some() {
                out.push(if docked {
                    DockEvent::PutBack { unit: tx.unit }
                } else {
                    DockEvent::TakenOut { unit: tx.unit }
                });
            }
            track.settled = Some(docked);
            track.candidate = None;
        }
        out
    }
}
