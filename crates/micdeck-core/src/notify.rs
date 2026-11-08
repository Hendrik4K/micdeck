//! Decides when a desktop notification is worth showing. Works on snapshots
//! only, so it is testable without a desktop.

use crate::verdict::{Level, Snapshot};

/// A problem must last this long before it is announced, so a replug or a
/// short radio dropout doesn't produce a notification.
pub const HOLD_MS: u64 = 3_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

fn notice(title: impl Into<String>, body: impl Into<String>) -> Notice {
    Notice { title: title.into(), body: body.into() }
}

#[derive(Debug, Default)]
pub struct Notifier {
    /// MicDeck was fully ready at least once; before that nothing is "lost".
    was_ready: bool,
    had_receiver: bool,
    /// Per transmitter: it was connected, and when it disappeared.
    tx_was_connected: Vec<bool>,
    tx_lost_since: Vec<Option<u64>>,
    error_since: Option<u64>,
    /// A problem was announced; the next "ready" gets announced too.
    problem_announced: bool,
}

impl Notifier {
    /// Feed the current snapshot; returns what should be shown now.
    pub fn update(&mut self, snap: &Snapshot, now_ms: u64) -> Vec<Notice> {
        let mut out = Vec::new();
        if snap.simulated {
            return out;
        }

        let has_receiver = snap.receiver.is_some();
        if self.had_receiver && !has_receiver && self.was_ready {
            out.push(notice("Empfänger getrennt", "Der DJI-Empfänger wurde vom USB getrennt."));
            self.problem_announced = true;
        }
        self.had_receiver = has_receiver;

        let n = snap.transmitters.len();
        self.tx_was_connected.resize(n, false);
        self.tx_lost_since.resize(n, None);
        for (i, tx) in snap.transmitters.iter().enumerate() {
            match tx.connected {
                Some(true) => {
                    self.tx_was_connected[i] = true;
                    self.tx_lost_since[i] = None;
                }
                Some(false) if self.tx_was_connected[i] => {
                    let since = *self.tx_lost_since[i].get_or_insert(now_ms);
                    if now_ms.saturating_sub(since) >= HOLD_MS {
                        out.push(notice(
                            format!("Sender {} getrennt", tx.unit),
                            format!("Sender {} hat keine Funkverbindung mehr zum Empfänger.", tx.unit),
                        ));
                        self.tx_was_connected[i] = false;
                        self.tx_lost_since[i] = None;
                        self.problem_announced = true;
                    }
                }
                // Unknown radio state (no status data, receiver gone): the
                // receiver notice covers it.
                _ => {
                    self.tx_was_connected[i] = false;
                    self.tx_lost_since[i] = None;
                }
            }
        }

        match snap.verdict.level {
            Level::Error => {
                let since = *self.error_since.get_or_insert(now_ms);
                if self.was_ready && !self.problem_announced && now_ms.saturating_sub(since) >= HOLD_MS {
                    out.push(notice(snap.verdict.title.clone(), snap.verdict.detail.clone()));
                    self.problem_announced = true;
                }
            }
            Level::Ok => {
                self.error_since = None;
                if self.problem_announced {
                    out.push(notice("Wieder einsatzbereit", snap.verdict.detail.clone()));
                    self.problem_announced = false;
                }
                self.was_ready = true;
            }
            _ => self.error_since = None,
        }
        out
    }
}
