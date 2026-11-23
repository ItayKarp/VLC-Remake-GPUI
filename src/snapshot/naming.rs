//! What a snapshot's file is called: `<title>_<hh-mm-ss.mmm>.<ext>`, the title made safe for
//! every OS's file names, and a number added when the name is taken.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Characters Windows refuses in file names (`/` is also the separator everywhere else).
const ILLEGAL: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
/// What replaces each illegal character.
const REPLACEMENT: char = '_';
/// What a snapshot of media without a usable title is named after.
const UNTITLED: &str = "Snapshot";
/// The most characters kept of the title: file names are limited to 255 bytes, and the time,
/// a number and the extension need room too.
const MAX_TITLE_CHARS: usize = 120;
/// Names Windows keeps for devices, with or without an extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The file name for a snapshot of `title` at `position`, with `extension`.
pub fn file_name(title: &str, position: Duration, extension: &str) -> String {
    format!("{}_{}.{extension}", safe_title(title), time(position))
}

/// `name` in `folder` for the first `attempt` (0), then with " (2)", " (3)", … before its
/// extension, for when the names before are taken.
pub fn numbered(folder: &Path, name: &str, attempt: u32) -> PathBuf {
    if attempt == 0 {
        return folder.join(name);
    }
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    folder.join(format!("{stem} ({}).{extension}", attempt + 1))
}

/// `position` as `hh-mm-ss.mmm`: the clock's colons aren't allowed in Windows file names.
fn time(position: Duration) -> String {
    let seconds = position.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    format!("{hours:02}-{minutes:02}-{seconds:02}.{:03}", position.subsec_millis())
}

/// `title` with the characters file names can't hold replaced, cut short, without the spaces
/// and dots Windows drops from a name's end, and never a device name Windows reserves.
fn safe_title(title: &str) -> String {
    let replaced: String = title
        .chars()
        .map(|c| if ILLEGAL.contains(&c) || c.is_control() { REPLACEMENT } else { c })
        .take(MAX_TITLE_CHARS)
        .collect();
    let trimmed = replaced.trim().trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        return UNTITLED.to_owned();
    }
    // Windows reads the part before the first dot: "NUL.mkv" is the device too.
    let device = trimmed.split('.').next().unwrap_or(trimmed).trim_end();
    if RESERVED.iter().any(|reserved| reserved.eq_ignore_ascii_case(device)) {
        return format!("{REPLACEMENT}{trimmed}");
    }
    trimmed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_carry_the_title_and_the_time() {
        let position = Duration::from_millis(3_723_045);
        assert_eq!(file_name("Big Buck Bunny", position, "png"), "Big Buck Bunny_01-02-03.045.png");
    }

    #[test]
    fn illegal_characters_are_replaced() {
        let name = file_name(r#"a:b?c*d/e\f|g"h<i>j"#, Duration::ZERO, "jpg");
        assert_eq!(name, "a_b_c_d_e_f_g_h_i_j_00-00-00.000.jpg");
    }

    #[test]
    fn reserved_windows_names_are_avoided() {
        assert_eq!(safe_title("CON"), "_CON");
        assert_eq!(safe_title("nul.part"), "_nul.part");
        assert_eq!(safe_title("Console"), "Console");
    }

    #[test]
    fn blank_and_dotted_titles_still_name_something() {
        assert_eq!(safe_title("  "), UNTITLED);
        assert_eq!(safe_title("..."), UNTITLED);
        assert_eq!(safe_title("Movie. "), "Movie");
    }

    #[test]
    fn long_titles_are_cut() {
        assert_eq!(safe_title(&"é".repeat(500)).chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn taken_names_get_a_number() {
        let folder = Path::new("/pics");
        assert_eq!(numbered(folder, "a_00.png", 0), Path::new("/pics/a_00.png"));
        assert_eq!(numbered(folder, "a_00.png", 1), Path::new("/pics/a_00 (2).png"));
    }
}
