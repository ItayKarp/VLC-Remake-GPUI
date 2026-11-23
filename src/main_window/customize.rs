//! Customize Player (S8.7): the main menu's command puts the bars in edit mode, every movable
//! piece outlined with a dashed border (the logo stays), and opens the palette over the video.
//! Pieces are dragged within and between the title bar and the control bar's rows, onto the
//! place for a second row, out of the palette to add them and into it to remove them.
//!
//! The bars show the draft while editing; Save stores it (enabled once it differs from what's
//! saved), Cancel, ✕ and Escape drop it, and Reset to default puts the designed layout back in
//! the draft. Saving without Play/Pause or the seek bar is allowed, with a reminder that the
//! keys still work.
//!
//! While editing the bars aren't fitted to the window: every piece shows, so each can be picked
//! up. Pieces' outlines are `outline.rs`, the palette `palette.rs`, their names and icons
//! `units.rs`. Without a pointer, the palette's list (`list.rs`) moves, removes and adds units
//! with buttons.

mod list;
mod outline;
mod palette;
#[cfg(test)]
mod tests;
mod units;

use std::collections::HashMap;

use gpui::{Bounds, Context, FocusHandle, Pixels, Point, Window};

pub(in crate::main_window) use self::outline::{DraggedUnit, placeholder};
pub(in crate::main_window) use self::units::label as unit_label;
use super::MainWindow;
use crate::player_layout::{Item, Layout, Spot, Target, Zone};
use crate::settings;
use crate::ui::toast::{Level, Toast};

/// A Customize Player session: the layout being edited and the drag over the bars.
pub(super) struct Customizing {
    /// The layout as edited so far.
    draft: Layout,
    /// The layout saved when the session started, to tell whether there's anything to save.
    saved: Layout,
    /// Where the unit being dragged would land.
    target: Option<Target>,
    /// Where each outlined piece was last drawn, by its spot, to place drops between them.
    bounds: HashMap<(Zone, usize), Bounds<Pixels>>,
    /// The palette's focus, for Escape.
    focus: FocusHandle,
    /// Whether the palette shows the layout as a list, for the keyboard.
    as_list: bool,
}

impl MainWindow {
    /// Starts Customize Player: the bars show (Hide UI ends) and the palette opens.
    pub(super) fn customize_player(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.customizing.is_some() {
            return;
        }
        self.menu = None;
        self.control_bar.close_overflow();
        self.close_popovers(window, cx);
        self.set_hide_ui(false, cx);
        let saved = self.control_bar.layout().clone();
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.customizing = Some(Customizing {
            draft: saved.clone(),
            saved,
            target: None,
            bounds: HashMap::new(),
            focus,
            as_list: false,
        });
        cx.notify();
    }

    /// Whether Customize Player is open.
    pub(super) fn is_customizing(&self) -> bool {
        self.customizing.is_some()
    }

    /// The layout the bars show: the draft while customizing.
    pub(super) fn shown_layout(&self) -> &Layout {
        self.customizing.as_ref().map_or_else(|| self.control_bar.layout(), |c| &c.draft)
    }

    /// Ends Customize Player without saving.
    fn cancel_customizing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.customizing.take().is_some() {
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    /// Stores the draft and ends Customize Player.
    fn save_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(customizing) = self.customizing.take() else { return };
        let stored = customizing.draft.to_stored();
        settings::update(cx, |s| s.prefs.player_layout = stored);
        self.control_bar.sync_layout(cx);
        for reminder in reminders(&customizing.draft) {
            self.show_toast(Toast::new(Level::Info, reminder), cx);
        }
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Switches the palette between its units and the layout as a list.
    fn toggle_list(&mut self, cx: &mut Context<Self>) {
        if let Some(customizing) = &mut self.customizing {
            customizing.as_list = !customizing.as_list;
            cx.notify();
        }
    }

    /// Puts the designed layout in the draft.
    fn reset_layout(&mut self, cx: &mut Context<Self>) {
        if let Some(customizing) = &mut self.customizing {
            customizing.draft = Layout::designed();
            customizing.bounds.clear();
            cx.notify();
        }
    }

    /// Notes where the outlined piece at `spot` is drawn.
    fn track_piece(&mut self, spot: Spot, bounds: Bounds<Pixels>) {
        if let Some(customizing) = &mut self.customizing {
            customizing.bounds.insert((spot.zone, spot.index), bounds);
        }
    }

    /// Follows a drag over the bar `zone`, drawn in `bounds`: the gap under the pointer while
    /// it's over the bar.
    fn track_zone(
        &mut self,
        zone: Zone,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(customizing) = &mut self.customizing else { return };
        let over = bounds.contains(&position);
        let target = if over {
            Some(Target::Into(Spot { zone, index: customizing.drop_index(zone, position.x) }))
        } else if matches!(customizing.target, Some(Target::Into(spot)) if spot.zone == zone) {
            None
        } else {
            return;
        };
        customizing.set_target(target, cx);
    }

    /// Follows a drag over the place for a new row, or the palette (`target` `None`).
    fn track_drop_area(&mut self, target: Option<Target>, over: bool, cx: &mut Context<Self>) {
        let Some(customizing) = &mut self.customizing else { return };
        if over {
            customizing.set_target(target, cx);
        } else if target.is_some() && customizing.target == target {
            customizing.set_target(None, cx);
        }
    }

    /// Drops `dragged` where the drag points: into a bar, or a new row.
    fn drop_unit(&mut self, dragged: &DraggedUnit, cx: &mut Context<Self>) {
        let Some(customizing) = &mut self.customizing else { return };
        let Some(target) = customizing.target.take() else { return };
        match dragged.from {
            Some(from) => customizing.draft.move_unit(from, target),
            None => customizing.draft.add(dragged.unit, target),
        }
        customizing.bounds.clear();
        cx.notify();
    }

    /// Drops `dragged` on the palette: a piece from the bars leaves them.
    fn remove_unit(&mut self, dragged: &DraggedUnit, cx: &mut Context<Self>) {
        let Some(customizing) = &mut self.customizing else { return };
        if let Some(from) = dragged.from {
            customizing.draft.remove(from);
            customizing.bounds.clear();
        }
        customizing.target = None;
        cx.notify();
    }
}

impl Customizing {
    /// The index in `zone` a drop at `x` goes to: before the first piece whose middle is right
    /// of it, else the end.
    fn drop_index(&self, zone: Zone, x: Pixels) -> usize {
        let mut drawn: Vec<(usize, Bounds<Pixels>)> = self
            .bounds
            .iter()
            .filter(|((piece_zone, _), _)| *piece_zone == zone)
            .map(|((_, index), bounds)| (*index, *bounds))
            .collect();
        drawn.sort_by_key(|(index, _)| *index);
        let after = drawn.into_iter().find(|(_, bounds)| bounds.center().x > x);
        after.map_or_else(|| self.draft.pieces(zone).len(), |(index, _)| index)
    }

    fn set_target(&mut self, target: Option<Target>, cx: &mut Context<MainWindow>) {
        if self.target != target {
            self.target = target;
            cx.notify();
        }
    }

    /// Whether Save would change anything.
    fn has_changes(&self) -> bool {
        self.draft != self.saved
    }
}

/// What to remind of after saving `layout`: the keys that still do what removed controls did.
fn reminders(layout: &Layout) -> Vec<&'static str> {
    let mut reminders = Vec::new();
    if !layout.is_placed(Item::PlayPause) {
        reminders.push("Play/Pause isn't in the player: Space still plays and pauses");
    }
    if !layout.is_placed(Item::SeekBar) {
        reminders.push("The seek bar isn't in the player: ← and → still seek");
    }
    reminders
}
