//! Screen-reader announcements (S9.1.2).
//!
//! GPUI builds an AccessKit tree from elements with an id and a role, but has no call to
//! speak a message. A live region does it: when a node marked live appears or its text
//! changes, the platform adapter speaks it (macOS posts an announcement from the node's
//! value, Windows raises UI Automation's LiveRegionChanged with its name, Linux AT-SPI
//! follows `live`). [`live_region`] marks any element that way; toasts and banners use it
//! so errors are spoken. [`Announcer`] is an invisible region a view keeps for news that
//! has no element of its own, such as the main window's play state and track changes.

use gpui::{
    Div, ElementId, SharedString, Stateful, StatefulInteractiveElement, accesskit, div, prelude::*,
    px,
};

/// How urgently a screen reader speaks an announcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Politeness {
    /// After whatever it is saying now: news.
    Polite,
    /// At once, interrupting: errors.
    Assertive,
}

impl Politeness {
    fn live(self) -> accesskit::Live {
        match self {
            Politeness::Polite => accesskit::Live::Polite,
            Politeness::Assertive => accesskit::Live::Assertive,
        }
    }

    /// The role that goes with it: an alert for errors, a status for news.
    pub fn role(self) -> accesskit::Role {
        match self {
            Politeness::Polite => accesskit::Role::Status,
            Politeness::Assertive => accesskit::Role::Alert,
        }
    }
}

/// Makes `element` a live region that speaks `text` when it appears or `text` changes.
/// The element needs an id; this gives it the politeness's role, and `text` as both its
/// name and value, since adapters read one or the other.
pub fn live_region<E: StatefulInteractiveElement>(
    element: E,
    text: impl Into<SharedString>,
    politeness: Politeness,
) -> E {
    let text = text.into();
    element
        .role(politeness.role())
        .aria_label(text.clone())
        .aria_value(text)
        .a11y_synthetic_children(move |tree| tree.parent_node().set_live(politeness.live()))
}

/// A view's latest announcement, rendered as an invisible live region.
#[derive(Debug, Default)]
pub struct Announcer {
    said: Option<(SharedString, Politeness)>,
}

impl Announcer {
    /// Has `text` spoken, unless it is what was said last: a state reported twice (playing
    /// again after buffering) isn't repeated. Returns whether it will be spoken.
    pub fn announce(&mut self, text: impl Into<SharedString>, politeness: Politeness) -> bool {
        let text = text.into();
        if self.said.as_ref().is_some_and(|(said, _)| *said == text) {
            return false;
        }
        self.said = Some((text, politeness));
        true
    }

    /// The live region, a 1 px element that draws nothing; `None` before the first
    /// announcement. Render it in the same place every frame, so its node stays the same
    /// and only its text changes.
    pub fn render(&self, id: impl Into<ElementId>) -> Option<Stateful<Div>> {
        let (text, politeness) = self.said.clone()?;
        let region = div().id(id).absolute().top_0().left_0().size(px(1.)).overflow_hidden();
        Some(live_region(region, text, politeness))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_are_not_spoken_again() {
        let mut announcer = Announcer::default();
        assert!(announcer.announce("Playing a.mkv", Politeness::Polite));
        assert!(!announcer.announce("Playing a.mkv", Politeness::Polite));
        assert!(announcer.announce("Paused: a.mkv", Politeness::Polite));
        assert!(announcer.announce("Playing a.mkv", Politeness::Polite));
        assert!(announcer.render("region").is_some());
    }

    #[test]
    fn errors_are_alerts_and_news_is_status() {
        assert_eq!(Politeness::Assertive.role(), accesskit::Role::Alert);
        assert_eq!(Politeness::Polite.role(), accesskit::Role::Status);
        assert!(Announcer::default().render("region").is_none());
    }
}
