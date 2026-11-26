//! The media's chapters as the player shows them (S8.2): the List panel's Chapters tab, the seek
//! bar's marks and hover card, the video menu's Chapter ▸, and Shift+N and Shift+P.
//!
//! Containers can list chapters badly, so [`listed`] tidies what the engine reports: chapters
//! starting at or past the end are dropped, the rest are put in time order and numbered from 1,
//! and a chapter without a title is "Chapter N". Before the first chapter starts there is no
//! chapter, rather than a made-up "Intro".

use std::time::Duration;

use gpui::{App, Context};

use crate::engine::media_info::Chapter;
use crate::engine::values::SeekTarget;
use crate::main_window::MainWindow;
use crate::main_window::osd::messages;

/// How far into a chapter Previous Chapter goes back to its start rather than to the chapter
/// before it, as disc players do.
const RESTART_GRACE: Duration = Duration::from_secs(3);
/// How close ahead a chapter may start and still not count as the next one: a seek to a chapter's
/// start can land a little before it.
const NEXT_LEEWAY: Duration = Duration::from_millis(500);

/// A chapter as listed: its number, its title and where it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::main_window) struct Listed {
    /// Its place in the list, from 1.
    pub number: usize,
    /// Its title, or "Chapter N".
    pub title: String,
    /// Where it starts.
    pub start: Duration,
}

/// Which way a chapter jump goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::main_window) enum Jump {
    /// The previous chapter, or the start of this one.
    Previous,
    /// The next chapter.
    Next,
}

/// Where a chapter jump lands: a chapter's start, or the media's start before the first chapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Landing {
    Chapter(usize),
    MediaStart,
}

/// The chapters worth showing, in time order: those starting before `duration`, when it is known.
pub(in crate::main_window) fn listed(
    chapters: &[Chapter],
    duration: Option<Duration>,
) -> Vec<Listed> {
    let mut kept: Vec<&Chapter> = chapters
        .iter()
        .filter(|chapter| duration.is_none_or(|duration| chapter.start < duration))
        .collect();
    kept.sort_by_key(|chapter| chapter.start);
    kept.into_iter()
        .enumerate()
        .map(|(index, chapter)| Listed {
            number: index + 1,
            title: title(chapter, index + 1),
            start: chapter.start,
        })
        .collect()
}

/// The chapter's title, or "Chapter 3" when the container names none.
fn title(chapter: &Chapter, number: usize) -> String {
    match chapter.title.as_deref().map(str::trim) {
        Some(title) if !title.is_empty() => title.to_owned(),
        _ => tr!("Chapter {number}", number),
    }
}

/// The chapter playing at `position`: the last one started by then. `None` before the first.
pub(in crate::main_window) fn current(chapters: &[Listed], position: Duration) -> Option<usize> {
    chapters.iter().rposition(|chapter| chapter.start <= position)
}

/// Where a jump `way` from `position` lands, if anywhere.
fn landing(chapters: &[Listed], position: Duration, way: Jump) -> Option<Landing> {
    match way {
        Jump::Next => chapters
            .iter()
            .position(|chapter| chapter.start > position + NEXT_LEEWAY)
            .map(Landing::Chapter),
        Jump::Previous => {
            let index = current(chapters, position)?;
            let into = position.saturating_sub(chapters[index].start);
            if into > RESTART_GRACE {
                Some(Landing::Chapter(index))
            } else if index > 0 {
                Some(Landing::Chapter(index - 1))
            } else {
                (chapters[0].start > Duration::ZERO).then_some(Landing::MediaStart)
            }
        }
    }
}

impl MainWindow {
    /// The loaded media's chapters, as listed; empty when nothing is loaded.
    pub(in crate::main_window) fn chapters(&self, cx: &App) -> Vec<Listed> {
        let Some(player) = self.loaded_player(cx) else { return Vec::new() };
        let player = player.read(cx);
        listed(player.chapters(), player.duration())
    }

    /// Seeks to the start of `chapter` (a Chapters tab row, Chapter ▸).
    pub(in crate::main_window) fn seek_to_chapter(
        &mut self,
        chapter: &Listed,
        cx: &mut Context<Self>,
    ) {
        self.seek_player(SeekTarget::Absolute(chapter.start), cx);
        self.show_osd(messages::chapter(chapter.number, &chapter.title), cx);
    }

    /// Shift+N and Shift+P: goes to the next chapter, or back to the start of this one or the one
    /// before. Does nothing where there is nowhere to go.
    pub(in crate::main_window) fn jump_chapter(&mut self, way: Jump, cx: &mut Context<Self>) {
        let Some(player) = self.seekable_player(cx) else { return };
        let chapters = self.chapters(cx);
        match landing(&chapters, player.read(cx).position(), way) {
            Some(Landing::Chapter(index)) => self.seek_to_chapter(&chapters[index], cx),
            Some(Landing::MediaStart) => self.seek_to(super::Edge::Start, cx),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    fn chapter(title: Option<&str>, start: u64) -> Chapter {
        Chapter { title: title.map(str::to_owned), start: secs(start) }
    }

    fn starts(chapters: &[Listed]) -> Vec<u64> {
        chapters.iter().map(|chapter| chapter.start.as_secs()).collect()
    }

    #[test]
    fn untitled_chapters_are_numbered() {
        let chapters = listed(&[chapter(Some("Winter"), 0), chapter(Some("  "), 9)], None);
        assert_eq!(chapters[0].title, "Winter");
        assert_eq!(chapters[1].title, "Chapter 2");
        assert_eq!(chapters[1].number, 2);
    }

    #[test]
    fn chapters_past_the_end_are_dropped_and_the_rest_sorted() {
        let reported =
            [chapter(None, 135), chapter(None, 6250), chapter(None, 0), chapter(None, 1785)];
        let chapters = listed(&reported, Some(secs(1540)));
        assert_eq!(starts(&chapters), [0, 135]);
        assert_eq!(chapters[1].title, "Chapter 2");
        assert_eq!(starts(&listed(&reported, None)), [0, 135, 1785, 6250]);
    }

    #[test]
    fn the_current_chapter_is_the_last_one_started() {
        let chapters = listed(&[chapter(None, 5), chapter(None, 60), chapter(None, 120)], None);
        assert_eq!(current(&chapters, Duration::ZERO), None);
        assert_eq!(current(&chapters, secs(5)), Some(0));
        assert_eq!(current(&chapters, secs(119)), Some(1));
        assert_eq!(current(&chapters, secs(4000)), Some(2));
    }

    #[test]
    fn next_skips_a_chapter_just_seeked_to() {
        let chapters = listed(&[chapter(None, 0), chapter(None, 60), chapter(None, 120)], None);
        let next = |at| landing(&chapters, at, Jump::Next);
        assert_eq!(next(secs(10)), Some(Landing::Chapter(1)));
        assert_eq!(next(Duration::from_millis(59_800)), Some(Landing::Chapter(2)));
        assert_eq!(next(secs(130)), None);
    }

    #[test]
    fn previous_restarts_the_chapter_unless_it_just_began() {
        let chapters = listed(&[chapter(None, 30), chapter(None, 60)], None);
        let previous = |at| landing(&chapters, at, Jump::Previous);
        assert_eq!(previous(secs(90)), Some(Landing::Chapter(1)));
        assert_eq!(previous(secs(61)), Some(Landing::Chapter(0)));
        assert_eq!(previous(secs(31)), Some(Landing::MediaStart));
        assert_eq!(previous(secs(10)), None);
        let from_zero = listed(&[chapter(None, 0)], None);
        assert_eq!(landing(&from_zero, secs(1), Jump::Previous), None);
    }
}
