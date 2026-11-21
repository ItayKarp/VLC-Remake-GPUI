//! Something the window shows for a moment after a change and then hides: the volume bubble
//! (S2.5.3) and the OSD message (S2.5.7). Each change shows it again and restarts its timer;
//! only the newest timer hides it.

use std::time::Duration;

use gpui::Context;

use super::MainWindow;

/// A moment of showing something.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flash {
    /// The ticket of the showing in progress.
    showing: Option<u64>,
    /// The ticket the next showing gets.
    next: u64,
}

impl Flash {
    /// Whether it shows now.
    pub fn is_showing(self) -> bool {
        self.showing.is_some()
    }

    /// Shows it (again), returning the ticket that ends this showing.
    fn show(&mut self) -> u64 {
        let ticket = self.next;
        self.next += 1;
        self.showing = Some(ticket);
        ticket
    }

    /// Hides it if `ticket` is still the newest showing; returns whether it did.
    fn end(&mut self, ticket: u64) -> bool {
        let newest = self.showing == Some(ticket);
        if newest {
            self.showing = None;
        }
        newest
    }
}

impl MainWindow {
    /// Shows the flash `field` picks out for `duration` from now.
    pub(super) fn flash(
        &mut self,
        field: fn(&mut Self) -> &mut Flash,
        duration: Duration,
        cx: &mut Context<Self>,
    ) {
        let ticket = field(self).show();
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            // The window may be gone by then; nothing is left to hide.
            let _ = this.update(cx, |this, cx| this.end_flash(field, ticket, cx));
        })
        .detach();
    }

    /// Hides the flash `field` picks out, unless a newer showing than `ticket` keeps it up.
    fn end_flash(
        &mut self,
        field: fn(&mut Self) -> &mut Flash,
        ticket: u64,
        cx: &mut Context<Self>,
    ) {
        if field(self).end(ticket) {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_newest_showing_hides_it() {
        let mut flash = Flash::default();
        let first = flash.show();
        let second = flash.show();
        assert!(!flash.end(first), "a newer change keeps it up");
        assert!(flash.is_showing());
        assert!(flash.end(second));
        assert!(!flash.is_showing());
    }
}
