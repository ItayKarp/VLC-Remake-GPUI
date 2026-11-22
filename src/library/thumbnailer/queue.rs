//! The thumbnailer's work order (S4.4.4): what is on screen first, in the order the view shows
//! it, then the background backlog. A new list of what is on screen replaces the old one, so
//! items that scrolled away are dropped before they start. The backlog waits while something
//! plays, so playback never stutters.

use std::collections::VecDeque;

use crate::db::media::MediaId;

/// Why a media is being made a thumbnail for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// It is on screen.
    Visible,
    /// Filling the cache ahead of time.
    Background,
}

/// One media to make a thumbnail for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Job {
    /// The media.
    pub media: MediaId,
    /// Why.
    pub priority: Priority,
}

/// What to work on next.
#[derive(Debug, Default)]
pub struct Queue {
    visible: VecDeque<MediaId>,
    /// Everything on screen, including what was already taken: a running job for media that
    /// left it may stop.
    on_screen: Vec<MediaId>,
    backlog: VecDeque<MediaId>,
    playing: bool,
}

impl Queue {
    /// What the view shows now, in order, replacing what it showed before.
    pub fn show(&mut self, media: Vec<MediaId>) {
        self.visible = media.iter().copied().collect();
        self.on_screen = media;
    }

    /// Whether `media` is on screen.
    pub fn is_on_screen(&self, media: MediaId) -> bool {
        self.on_screen.contains(&media)
    }

    /// Adds `media` to the backlog.
    pub fn fill(&mut self, media: Vec<MediaId>) {
        self.backlog.extend(media);
    }

    /// Whether something plays, which holds the backlog back.
    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    /// Whether the backlog may run now.
    pub fn may_fill(&self) -> bool {
        !self.playing
    }

    /// The next job: what is on screen first, then the backlog unless something plays.
    pub fn next(&mut self) -> Option<Job> {
        if let Some(media) = self.visible.pop_front() {
            return Some(Job { media, priority: Priority::Visible });
        }
        if self.playing {
            return None;
        }
        let media = self.backlog.pop_front()?;
        Some(Job { media, priority: Priority::Background })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(ids: &[i64]) -> Vec<MediaId> {
        ids.iter().copied().map(MediaId).collect()
    }

    fn job(media: i64, priority: Priority) -> Job {
        Job { media: MediaId(media), priority }
    }

    #[test]
    fn what_is_on_screen_comes_first_in_order() {
        let mut queue = Queue::default();
        queue.fill(ids(&[9]));
        queue.show(ids(&[2, 1]));
        assert_eq!(queue.next(), Some(job(2, Priority::Visible)));
        assert_eq!(queue.next(), Some(job(1, Priority::Visible)));
        assert_eq!(queue.next(), Some(job(9, Priority::Background)));
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn scrolling_away_drops_what_has_not_started() {
        let mut queue = Queue::default();
        queue.show(ids(&[1, 2, 3]));
        assert_eq!(queue.next(), Some(job(1, Priority::Visible)));
        queue.show(ids(&[4]));
        assert!(!queue.is_on_screen(MediaId(1)) && queue.is_on_screen(MediaId(4)));
        assert_eq!(queue.next(), Some(job(4, Priority::Visible)));
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn the_backlog_waits_while_something_plays() {
        let mut queue = Queue::default();
        queue.fill(ids(&[9]));
        queue.set_playing(true);
        assert!(!queue.may_fill());
        queue.show(ids(&[1]));
        assert_eq!(queue.next(), Some(job(1, Priority::Visible)));
        assert_eq!(queue.next(), None);
        queue.set_playing(false);
        assert_eq!(queue.next(), Some(job(9, Priority::Background)));
    }
}
