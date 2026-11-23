//! What the on-screen messages say (S2.10.2), kept apart from drawing so it can be tested.

use std::time::Duration;

use crate::main_window::control_bar::clock;
use crate::main_window::seeking::Way;

/// Where playback is, for T and after a seek: "00:01:23 / 00:45:00", or the position alone
/// while the length is unknown (a live stream).
pub fn position(position: Duration, duration: Option<Duration>) -> String {
    let at = clock::format(position);
    match duration {
        Some(duration) => format!("{at} / {}", clock::format(duration)),
        None => at,
    }
}

/// A skip and where it landed: "+10 s · 00:01:23 / 00:45:00".
pub fn skip(way: Way, jump: Duration, landed: &str) -> String {
    let sign = match way {
        Way::Back => '−',
        Way::Forward => '+',
    };
    format!("{sign}{} · {landed}", length(jump))
}

/// The chapter just gone to: "Chapter 2 · Spring", or "Chapter 2" when that is its title.
pub fn chapter(number: usize, title: &str) -> String {
    let numbered = format!("Chapter {number}");
    if title == numbered { numbered } else { format!("{numbered} · {title}") }
}

/// A jump's length as people say it: "3 s", "1 min", "1 min 30 s", "5 min".
fn length(jump: Duration) -> String {
    let seconds = jump.as_secs();
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    match (minutes, seconds) {
        (0, seconds) => format!("{seconds} s"),
        (minutes, 0) => format!("{minutes} min"),
        (minutes, seconds) => format!("{minutes} min {seconds} s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn the_position_names_the_length_when_known() {
        assert_eq!(position(secs(83), Some(secs(2700))), "00:01:23 / 00:45:00");
        assert_eq!(position(secs(83), None), "00:01:23");
    }

    #[test]
    fn a_skip_says_how_far_and_where_it_landed() {
        assert_eq!(skip(Way::Forward, secs(10), "00:00:20"), "+10 s · 00:00:20");
        assert_eq!(skip(Way::Back, secs(60), "00:00:00"), "−1 min · 00:00:00");
        assert_eq!(skip(Way::Forward, secs(90), "x"), "+1 min 30 s · x");
        assert_eq!(skip(Way::Back, secs(300), "x"), "−5 min · x");
    }

    #[test]
    fn a_chapter_is_named_once() {
        assert_eq!(chapter(2, "Spring"), "Chapter 2 · Spring");
        assert_eq!(chapter(3, "Chapter 3"), "Chapter 3");
    }
}
