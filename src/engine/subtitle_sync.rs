//! Subtitle synchronization (S6.3): how one subtitle track's times line up with the video.
//!
//! - **Delay** moves the subtitles later (positive) or earlier, up to ten minutes either way.
//! - **Speed** runs the file's clock faster or slower than the video's, for a file timed for
//!   another frame rate: a file made for 23.976 fps on a 25 fps video needs 25 / 23.976 ≈ 1.043.
//! - **Duration factor** keeps each line up longer, as VLC's: 0 is off, 2 shows each line twice
//!   as long, never past the start of the next one.
//!
//! The sync belongs to one track of one media (D24), named by a [`SubtitleKey`], so a delay set
//! for a file doesn't follow to an embedded track picked later.

use std::path::PathBuf;
use std::time::Duration;

use super::values::Offset;

/// The furthest a delay goes either way: ten minutes.
pub const MAX_DELAY_MS: i64 = 600_000;
/// The slowest and fastest subtitle speed.
pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 2.;
/// The longest the duration factor stretches a line.
pub const MAX_DURATION_FACTOR: f32 = 20.;

/// Which subtitle track a sync belongs to, the same each time the media opens.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SubtitleKey {
    /// A subtitle file beside the video or added to it, by its path.
    File(PathBuf),
    /// One of the media's own subtitle streams, by the backend's id.
    Stream(i32),
}

/// One subtitle track's delay, speed and duration factor, each kept in its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubtitleSync {
    delay: Offset,
    speed: f32,
    duration_factor: f32,
}

impl Default for SubtitleSync {
    fn default() -> Self {
        Self { delay: Offset::default(), speed: 1., duration_factor: 0. }
    }
}

impl SubtitleSync {
    /// The delay: positive shows the subtitles later.
    pub fn delay(self) -> Offset {
        self.delay
    }

    /// How fast the file's clock runs against the video's; 1 is as written.
    pub fn speed(self) -> f32 {
        self.speed
    }

    /// How much longer each line stays up; 0 is off.
    pub fn duration_factor(self) -> f32 {
        self.duration_factor
    }

    /// This sync with `delay`, kept within [`MAX_DELAY_MS`].
    #[must_use]
    pub fn with_delay(self, delay: Offset) -> Self {
        let millis = delay.as_millis().clamp(-MAX_DELAY_MS, MAX_DELAY_MS);
        Self { delay: Offset::from_millis(millis), ..self }
    }

    /// This sync with `speed`, kept within [`MIN_SPEED`] and [`MAX_SPEED`]; a speed that isn't
    /// a number is ignored.
    #[must_use]
    pub fn with_speed(self, speed: f32) -> Self {
        if speed.is_nan() {
            return self;
        }
        Self { speed: speed.clamp(MIN_SPEED, MAX_SPEED), ..self }
    }

    /// This sync with `factor`, kept within 0 and [`MAX_DURATION_FACTOR`]; one that isn't a
    /// number is ignored.
    #[must_use]
    pub fn with_duration_factor(self, factor: f32) -> Self {
        if factor.is_nan() {
            return self;
        }
        Self { duration_factor: factor.clamp(0., MAX_DURATION_FACTOR), ..self }
    }

    /// Whether this changes nothing.
    pub fn is_default(self) -> bool {
        self == Self::default()
    }

    /// The time in the subtitle file to show at `media_time`: the delay taken off, then the
    /// file's clock run at its speed.
    pub fn file_time(self, media_time: Duration) -> Duration {
        let delayed = Offset::from_millis(-self.delay.as_millis()).apply(media_time);
        delayed.mul_f64(f64::from(self.speed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_stay_in_range() {
        let sync = SubtitleSync::default()
            .with_delay(Offset::from_millis(-900_000))
            .with_speed(5.)
            .with_duration_factor(-1.);
        assert_eq!(sync.delay().as_millis(), -MAX_DELAY_MS);
        assert!((sync.speed() - MAX_SPEED).abs() < f32::EPSILON);
        assert!(sync.duration_factor().abs() < f32::EPSILON);
        assert_eq!(sync.with_speed(f32::NAN), sync);
    }

    #[test]
    fn file_time_takes_the_delay_off_then_runs_at_the_speed() {
        let sync = SubtitleSync::default().with_delay(Offset::from_millis(2_000)).with_speed(2.);
        assert_eq!(sync.file_time(Duration::from_secs(5)), Duration::from_secs(6));
        assert_eq!(sync.file_time(Duration::from_secs(1)), Duration::ZERO);
        let earlier = SubtitleSync::default().with_delay(Offset::from_millis(-500));
        assert_eq!(earlier.file_time(Duration::from_secs(1)), Duration::from_millis(1_500));
    }

    #[test]
    fn the_default_changes_nothing() {
        assert!(SubtitleSync::default().is_default());
        assert_eq!(
            SubtitleSync::default().file_time(Duration::from_secs(3)),
            Duration::from_secs(3)
        );
        assert!(!SubtitleSync::default().with_duration_factor(1.5).is_default());
    }
}
