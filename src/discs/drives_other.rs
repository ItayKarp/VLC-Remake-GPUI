//! Discs in Windows drives: every drive letter holding a disc's folders, or an audio CD's
//! `Track01.cda` files, which Windows shows for each track.

use std::path::PathBuf;

use super::{Disc, DiscKind, kind_in};

/// The drive letters looked at; A and B are floppy drives.
const LETTERS: std::ops::RangeInclusive<u8> = b'C'..=b'Z';
/// The file Windows shows for an audio CD's first track.
const FIRST_TRACK: &str = "Track01.cda";

/// The drives holding a disc.
pub fn discs() -> Vec<Disc> {
    LETTERS.filter_map(|letter| disc(char::from(letter))).collect()
}

/// The disc in drive `letter`, if there is one.
fn disc(letter: char) -> Option<Disc> {
    let root = PathBuf::from(format!("{letter}:\\"));
    let kind =
        kind_in(&root).or_else(|| root.join(FIRST_TRACK).is_file().then_some(DiscKind::AudioCd))?;
    Some(Disc {
        name: format!("{} ({letter}:)", kind.name()),
        kind: Some(kind),
        device: Some(root.clone()),
        mount: Some(root),
    })
}
