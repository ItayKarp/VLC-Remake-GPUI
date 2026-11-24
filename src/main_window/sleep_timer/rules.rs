//! The sleep timer's rules (S8.13.1), kept apart from the window so they can be tested: its
//! choices, when it fades and fires, and what it says.
//!
//! A timer of minutes runs on the wall clock, not the app's monotonic clock: a Mac asleep when
//! it was due wakes past due, and the timer then fires at once.

use std::time::{Duration, SystemTime};

/// How long the sound fades out before the timer fires.
pub const FADE: Duration = Duration::from_secs(10);
/// The longest custom timer, a day.
pub const MAX_MINUTES: u32 = 24 * 60;
/// Seconds in a minute.
const MINUTE: u64 = 60;

/// What the Sleep Timer dropdown offers, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// No timer.
    Off,
    /// Stop after this many minutes.
    Minutes(u32),
    /// Stop when the item playing ends.
    EndOfItem,
    /// Stop after minutes the user types.
    Custom,
}

impl Choice {
    /// The dropdown's entries.
    pub const ALL: [Self; 8] = [
        Self::Off,
        Self::Minutes(15),
        Self::Minutes(30),
        Self::Minutes(45),
        Self::Minutes(60),
        Self::Minutes(90),
        Self::EndOfItem,
        Self::Custom,
    ];

    /// The entry's text.
    pub fn label(self) -> String {
        match self {
            Self::Off => "Off".into(),
            Self::Minutes(minutes) => format!("{minutes} min"),
            Self::EndOfItem => "End of current item".into(),
            Self::Custom => "Custom".into(),
        }
    }
}

/// A running timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timer {
    /// Fires at this time on the wall clock.
    At(SystemTime),
    /// Fires when the item playing ends.
    EndOfItem,
}

/// What a running timer does now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Nothing yet: the sound plays as set.
    Wait,
    /// The sound plays at this share of its volume, 1.0 down to 0.0.
    Fade(f32),
    /// Playback stops now.
    Fire,
}

impl Timer {
    /// A timer firing `minutes` from `now`.
    pub fn after_minutes(minutes: u32, now: SystemTime) -> Self {
        Self::At(now + Duration::from_secs(u64::from(minutes) * MINUTE))
    }

    /// How long until it fires: `None` for the end of an item whose length or position isn't
    /// known (`item_left`).
    pub fn left(self, now: SystemTime, item_left: Option<Duration>) -> Option<Duration> {
        match self {
            Self::At(due) => Some(due.duration_since(now).unwrap_or_default()),
            Self::EndOfItem => item_left,
        }
    }

    /// What to do at `now`, with `item_left` of the item to play. The end of an item fires on
    /// its own end, not here.
    pub fn step(self, now: SystemTime, item_left: Option<Duration>) -> Step {
        let Some(left) = self.left(now, item_left) else { return Step::Wait };
        match self {
            Self::At(_) if left.is_zero() => Step::Fire,
            _ if left < FADE => Step::Fade(left.as_secs_f32() / FADE.as_secs_f32()),
            _ => Step::Wait,
        }
    }
}

/// Text that isn't a number of minutes from 1 to [`MAX_MINUTES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invalid;

/// Reads a custom timer: whole minutes ("20", "20 min", "20m").
pub fn parse_minutes(text: &str) -> Result<u32, Invalid> {
    let text = text.trim().to_lowercase();
    let number = text.strip_suffix("min").or_else(|| text.strip_suffix('m')).unwrap_or(&text);
    let minutes: u32 = number.trim().parse().map_err(|_| Invalid)?;
    (1..=MAX_MINUTES).contains(&minutes).then_some(minutes).ok_or(Invalid)
}

/// The line under the dropdown while a timer runs: "Stops in 12 min", "Stops in 45 s", or for
/// the end of an item "Stops when this item ends".
pub fn hint(timer: Timer, left: Option<Duration>) -> String {
    match (timer, left) {
        (Timer::EndOfItem, _) | (Timer::At(_), None) => "Stops when this item ends".into(),
        (Timer::At(_), Some(left)) if left.as_secs() < MINUTE => {
            format!("Stops in {} s", left.as_secs())
        }
        // Rounded up, so "1 min" means it's under a minute and a bit.
        (Timer::At(_), Some(left)) => format!("Stops in {} min", left.as_secs().div_ceil(MINUTE)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn a_timer_waits_fades_over_its_last_seconds_then_fires() {
        let timer = Timer::after_minutes(15, at(0));
        assert_eq!(timer, Timer::At(at(900)));
        assert_eq!(timer.step(at(880), None), Step::Wait);
        assert_eq!(timer.step(at(890), None), Step::Wait);
        assert_eq!(timer.step(at(895), None), Step::Fade(0.5));
        assert_eq!(timer.step(at(900), None), Step::Fire);
    }

    #[test]
    fn a_timer_past_due_after_sleep_fires_at_once() {
        assert_eq!(Timer::At(at(900)).step(at(4000), None), Step::Fire);
    }

    #[test]
    fn the_end_of_an_item_fades_with_the_item_and_never_fires_by_itself() {
        let timer = Timer::EndOfItem;
        assert_eq!(timer.step(at(0), None), Step::Wait);
        assert_eq!(timer.step(at(0), Some(Duration::from_secs(30))), Step::Wait);
        assert_eq!(timer.step(at(0), Some(Duration::from_secs(2))), Step::Fade(0.2));
        assert_eq!(timer.step(at(0), Some(Duration::ZERO)), Step::Fade(0.0));
    }

    #[test]
    fn custom_minutes_are_whole_and_in_range() {
        assert_eq!(parse_minutes(" 20 "), Ok(20));
        assert_eq!(parse_minutes("20 min"), Ok(20));
        assert_eq!(parse_minutes("20m"), Ok(20));
        assert_eq!(parse_minutes("0"), Err(Invalid));
        assert_eq!(parse_minutes("1.5"), Err(Invalid));
        assert_eq!(parse_minutes("1441"), Err(Invalid));
        assert_eq!(parse_minutes("soon"), Err(Invalid));
    }

    #[test]
    fn the_hint_counts_down() {
        let timer = Timer::At(at(0));
        assert_eq!(hint(timer, Some(Duration::from_secs(661))), "Stops in 12 min");
        assert_eq!(hint(timer, Some(Duration::from_secs(45))), "Stops in 45 s");
        assert_eq!(hint(Timer::EndOfItem, None), "Stops when this item ends");
    }

    #[test]
    fn the_choices_read_as_designed() {
        let labels: Vec<_> = Choice::ALL.iter().map(|choice| choice.label()).collect();
        let expected =
            ["Off", "15 min", "30 min", "45 min", "60 min", "90 min", "End of current item"];
        assert_eq!(labels[..7], expected);
        assert_eq!(labels[7], "Custom");
    }
}
