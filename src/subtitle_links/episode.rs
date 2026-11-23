//! The episode a file name gives (S6.4.2): `S01E05`, `S01E05E06` and `S01E05-E06`, `1x05`,
//! `105` (season 1, episode 5), `E05`, `Ep 5` or `Episode 5`, and a bare `05` as an absolute
//! episode number, tried in that order.
//!
//! Years (`2019`) and resolutions (`720`, `1080`) are not episodes.

/// The most seasons a three- or four-digit number (`105`, `1012`) is read as.
const MAX_SEASON: u32 = 30;
/// Numbers that are picture heights, not episodes.
const RESOLUTIONS: [u32; 6] = [360, 480, 576, 720, 1080, 2160];
/// The years a four-digit number is taken for instead of an episode.
const YEARS: std::ops::RangeInclusive<u32> = 1900..=2099;
/// Words that announce an episode number in the next word.
const EPISODE_WORDS: [&str; 3] = ["e", "ep", "episode"];

/// A way of finding an episode among a name's words.
type Finder = fn(&[String]) -> Option<Episode>;

/// The episodes a file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Episode {
    /// The season, when the name gives one; `0` holds specials.
    pub season: Option<u32>,
    /// The first episode.
    pub first: u32,
    /// The last episode of a multi-episode file (`S01E05E06`); `first` otherwise.
    pub last: u32,
}

impl Episode {
    fn single(season: Option<u32>, episode: u32) -> Self {
        Self { season, first: episode, last: episode }
    }

    /// Whether `self` and `other` are the same episode: their ranges overlap, in the same
    /// season when both name one. A file without a season (absolute numbering) matches by
    /// number alone.
    pub fn matches(self, other: Self) -> bool {
        let seasons_agree = match (self.season, other.season) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        };
        seasons_agree && self.first <= other.last && other.first <= self.last
    }
}

/// The episode `name` (a file name without its extension) gives, if any.
pub fn parse(name: &str) -> Option<Episode> {
    let words: Vec<String> = name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let finders: [Finder; 4] = [season_episode, cross, episode_word, packed];
    finders.iter().find_map(|find| find(&words)).or_else(|| absolute(&words))
}

/// `s01e05`, `s01e05e06`, or `s01e05` followed by the word `e06`.
fn season_episode(words: &[String]) -> Option<Episode> {
    words.iter().enumerate().find_map(|(index, word)| {
        let rest = word.strip_prefix('s')?;
        let (season, rest) = leading_number(rest)?;
        let rest = rest.strip_prefix('e')?;
        let (first, rest) = leading_number(rest)?;
        let last = match rest.strip_prefix('e').and_then(leading_number) {
            Some((last, "")) => last,
            _ if rest.is_empty() => next_episode(words.get(index + 1)).unwrap_or(first),
            _ => return None,
        };
        Some(Episode { season: Some(season), first, last: last.max(first) })
    })
}

/// The episode in a word that continues a range: `e06` after `s01e05-`.
fn next_episode(word: Option<&String>) -> Option<u32> {
    match leading_number(word?.strip_prefix('e')?)? {
        (episode, "") => Some(episode),
        _ => None,
    }
}

/// `1x05`.
fn cross(words: &[String]) -> Option<Episode> {
    words.iter().find_map(|word| {
        let (season, rest) = leading_number(word)?;
        match leading_number(rest.strip_prefix('x')?)? {
            (episode, "") => Some(Episode::single(Some(season), episode)),
            _ => None,
        }
    })
}

/// `e05` and `ep05` as one word, or `ep`, `e` or `episode` followed by a number.
fn episode_word(words: &[String]) -> Option<Episode> {
    words.iter().enumerate().find_map(|(index, word)| {
        let glued = ["ep", "e"].iter().find_map(|prefix| {
            match leading_number(word.strip_prefix(prefix)?)? {
                (episode, "") => Some(episode),
                _ => None,
            }
        });
        let announced =
            || EPISODE_WORDS.contains(&word.as_str()).then(|| number(words.get(index + 1)))?;
        glued.or_else(announced).map(|episode| Episode::single(None, episode))
    })
}

/// A three- or four-digit number that is neither a year nor a resolution: `105` is season 1,
/// episode 5, and `1012` season 10, episode 12.
fn packed(words: &[String]) -> Option<Episode> {
    words.iter().find_map(|word| {
        if !(3..=4).contains(&word.len()) {
            return None;
        }
        let value = number(Some(word))?;
        if YEARS.contains(&value) || RESOLUTIONS.contains(&value) {
            return None;
        }
        let (season, episode) = (value / 100, value % 100);
        (season <= MAX_SEASON && episode > 0).then(|| Episode::single(Some(season), episode))
    })
}

/// The last one- or two-digit number, as an absolute episode number (`Show - 05`).
fn absolute(words: &[String]) -> Option<Episode> {
    words
        .iter()
        .rev()
        .filter(|word| word.len() <= 2)
        .find_map(|word| number(Some(word)))
        .filter(|episode| *episode > 0)
        .map(|episode| Episode::single(None, episode))
}

/// The number `word` is made of, if it is all digits.
fn number(word: Option<&String>) -> Option<u32> {
    let word = word?;
    if word.is_empty() || !word.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    word.parse().ok()
}

/// The digits at the start of `text`, as a number, and what follows them.
fn leading_number(text: &str) -> Option<(u32, &str)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 4 {
        return None;
    }
    Some((text[..digits].parse().ok()?, &text[digits..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(season: Option<u32>, first: u32, last: u32) -> Option<Episode> {
        let episode = Episode { season, first, last };
        (first <= last).then_some(episode)
    }

    #[test]
    fn season_and_episode_read_in_every_common_form() {
        assert_eq!(parse("The_Wand_S01E05"), episode(Some(1), 5, 5));
        assert_eq!(parse("the.wand.s02e10.720p"), episode(Some(2), 10, 10));
        assert_eq!(parse("Show 1x05"), episode(Some(1), 5, 5));
        assert_eq!(parse("Show.105.HDTV"), episode(Some(1), 5, 5));
        assert_eq!(parse("Show S00E03 Special"), episode(Some(0), 3, 3));
    }

    #[test]
    fn multi_episode_files_hold_a_range() {
        assert_eq!(parse("Show.S01E05E06"), episode(Some(1), 5, 6));
        assert_eq!(parse("Show.S01E05-E06"), episode(Some(1), 5, 6));
    }

    #[test]
    fn episodes_without_a_season_are_absolute() {
        assert_eq!(parse("Show - Episode 12"), episode(None, 12, 12));
        assert_eq!(parse("Show Ep05"), episode(None, 5, 5));
        assert_eq!(parse("Show - 05"), episode(None, 5, 5));
    }

    #[test]
    fn years_and_resolutions_are_not_episodes() {
        assert_eq!(parse("Movie 2019 1080"), None);
        assert_eq!(parse("Movie.2019"), None);
        assert_eq!(parse("Movie"), None);
    }

    #[test]
    fn episodes_match_by_overlap_within_a_season() {
        let double = Episode { season: Some(1), first: 5, last: 6 };
        assert!(double.matches(Episode::single(Some(1), 6)));
        assert!(!double.matches(Episode::single(Some(2), 6)));
        assert!(double.matches(Episode::single(None, 5)));
        assert!(!double.matches(Episode::single(Some(1), 7)));
    }
}
