//! The A-B loop (S8.13.4, VLC's Shift+L, not designed): playback goes back to point A whenever
//! it reaches point B. The first press marks A, the second B, the third ends the loop.

use std::time::Duration;

/// The shortest loop: closer marks would seek back before the first picture after A showed.
pub const MIN_SPAN: Duration = Duration::from_millis(500);
/// How far before the media's end B may be: libVLC may end the media before it reports a time
/// that close to its end, and the loop would then never wrap.
pub const END_MARGIN: Duration = Duration::from_millis(500);

/// The loop's marks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AbLoop {
    /// No loop.
    #[default]
    Off,
    /// A is marked; B isn't yet, so playback goes on.
    From(Duration),
    /// Playback goes back to `a` whenever it reaches `b`.
    Between {
        /// Where the loop starts.
        a: Duration,
        /// Where it goes back to `a`.
        b: Duration,
    },
}

impl AbLoop {
    /// The loop once the user marks `at` in media `duration` long (`None` while unknown): A,
    /// then B, then off. Marks in either order make the loop between them. A B closer than
    /// [`MIN_SPAN`] to A is moved [`MIN_SPAN`] past it, and one past [`END_MARGIN`] before the
    /// end is moved back; when that leaves no loop, A stays marked alone.
    pub fn marked(self, at: Duration, duration: Option<Duration>) -> Self {
        match self {
            Self::Off => Self::From(at),
            Self::From(a) => {
                let (a, b) = if at < a { (at, a) } else { (a, at) };
                let b = b.max(a + MIN_SPAN);
                let b = duration.map_or(b, |duration| b.min(duration.saturating_sub(END_MARGIN)));
                if b > a { Self::Between { a, b } } else { Self::From(a) }
            }
            Self::Between { .. } => Self::Off,
        }
    }

    /// Where playback goes back to, having reached `at`; `None` while it plays on.
    pub fn wraps(self, at: Duration) -> Option<Duration> {
        match self {
            Self::Between { a, b } if at >= b => Some(a),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn marks_go_a_then_b_then_off() {
        let looped = AbLoop::Off.marked(secs(10), Some(secs(60)));
        assert_eq!(looped, AbLoop::From(secs(10)));
        let looped = looped.marked(secs(20), Some(secs(60)));
        assert_eq!(looped, AbLoop::Between { a: secs(10), b: secs(20) });
        assert_eq!(looped.marked(secs(30), Some(secs(60))), AbLoop::Off);
    }

    #[test]
    fn a_b_before_a_swaps_them() {
        let looped = AbLoop::From(secs(20)).marked(secs(10), None);
        assert_eq!(looped, AbLoop::Between { a: secs(10), b: secs(20) });
    }

    #[test]
    fn a_short_loop_is_lengthened_and_one_at_the_end_pulled_back() {
        let short = AbLoop::From(secs(10)).marked(secs(10), None);
        assert_eq!(short, AbLoop::Between { a: secs(10), b: secs(10) + MIN_SPAN });
        let at_end = AbLoop::From(secs(10)).marked(secs(60), Some(secs(60)));
        assert_eq!(at_end, AbLoop::Between { a: secs(10), b: Duration::from_millis(59_500) });
    }

    #[test]
    fn marks_at_the_very_end_leave_a_alone() {
        let end = secs(60);
        let looped = AbLoop::From(end).marked(end, Some(end));
        assert_eq!(looped, AbLoop::From(end));
    }

    #[test]
    fn only_a_whole_loop_wraps_and_only_from_b_on() {
        assert_eq!(AbLoop::Off.wraps(secs(5)), None);
        assert_eq!(AbLoop::From(secs(1)).wraps(secs(5)), None);
        let looped = AbLoop::Between { a: secs(10), b: secs(20) };
        assert_eq!(looped.wraps(secs(19)), None);
        assert_eq!(looped.wraps(secs(20)), Some(secs(10)));
    }
}
