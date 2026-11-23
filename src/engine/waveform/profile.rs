//! The peak profile (S7.8.2): a media's loudness in [`BUCKETS`] equal stretches of its length,
//! gathered from decoded samples as they come, so a part of it can be drawn while the rest is
//! still decoding.
//!
//! Each bucket keeps the root mean square of its samples rather than their peak: over a stretch
//! of seconds nearly every peak of music reaches the top, while its loudness still varies. The
//! levels are scaled so the loudest bucket is full height.

use std::time::Duration;

/// How many stretches the media's length is cut into.
pub const BUCKETS: usize = 600;
/// The largest level, a bucket as loud as the loudest.
pub const FULL: u8 = u8::MAX;

/// A media's loudness along its length, from its start to how far it has been decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peaks {
    /// One level per bucket, 0 to [`FULL`]; those past [`Self::known`] are 0.
    levels: Vec<u8>,
    /// How many buckets from the start are decoded.
    known: usize,
}

impl Peaks {
    /// The profile of a whole media from its stored `levels`; `None` unless there are
    /// [`BUCKETS`] of them.
    pub fn complete(levels: Vec<u8>) -> Option<Self> {
        (levels.len() == BUCKETS).then_some(Self { levels, known: BUCKETS })
    }

    /// One level per bucket, 0 to [`FULL`].
    pub fn levels(&self) -> &[u8] {
        &self.levels
    }

    /// How many buckets from the start are decoded.
    pub fn known(&self) -> usize {
        self.known
    }

    /// Whether the whole media is decoded.
    pub fn is_complete(&self) -> bool {
        self.known == BUCKETS
    }

    /// The loudest level, 0 to 1, between the fractions `from` and `to` of the length; `None`
    /// where that stretch isn't decoded yet.
    pub fn level_between(&self, from: f32, to: f32) -> Option<f32> {
        let first = bucket_at(from);
        let last = bucket_at(to).max(first);
        if last >= self.known {
            return None;
        }
        let loudest = self.levels[first..=last].iter().copied().max().unwrap_or_default();
        Some(f32::from(loudest) / f32::from(FULL))
    }
}

/// The bucket at `fraction` of the length.
fn bucket_at(fraction: f32) -> usize {
    ((fraction.clamp(0., 1.) * BUCKETS as f32) as usize).min(BUCKETS - 1)
}

/// Gathers a [`Peaks`] from samples in the order they play.
#[derive(Clone, Debug)]
pub struct Profile {
    /// Each bucket's sum of squared samples, and how many samples it holds.
    sums: Vec<(f64, u64)>,
    length: Duration,
    /// How long the samples so far play.
    played: Duration,
}

impl Profile {
    /// An empty profile of media `length` long.
    pub fn new(length: Duration) -> Self {
        Self { sums: vec![(0., 0); BUCKETS], length, played: Duration::ZERO }
    }

    /// Adds interleaved 16-bit `samples` of `channels` channels at `rate` frames a second.
    pub fn push(&mut self, samples: &[i16], channels: usize, rate: u32) {
        if channels == 0 || rate == 0 || self.length.is_zero() {
            return;
        }
        let frame_time = Duration::from_secs(1) / rate;
        for frame in samples.chunks_exact(channels) {
            let bucket = self.bucket_of(self.played);
            let (sum, count) = &mut self.sums[bucket];
            for &sample in frame {
                let level = f64::from(sample) / f64::from(i16::MAX);
                *sum += level * level;
                *count += 1;
            }
            self.played += frame_time;
        }
    }

    /// The profile so far; `complete` once every sample was pushed, which counts every
    /// bucket as decoded.
    pub fn peaks(&self, complete: bool) -> Peaks {
        let rms: Vec<f64> = self
            .sums
            .iter()
            .map(|&(sum, count)| if count == 0 { 0. } else { (sum / count as f64).sqrt() })
            .collect();
        let loudest = rms.iter().copied().fold(0., f64::max);
        let levels =
            rms.iter()
                .map(|level| {
                    if loudest > 0. { (level / loudest * f64::from(FULL)).round() as u8 } else { 0 }
                })
                .collect();
        let known = if complete { BUCKETS } else { self.bucket_of(self.played) };
        Peaks { levels, known }
    }

    /// The bucket playing at `time`; the last one past the length.
    fn bucket_of(&self, time: Duration) -> usize {
        let fraction = time.as_secs_f64() / self.length.as_secs_f64();
        ((fraction * BUCKETS as f64) as usize).min(BUCKETS - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One second of a constant `level` at 100 frames a second, in mono.
    fn second(level: i16) -> Vec<i16> {
        vec![level; 100]
    }

    #[test]
    fn the_loudest_bucket_is_full_height_and_silence_is_zero() {
        let mut profile = Profile::new(Duration::from_secs(2));
        profile.push(&second(i16::MAX / 2), 1, 100);
        profile.push(&second(0), 1, 100);
        let peaks = profile.peaks(true);
        assert_eq!(peaks.levels()[0], FULL);
        assert_eq!(peaks.levels()[BUCKETS - 1], 0);
        assert_eq!(peaks.level_between(0., 0.1), Some(1.));
        assert_eq!(peaks.level_between(0.9, 1.), Some(0.));
    }

    #[test]
    fn only_the_decoded_start_is_known_until_complete() {
        let mut profile = Profile::new(Duration::from_secs(4));
        profile.push(&second(1000), 1, 100);
        let partial = profile.peaks(false);
        assert_eq!(partial.known(), BUCKETS / 4);
        assert!(partial.level_between(0., 0.2).is_some());
        assert_eq!(partial.level_between(0.5, 0.6), None);
        assert!(profile.peaks(true).is_complete());
    }

    #[test]
    fn channels_share_a_frame_and_samples_past_the_end_fill_the_last_bucket() {
        let mut profile = Profile::new(Duration::from_secs(1));
        let stereo: Vec<i16> = [0, 8000].repeat(900);
        profile.push(&stereo, 2, 600);
        let peaks = profile.peaks(true);
        assert!(peaks.levels().iter().all(|level| *level == FULL));
    }

    #[test]
    fn stored_levels_must_have_every_bucket() {
        assert!(Peaks::complete(vec![0; BUCKETS]).is_some());
        assert!(Peaks::complete(vec![0; 3]).is_none());
    }
}
