//! Level math for the live meter: peak and RMS in dBFS, clipping and digital
//! silence, computed from interleaved `f32` samples (full scale = 1.0).

use crate::model::LevelFrame;

/// Lowest level reported; anything quieter (including true silence) reads as this.
pub const FLOOR_DBFS: f32 = -120.0;

/// A sample at or above this magnitude counts as clipping (≈ −0.01 dBFS). The
/// largest positive 16- or 24-bit sample converts to just below 1.0, so an
/// exact `>= 1.0` test would miss real clipping.
pub const CLIP_THRESHOLD: f32 = 0.999;

/// Convert a linear amplitude (1.0 = full scale) to dBFS.
pub fn to_dbfs(amplitude: f32) -> f32 {
    if amplitude.is_finite() && amplitude > 0.0 {
        (20.0 * amplitude.log10()).max(FLOOR_DBFS)
    } else {
        FLOOR_DBFS
    }
}

/// Collects samples until a meter window is complete.
#[derive(Debug, Clone)]
pub struct LevelAccumulator {
    peak: Vec<f32>,
    sum_sq: Vec<f64>,
    frames: u32,
    clipped: bool,
    any_nonzero: bool,
}

impl LevelAccumulator {
    pub fn new(channels: usize) -> Self {
        let channels = channels.max(1);
        Self {
            peak: vec![0.0; channels],
            sum_sq: vec![0.0; channels],
            frames: 0,
            clipped: false,
            any_nonzero: false,
        }
    }

    pub fn channels(&self) -> usize {
        self.peak.len()
    }

    /// Samples per channel collected so far.
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Add interleaved samples. A trailing partial frame is ignored.
    pub fn push_interleaved(&mut self, samples: &[f32]) {
        let ch = self.channels();
        for frame in samples.chunks_exact(ch) {
            for (c, &s) in frame.iter().enumerate() {
                let a = if s.is_finite() { s.abs() } else { 0.0 };
                if a > self.peak[c] {
                    self.peak[c] = a;
                }
                self.sum_sq[c] += f64::from(a) * f64::from(a);
                if a >= CLIP_THRESHOLD {
                    self.clipped = true;
                }
                if s != 0.0 {
                    self.any_nonzero = true;
                }
            }
            self.frames += 1;
        }
    }

    /// Add interleaved little-endian `f32` bytes as delivered by PipeWire.
    pub fn push_f32le_bytes(&mut self, bytes: &[u8]) {
        let mut buf = [0f32; 256];
        let mut n = 0;
        for sample in bytes.as_chunks::<4>().0 {
            buf[n] = f32::from_le_bytes(*sample);
            n += 1;
            // Flush whole frames only, so a frame never straddles two pushes.
            if n == buf.len() - buf.len() % self.channels() {
                self.push_interleaved(&buf[..n]);
                n = 0;
            }
        }
        self.push_interleaved(&buf[..n]);
    }

    /// Finish the window, returning its levels and starting a new one.
    pub fn finish(&mut self, node_name: &str, simulated: bool) -> LevelFrame {
        let frames = self.frames.max(1);
        let frame = LevelFrame {
            node_name: node_name.to_string(),
            peak_dbfs: self.peak.iter().map(|&p| to_dbfs(p)).collect(),
            rms_dbfs: self
                .sum_sq
                .iter()
                .map(|&s| to_dbfs((s / f64::from(frames)).sqrt() as f32))
                .collect(),
            clipped: self.clipped,
            digital_silence: self.frames > 0 && !self.any_nonzero,
            frames: self.frames,
            simulated,
        };
        *self = Self::new(self.channels());
        frame
    }
}

/// How fast the smoothed loudness follows a rising level (ms time constant).
const LOUDNESS_ATTACK_MS: f32 = 1_500.0;
/// How fast it falls back; slower, so pauses between words don't flip the label.
const LOUDNESS_RELEASE_MS: f32 = 6_000.0;
/// Largest drop a single quiet frame may pull towards (see `Loudness::update`).
const LOUDNESS_MAX_PULL_DB: f32 = 15.0;
/// A label only changes once the level is this far past the boundary.
const LOUDNESS_HYSTERESIS_DB: f32 = 2.0;

/// Plain-language loudness of what arrives at the PC, quietest first. The
/// boundaries are RMS dBFS of the louder channel and only a rough guide: the
/// same voice reads louder with more gain on the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoudnessClass {
    Silence,
    VeryQuiet,
    Quiet,
    Speech,
    Loud,
    VeryLoud,
}

impl LoudnessClass {
    /// Lower boundary of each class above `Silence`, in RMS dBFS.
    const BOUNDS: [(LoudnessClass, f32); 5] = [
        // Below this is room noise with nobody speaking (measured: about -61 dB
        // with a DJI Mic Mini at gain 0 in a quiet room).
        (LoudnessClass::VeryQuiet, -58.0),
        (LoudnessClass::Quiet, -50.0),
        (LoudnessClass::Speech, -38.0),
        (LoudnessClass::Loud, -24.0),
        (LoudnessClass::VeryLoud, -12.0),
    ];

    fn of(dbfs: f32) -> Self {
        Self::BOUNDS.iter().rev().find(|(_, low)| dbfs >= *low).map_or(Self::Silence, |(c, _)| *c)
    }

    fn lower_bound(self) -> f32 {
        Self::BOUNDS.iter().find(|(c, _)| *c == self).map_or(f32::NEG_INFINITY, |(_, low)| *low)
    }

    fn upper_bound(self) -> f32 {
        Self::BOUNDS.iter().find(|(c, _)| *c > self).map_or(f32::INFINITY, |(_, low)| *low)
    }

    pub fn label(self) -> &'static str {
        match self {
            // Room noise, nobody speaking; true digital silence is reported separately.
            Self::Silence => "Ruhig",
            Self::VeryQuiet => "Flüsterleise",
            Self::Quiet => "Leise",
            Self::Speech => "Normale Sprechlautstärke",
            Self::Loud => "Laut",
            Self::VeryLoud => "Sehr laut",
        }
    }
}

/// Smoothed loudness over the last seconds, for a calm label instead of the
/// jumpy instantaneous meter.
#[derive(Debug, Clone, Default)]
pub struct Loudness {
    dbfs: Option<f32>,
    class: Option<LoudnessClass>,
    last_ms: Option<u64>,
}

impl Loudness {
    /// Feed the RMS level of one meter frame measured at `now_ms`.
    pub fn update(&mut self, rms_dbfs: f32, now_ms: u64) {
        let target = rms_dbfs.max(FLOOR_DBFS);
        let smoothed = match (self.dbfs, self.last_ms) {
            (Some(current), Some(last)) => {
                // Cap the step so a gap in frames doesn't count as a long average.
                let dt = now_ms.saturating_sub(last).min(1_000) as f32;
                let tau = if target > current { LOUDNESS_ATTACK_MS } else { LOUDNESS_RELEASE_MS };
                // A pause reads as -90 dB or less; pulling towards that would
                // drop the label after every sentence. Limit how far below the
                // current value one frame may pull, so silence fades in gently.
                let target = target.max(current - LOUDNESS_MAX_PULL_DB);
                current + (target - current) * (1.0 - (-dt / tau).exp())
            }
            _ => target,
        };
        self.dbfs = Some(smoothed);
        self.last_ms = Some(now_ms);
        self.class = Some(match self.class {
            Some(c)
                if smoothed >= c.lower_bound() - LOUDNESS_HYSTERESIS_DB
                    && smoothed < c.upper_bound() + LOUDNESS_HYSTERESIS_DB =>
            {
                c
            }
            _ => LoudnessClass::of(smoothed),
        });
    }

    pub fn dbfs(&self) -> Option<f32> {
        self.dbfs
    }

    pub fn class(&self) -> Option<LoudnessClass> {
        self.class
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(l: &mut Loudness, dbfs: f32, from_ms: u64, to_ms: u64) {
        let mut t = from_ms;
        while t <= to_ms {
            l.update(dbfs, t);
            t += 33;
        }
    }

    #[test]
    fn loudness_rises_quickly_and_falls_slowly() {
        let mut l = Loudness::default();
        feed(&mut l, -54.0, 0, 3_000);
        assert_eq!(l.class(), Some(LoudnessClass::VeryQuiet));
        feed(&mut l, -30.0, 3_033, 7_000);
        assert_eq!(l.class(), Some(LoudnessClass::Speech), "{:?}", l.dbfs());
        // A one-second pause barely moves it.
        feed(&mut l, -90.0, 7_033, 8_000);
        assert_eq!(l.class(), Some(LoudnessClass::Speech), "{:?}", l.dbfs());
        // A long silence does.
        feed(&mut l, -90.0, 8_033, 40_000);
        assert_eq!(l.class(), Some(LoudnessClass::Silence), "{:?}", l.dbfs());
    }

    #[test]
    fn loudness_label_does_not_flicker_at_a_boundary() {
        let mut l = Loudness::default();
        feed(&mut l, -37.0, 0, 10_000);
        assert_eq!(l.class(), Some(LoudnessClass::Speech));
        // Hovering just under the -38 dB boundary keeps the label.
        feed(&mut l, -39.0, 10_033, 30_000);
        assert_eq!(l.class(), Some(LoudnessClass::Speech));
        feed(&mut l, -44.0, 30_033, 60_000);
        assert_eq!(l.class(), Some(LoudnessClass::Quiet));
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn dbfs_of_reference_levels() {
        assert!(close(to_dbfs(1.0), 0.0));
        assert!(close(to_dbfs(0.5), -6.02));
        assert!(close(to_dbfs(0.1), -20.0));
        assert_eq!(to_dbfs(0.0), FLOOR_DBFS);
        assert_eq!(to_dbfs(f32::NAN), FLOOR_DBFS);
    }

    #[test]
    fn full_scale_sine_has_minus_3_db_rms() {
        let mut acc = LevelAccumulator::new(1);
        let samples: Vec<f32> = (0..4800)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 1000.0 / 48000.0).sin())
            .collect();
        acc.push_interleaved(&samples);
        let f = acc.finish("src", false);
        assert!(close(f.peak_dbfs[0], 0.0), "peak {}", f.peak_dbfs[0]);
        assert!((f.rms_dbfs[0] + 3.01).abs() < 0.05, "rms {}", f.rms_dbfs[0]);
        assert!(f.clipped, "a full-scale sine touches 1.0");
        assert!(!f.digital_silence);
        assert_eq!(f.frames, 4800);
    }

    #[test]
    fn separates_channels_and_detects_silence() {
        let mut acc = LevelAccumulator::new(2);
        // Left at -6 dBFS, right exactly zero.
        acc.push_interleaved(&[0.5, 0.0, -0.5, 0.0, 0.25, 0.0]);
        let f = acc.finish("src", false);
        assert!(close(f.peak_dbfs[0], -6.02));
        assert_eq!(f.peak_dbfs[1], FLOOR_DBFS);
        assert!(!f.clipped);
        assert!(!f.digital_silence, "one channel carries signal");

        acc.push_interleaved(&[0.0; 8]);
        let f = acc.finish("src", false);
        assert!(f.digital_silence);
        assert_eq!(f.frames, 4);
    }

    #[test]
    fn byte_input_matches_sample_input_across_frame_boundaries() {
        let samples: Vec<f32> = (0..1001).map(|i| ((i % 17) as f32 - 8.0) / 10.0).collect();
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();

        let mut a = LevelAccumulator::new(3);
        a.push_interleaved(&samples);
        let mut b = LevelAccumulator::new(3);
        b.push_f32le_bytes(&bytes);
        assert_eq!(a.frames(), 333);
        assert_eq!(a.finish("x", false), b.finish("x", false));
    }

    #[test]
    fn finish_resets_the_window() {
        let mut acc = LevelAccumulator::new(1);
        acc.push_interleaved(&[1.0]);
        assert!(acc.finish("x", false).clipped);
        acc.push_interleaved(&[0.1]);
        let f = acc.finish("x", false);
        assert!(!f.clipped);
        assert!(close(f.peak_dbfs[0], -20.0));
    }
}
