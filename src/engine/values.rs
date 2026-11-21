//! The small value types the engine's commands take: rates, volumes, signed offsets, seek
//! targets and the open generation.

use std::time::Duration;

/// Which `open` an event belongs to. Every open gets a new one, so events that arrive late from
/// a cancelled open can be recognized and dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(u64);

impl Generation {
    /// The generation after this one.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// The slowest playback rate offered.
pub const MIN_RATE: f32 = 0.25;
/// The fastest playback rate offered. Video may drop frames this fast.
pub const MAX_RATE: f32 = 8.0;

/// A playback speed multiplier, kept within [`MIN_RATE`]`..=`[`MAX_RATE`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rate(f32);

impl Rate {
    /// Normal speed.
    pub const NORMAL: Self = Self(1.0);

    /// The rate `value`, clamped into range; NaN becomes normal speed.
    pub fn new(value: f32) -> Self {
        if value.is_nan() { Self::NORMAL } else { Self(value.clamp(MIN_RATE, MAX_RATE)) }
    }

    /// The multiplier.
    pub fn get(self) -> f32 {
        self.0
    }
}

impl Default for Rate {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// The loudest volume the "Maximum volume" setting can allow: 300%.
pub const MAX_VOLUME: f32 = 3.0;
/// The "Maximum volume" setting's default: 125%, as VLC's volume slider.
pub const DEFAULT_MAX_VOLUME: Volume = Volume(1.25);
/// The lowest "Maximum volume" setting: 100%, the media's own level.
pub const MIN_MAX_VOLUME: Volume = Volume::FULL;

/// An output volume from silent (0.0) through 100% (1.0) to [`MAX_VOLUME`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume(f32);

impl Volume {
    /// 100%, the media's own level.
    pub const FULL: Self = Self(1.0);

    /// The volume `value`, clamped into range; NaN becomes silent.
    pub fn new(value: f32) -> Self {
        if value.is_nan() { Self(0.0) } else { Self(value.clamp(0.0, MAX_VOLUME)) }
    }

    /// The volume `percent`%, clamped into range.
    pub fn from_percent(percent: u32) -> Self {
        Self::new(percent as f32 / 100.0)
    }

    /// The level, where 1.0 is 100%.
    pub fn get(self) -> f32 {
        self.0
    }

    /// The level in whole percent, rounded.
    pub fn as_percent(self) -> u32 {
        (self.0 * 100.0).round() as u32
    }

    /// The quieter of the two.
    pub fn min(self, other: Self) -> Self {
        if other.0 < self.0 { other } else { self }
    }

    /// This volume raised or lowered by `step`, staying in range.
    pub fn stepped(self, change: VolumeChange, step: Self) -> Self {
        match change {
            VolumeChange::Up => Self::new(self.0 + step.0),
            VolumeChange::Down => Self::new(self.0 - step.0),
        }
    }
}

/// Which way a volume step goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VolumeChange {
    /// Louder.
    Up,
    /// Quieter.
    Down,
}

impl Default for Volume {
    fn default() -> Self {
        Self::FULL
    }
}

/// A signed time offset in milliseconds: an audio or subtitle delay, or a relative seek.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Offset(i64);

impl Offset {
    /// An offset of `millis` milliseconds; negative is earlier.
    pub fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// The offset in milliseconds.
    pub fn as_millis(self) -> i64 {
        self.0
    }

    /// `time` moved by this offset, stopping at zero.
    pub fn apply(self, time: Duration) -> Duration {
        let magnitude = Duration::from_millis(self.0.unsigned_abs());
        if self.0 < 0 { time.saturating_sub(magnitude) } else { time.saturating_add(magnitude) }
    }
}

/// Where the user asked to seek to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeekTarget {
    /// A time from the start.
    Absolute(Duration),
    /// A jump from the current position (arrow keys).
    Relative(Offset),
    /// A fraction of the duration, 0.0 to 1.0 (the seek bar).
    Fraction(f32),
}

/// A seek the backend carries out: a target resolved against what the player knows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackendSeek {
    /// Seek to this time.
    Time(Duration),
    /// Seek to this fraction of the media; used while the duration is unknown.
    Fraction(f32),
}

/// Which way a frame step goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepDirection {
    /// Show the next frame.
    Forward,
    /// Show the previous frame.
    Backward,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_and_volume_stay_in_range() {
        assert_eq!(Rate::new(0.1), Rate::new(MIN_RATE));
        assert_eq!(Rate::new(100.0), Rate::new(MAX_RATE));
        assert_eq!(Rate::new(f32::NAN), Rate::NORMAL);
        assert_eq!(Volume::new(-1.0), Volume::new(0.0));
        assert_eq!(Volume::new(9.0), Volume::new(MAX_VOLUME));
        assert_eq!(Volume::new(f32::NAN), Volume::new(0.0));
    }

    #[test]
    fn volumes_convert_and_step() {
        assert_eq!(Volume::from_percent(150), Volume::new(1.5));
        assert_eq!(Volume::new(1.234).as_percent(), 123);
        assert_eq!(
            Volume::from_percent(98)
                .stepped(VolumeChange::Up, Volume::from_percent(5))
                .as_percent(),
            103
        );
        assert_eq!(
            Volume::from_percent(3).stepped(VolumeChange::Down, Volume::from_percent(5)),
            Volume::new(0.0)
        );
        assert_eq!(Volume::FULL.min(DEFAULT_MAX_VOLUME), Volume::FULL);
    }

    #[test]
    fn offsets_move_time_and_stop_at_zero() {
        let ten = Duration::from_secs(10);
        assert_eq!(Offset::from_millis(5_000).apply(ten), Duration::from_secs(15));
        assert_eq!(Offset::from_millis(-5_000).apply(ten), Duration::from_secs(5));
        assert_eq!(Offset::from_millis(-60_000).apply(ten), Duration::ZERO);
    }

    #[test]
    fn generations_increase() {
        let first = Generation::default();
        assert!(first.next() > first);
    }
}
