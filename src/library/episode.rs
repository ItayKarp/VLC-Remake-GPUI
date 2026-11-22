//! TV episode names (S4.3.3): `Show_S01E05`, `Show.1x05` and `Show - 105` become the show,
//! season and episode, for a nicer display title. The file name itself is kept, and shown where
//! the design shows it.

use std::fmt;

/// An episode named by its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Episode {
    /// The show's name, with dots and underscores read as spaces.
    pub show: String,
    /// The season number.
    pub season: u32,
    /// The episode's number in its season.
    pub number: u32,
    /// The episode's own title, when the name gives one after a ` - `.
    pub title: Option<String>,
}

impl fmt::Display for Episode {
    /// `Show S01E05`, or `Show S01E05 · Pilot`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} S{:02}E{:02}", self.show, self.season, self.number)?;
        match &self.title {
            Some(title) => write!(f, " · {title}"),
            None => Ok(()),
        }
    }
}

/// Where in a name the season and episode are.
struct Found {
    start: usize,
    end: usize,
    season: u32,
    episode: u32,
}

/// The episode a file name without its extension (`stem`) names, if it names one.
pub fn parse(stem: &str) -> Option<Episode> {
    let bytes = stem.as_bytes();
    let found = (0..bytes.len()).find_map(|at| {
        season_episode(bytes, at).or_else(|| cross(bytes, at)).or_else(|| dashed(bytes, at))
    })?;
    let show = clean(&stem[..found.start]);
    if show.is_empty() {
        return None;
    }
    let title = stem[found.end..].strip_prefix(" - ").map(clean).filter(|title| !title.is_empty());
    Some(Episode { show, season: found.season, number: found.episode, title })
}

/// `S01E05` at `at`, after a separator.
fn season_episode(bytes: &[u8], at: usize) -> Option<Found> {
    if !separated_before(bytes, at) || !bytes[at].eq_ignore_ascii_case(&b's') {
        return None;
    }
    let (season, after_season) = digits(bytes, at + 1, 1..=2)?;
    if !bytes.get(after_season)?.eq_ignore_ascii_case(&b'e') {
        return None;
    }
    let (episode, end) = digits(bytes, after_season + 1, 1..=3)?;
    let next = bytes.get(end);
    next.is_none_or(|next| !next.is_ascii_digit()).then_some(Found {
        start: at,
        end,
        season,
        episode,
    })
}

/// `1x05` at `at`, between separators.
fn cross(bytes: &[u8], at: usize) -> Option<Found> {
    if !separated_before(bytes, at) {
        return None;
    }
    let (season, after_season) = digits(bytes, at, 1..=2)?;
    if !bytes.get(after_season)?.eq_ignore_ascii_case(&b'x') {
        return None;
    }
    let (episode, end) = digits(bytes, after_season + 1, 2..=3)?;
    separated_after(bytes, end).then_some(Found { start: at, end, season, episode })
}

/// ` - 105` at `at`: three digits, the first the season. Four digits would be a year.
fn dashed(bytes: &[u8], at: usize) -> Option<Found> {
    if !bytes[at..].starts_with(b" - ") {
        return None;
    }
    let (number, end) = digits(bytes, at + 3, 3..=3)?;
    let found = Found { start: at, end, season: number / 100, episode: number % 100 };
    (separated_after(bytes, end) && found.season > 0).then_some(found)
}

/// The number written with between `count` digits from `at`, and where it ends.
fn digits(bytes: &[u8], at: usize, count: std::ops::RangeInclusive<usize>) -> Option<(u32, usize)> {
    let length = bytes.get(at..)?.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if !count.contains(&length) {
        return None;
    }
    let text = std::str::from_utf8(&bytes[at..at + length]).ok()?;
    Some((text.parse().ok()?, at + length))
}

fn separated_before(bytes: &[u8], at: usize) -> bool {
    at > 0 && !bytes[at - 1].is_ascii_alphanumeric()
}

fn separated_after(bytes: &[u8], at: usize) -> bool {
    bytes.get(at).is_none_or(|byte| !byte.is_ascii_alphanumeric())
}

/// A name part with dots and underscores as spaces, and no separators around it.
fn clean(part: &str) -> String {
    let spaced = part.replace(['.', '_'], " ");
    let trimmed = spaced.trim_matches(|c: char| c.is_whitespace() || c == '-' || c == '[');
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(show: &str, season: u32, number: u32, title: Option<&str>) -> Episode {
        Episode { show: show.into(), season, number, title: title.map(Into::into) }
    }

    #[test]
    fn the_three_common_namings_are_read() {
        assert_eq!(parse("Show_S01E05"), Some(episode("Show", 1, 5, None)));
        assert_eq!(parse("The.Show.1x05"), Some(episode("The Show", 1, 5, None)));
        assert_eq!(parse("The Show - 105"), Some(episode("The Show", 1, 5, None)));
    }

    #[test]
    fn a_title_after_a_dash_is_kept_and_release_tags_are_not() {
        assert_eq!(parse("Show - s02e10 - The End"), Some(episode("Show", 2, 10, Some("The End"))));
        assert_eq!(parse("Show.S01E05.720p.WEB"), Some(episode("Show", 1, 5, None)));
        assert_eq!(parse("Show S01E05E06"), Some(episode("Show", 1, 5, None)));
    }

    #[test]
    fn names_that_arent_episodes_stay_names() {
        assert_eq!(parse("Movie - 1999"), None);
        assert_eq!(parse("S01E05"), None);
        assert_eq!(parse("Bus 1x2"), None);
        assert_eq!(parse("Holiday"), None);
        assert_eq!(parse("Mixes - 099"), None);
    }

    #[test]
    fn episodes_display_with_padded_numbers() {
        let named = parse("Show - 105 - Pilot").expect("an episode");
        assert_eq!(named.to_string(), "Show S01E05 · Pilot");
    }
}
