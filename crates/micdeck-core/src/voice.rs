//! Tells speech from room noise by level alone, for the microphone session:
//! when did someone start talking, how long have they been quiet, and – while
//! a take plays back over speakers – is the voice clearly louder than the
//! playback's own echo.

/// Speech must be this far above the room noise …
const START_MARGIN_DB: f32 = 12.0;
/// … and at least this loud, so a very quiet room doesn't make breathing count.
const START_MIN_DBFS: f32 = -50.0;
/// Quiet only counts once it is this far below the start threshold.
const END_HYSTERESIS_DB: f32 = 3.0;
/// How fast the noise estimate rises in quiet stretches (ms time constant).
const NOISE_RISE_MS: f32 = 2_000.0;
/// Starting point until the room has been heard.
const DEFAULT_NOISE_DBFS: f32 = -70.0;

#[derive(Debug, Clone, Default)]
pub struct VoiceDetector {
    noise_db: Option<f32>,
    /// Extra bar while a take plays back (echo protection).
    barge_db: Option<f32>,
    voice_ms: f32,
    silence_ms: f32,
}

impl VoiceDetector {
    pub fn threshold_db(&self) -> f32 {
        (self.noise_db.unwrap_or(DEFAULT_NOISE_DBFS) + START_MARGIN_DB).max(START_MIN_DBFS)
    }

    /// Feed the RMS level (louder channel, dBFS) of `dt_ms` of audio.
    pub fn feed(&mut self, rms_db: f32, dt_ms: f32) {
        let threshold = self.threshold_db();
        let start = self.barge_db.map_or(threshold, |b| b.max(threshold));
        if rms_db > start {
            self.voice_ms += dt_ms;
        } else {
            self.voice_ms = 0.0;
        }
        if rms_db < threshold - END_HYSTERESIS_DB {
            self.silence_ms += dt_ms;
        } else {
            self.silence_ms = 0.0;
        }
        // Room noise: follow drops at once, rises only slowly and only in
        // quiet stretches, so speech never becomes "noise".
        self.noise_db = Some(match self.noise_db {
            None => rms_db,
            Some(n) if rms_db < n => rms_db,
            Some(n) if rms_db < threshold => n + (rms_db - n) * (1.0 - (-dt_ms / NOISE_RISE_MS).exp()),
            Some(n) => n,
        });
    }

    /// Someone has been speaking for at least `min_ms`.
    pub fn speaking_for(&self, min_ms: f32) -> bool {
        self.voice_ms >= min_ms
    }

    /// How long it has been quiet (0 while speaking).
    pub fn silence_ms(&self) -> f32 {
        self.silence_ms
    }

    /// While playing back over speakers, only a voice above `echo_db` plus a
    /// margin may interrupt; `None` lifts the extra bar again.
    pub fn set_echo_level(&mut self, echo_db: Option<f32>, margin_db: f32) {
        self.barge_db = echo_db.map(|e| e + margin_db);
        self.voice_ms = 0.0;
    }

    pub fn reset_counters(&mut self) {
        self.voice_ms = 0.0;
        self.silence_ms = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(v: &mut VoiceDetector, db: f32, ms: u32) {
        for _ in 0..ms / 20 {
            v.feed(db, 20.0);
        }
    }

    #[test]
    fn speech_starts_above_room_noise_and_ends_after_quiet() {
        let mut v = VoiceDetector::default();
        feed(&mut v, -65.0, 1_000);
        // 12 dB above the room, but never below -50 dBFS.
        assert!((v.threshold_db() - (-50.0)).abs() < 0.5, "{}", v.threshold_db());
        feed(&mut v, -30.0, 100);
        assert!(!v.speaking_for(150.0), "100 ms is not speech yet");
        feed(&mut v, -30.0, 100);
        assert!(v.speaking_for(150.0));
        feed(&mut v, -64.0, 1_300);
        assert!(v.silence_ms() >= 1_200.0);
    }

    #[test]
    fn speech_does_not_raise_the_noise_estimate() {
        let mut v = VoiceDetector::default();
        feed(&mut v, -55.0, 1_000);
        feed(&mut v, -25.0, 10_000);
        assert!((v.threshold_db() - (-43.0)).abs() < 0.5, "{}", v.threshold_db());
    }

    #[test]
    fn echo_bar_blocks_playback_but_not_a_louder_voice() {
        let mut v = VoiceDetector::default();
        feed(&mut v, -65.0, 1_000);
        v.set_echo_level(Some(-40.0), 10.0);
        feed(&mut v, -40.0, 1_000);
        assert!(!v.speaking_for(250.0), "the echo itself must not interrupt");
        feed(&mut v, -22.0, 300);
        assert!(v.speaking_for(250.0));
        v.set_echo_level(None, 10.0);
        feed(&mut v, -40.0, 300);
        assert!(v.speaking_for(250.0), "without playback, -40 dB is speech again");
    }

    #[test]
    fn a_muted_input_never_counts_as_speech() {
        let mut v = VoiceDetector::default();
        feed(&mut v, -120.0, 3_000);
        assert!(!v.speaking_for(150.0));
        assert!(v.silence_ms() >= 2_900.0);
    }
}
