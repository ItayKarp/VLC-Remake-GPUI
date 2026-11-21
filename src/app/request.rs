//! What one launch asks for (S0.6.2): which screen to start on and what to open.
//! A launch is either handled here or forwarded to the running instance (S0.6.1),
//! so it's plain data with no GPUI types.

use super::media::Media;

/// Which window a launch asks for, overriding the normal routing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StartScreen {
    /// Onboarding on first run, otherwise the main window.
    #[default]
    Auto,
    /// Onboarding even though it was completed (`--reset-onboarding`).
    Onboarding,
    /// The component gallery (`--component-gallery`).
    Gallery,
}

/// Whether opened items replace what's open or are added after it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// Replace the queue (the default, like double-clicking a file).
    #[default]
    Replace,
    /// Add to the end of the queue (`--enqueue`, Add to Queue, Shift+drop).
    Enqueue,
    /// Insert right after the current item (Play Next, S3.1.2).
    PlayNext,
}

/// Items to open and how.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenRequest {
    /// Files and URLs, in the order given.
    pub items: Vec<Media>,
    /// Replace the queue or add to it.
    pub placement: Placement,
    /// Switch the main window to fullscreen (`--fullscreen`).
    pub fullscreen: bool,
}

impl OpenRequest {
    /// Opens `items`, replacing what's open.
    pub fn replace(items: Vec<Media>) -> Self {
        OpenRequest { items, ..Self::default() }
    }

    /// Opens `items` placed as `placement` says.
    pub fn placed(items: Vec<Media>, placement: Placement) -> Self {
        OpenRequest { items, placement, ..Self::default() }
    }

    /// Whether this request opens nothing and changes nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && !self.fullscreen
    }

    /// The request that has the effect of `self` followed by `next`, for requests that
    /// wait (while onboarding is showing). A replacing `next` drops what came before.
    #[must_use]
    pub fn then(mut self, next: OpenRequest) -> OpenRequest {
        let fullscreen = self.fullscreen || next.fullscreen;
        match next.placement {
            Placement::Replace if !next.items.is_empty() => OpenRequest { fullscreen, ..next },
            _ => {
                self.items.extend(next.items);
                self.fullscreen = fullscreen;
                self
            }
        }
    }
}

/// Everything one launch of the app asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    /// The screen to start on.
    pub start: StartScreen,
    /// What to open.
    pub open: OpenRequest,
}

impl Launch {
    /// The launch for a log message: counts instead of paths, which are personal data.
    pub fn summary(&self) -> String {
        let open = &self.open;
        format!(
            "{:?} screen, {} item(s) to {:?}, fullscreen: {}",
            self.start,
            open.items.len(),
            open.placement,
            open.fullscreen
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn file(name: &str) -> Media {
        Media::File(PathBuf::from(name))
    }

    fn enqueue(items: Vec<Media>) -> OpenRequest {
        OpenRequest { items, placement: Placement::Enqueue, fullscreen: false }
    }

    #[test]
    fn replace_drops_what_came_before() {
        let first = OpenRequest::replace(vec![file("a")]);
        let merged = first.then(OpenRequest::replace(vec![file("b")]));
        assert_eq!(merged, OpenRequest::replace(vec![file("b")]));
    }

    #[test]
    fn enqueue_appends_and_keeps_the_first_placement() {
        let first = OpenRequest::replace(vec![file("a")]);
        let merged = first.then(enqueue(vec![file("b")]));
        assert_eq!(merged, OpenRequest::replace(vec![file("a"), file("b")]));
    }

    #[test]
    fn an_empty_request_changes_nothing_but_fullscreen() {
        let first = OpenRequest::replace(vec![file("a")]);
        let fullscreen = OpenRequest { fullscreen: true, ..OpenRequest::default() };
        assert!(!fullscreen.is_empty());
        assert!(OpenRequest::default().is_empty());
        let merged = first.then(fullscreen);
        assert_eq!(merged.items, [file("a")]);
        assert!(merged.fullscreen);
    }
}
