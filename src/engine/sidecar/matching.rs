//! "File name matching" (S6.1.1): whether a subtitle file's name belongs to the video's, at each
//! level from Exact name to Any subtitle in the folder.

use crate::engine::subtitle_style::FileMatching;

/// Whether a subtitle file named `name` (without its extension) goes with a video titled `title`
/// (its file name without the extension) at `level`. Case is ignored.
pub(super) fn matches(title: &str, name: &str, level: FileMatching) -> bool {
    match level {
        FileMatching::Exact => name.eq_ignore_ascii_case(title),
        FileMatching::TitleWithExtra => starts_with_title(title, name),
        FileMatching::ContainingTitle => {
            let title = letters_and_digits(title);
            !title.is_empty() && letters_and_digits(name).contains(&title)
        }
        FileMatching::AnyInFolder => true,
    }
}

/// `name` is `title`, or `title` followed by something that isn't a letter or digit
/// (`Movie.en`, `Movie [Forced]`), so "Movies" isn't taken for "Movie".
fn starts_with_title(title: &str, name: &str) -> bool {
    let Some(rest) = name.get(title.len()..) else { return false };
    name[..title.len()].eq_ignore_ascii_case(title)
        && rest.chars().next().is_none_or(|next| !next.is_alphanumeric())
}

/// `text`'s letters and digits in lower case: "[Group] Movie (2020)" → "groupmovie2020".
fn letters_and_digits(text: &str) -> String {
    text.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels(name: &str) -> Vec<FileMatching> {
        let all = [
            FileMatching::Exact,
            FileMatching::TitleWithExtra,
            FileMatching::ContainingTitle,
            FileMatching::AnyInFolder,
        ];
        all.into_iter().filter(|level| matches("Movie (2020)", name, *level)).collect()
    }

    #[test]
    fn each_level_takes_the_names_of_the_stricter_ones() {
        use FileMatching::{AnyInFolder, ContainingTitle, Exact, TitleWithExtra};
        assert_eq!(levels("movie (2020)"), [Exact, TitleWithExtra, ContainingTitle, AnyInFolder]);
        assert_eq!(levels("Movie (2020).en"), [TitleWithExtra, ContainingTitle, AnyInFolder]);
        assert_eq!(levels("[Group] Movie.2020"), [ContainingTitle, AnyInFolder]);
        assert_eq!(levels("Other"), [AnyInFolder]);
    }

    #[test]
    fn the_title_must_end_where_a_word_ends() {
        assert!(!matches("Movie", "Movies", FileMatching::TitleWithExtra));
        assert!(!matches("Movie", "Mov", FileMatching::TitleWithExtra));
        assert!(matches("Movie", "Movie_2", FileMatching::TitleWithExtra));
    }

    #[test]
    fn titles_in_other_scripts_match() {
        assert!(matches("סרט", "סרט.he", FileMatching::TitleWithExtra));
        assert!(!matches("סרט", "ס", FileMatching::TitleWithExtra));
    }
}
