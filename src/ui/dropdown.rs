//! Dropdown / select (S0.4.3): `shared/components/dropdown-*`,
//! `settings/components/setting_preset-*`.
//!
//! Closed: a well-framed control face with the value and a chevron. Open: a list of
//! option rows under it (above when there's no room), the chosen one with an
//! accent ✓, scrolling when there are many.
//!
//! Keyboard: Enter, Space, Alt+Down or Down open it; Up/Down/Home/End/PageUp/
//! PageDown move the highlight; Enter or Space chooses; Escape or Tab closes.
//! Typing selects by prefix (closed) or highlights (open); repeating one letter
//! cycles through the items that start with it.

use std::time::{Duration, Instant};

use gpui::{
    App, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, KeyDownEvent, ScrollHandle,
    SharedString, Window, accesskit, div, prelude::*, px,
};

use super::focus::{self, Axis, Layout, Move};
use super::icon::icon;
use super::metrics;
use super::option_row::OptionRow;
use super::popover::{self, BoundsTracker, Side};
use crate::assets::icons;
use crate::theme;

/// Rows shown before the list scrolls.
const VISIBLE_ROWS: usize = 8;
const ROW_HEIGHT: f32 = 40.;

/// The value changed (by mouse, keyboard or typing).
pub struct Changed(pub usize);

pub struct Dropdown {
    options: Vec<SharedString>,
    selected: usize,
    label: SharedString,
    open: bool,
    highlighted: Option<usize>,
    focus_handle: FocusHandle,
    trigger: BoundsTracker,
    list: BoundsTracker,
    scroll: ScrollHandle,
    type_ahead: TypeAhead,
    disabled: bool,
}

impl EventEmitter<Changed> for Dropdown {}
impl EventEmitter<DismissEvent> for Dropdown {}

impl Focusable for Dropdown {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Dropdown {
    /// `label` names the setting for screen readers.
    pub fn new(
        label: impl Into<SharedString>,
        options: Vec<SharedString>,
        selected: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::observe(window, cx);
        popover::close_on_deactivate(window, cx, |this: &mut Self, _, cx| this.close(cx));
        let focus_handle = cx.focus_handle().tab_stop(true);
        // Tabbing away closes the list.
        cx.on_blur(&focus_handle, window, |this, _, cx| this.close(cx))
            .detach();
        Self {
            selected: selected.min(options.len().saturating_sub(1)),
            options,
            label: label.into(),
            open: false,
            highlighted: None,
            focus_handle,
            trigger: BoundsTracker::new(),
            list: BoundsTracker::new(),
            scroll: ScrollHandle::new(),
            type_ahead: TypeAhead::default(),
            disabled: false,
        }
    }

    #[allow(dead_code)] // Kit API for later stages, exercised by the tests.
    pub fn selected(&self) -> usize {
        self.selected
    }

    #[allow(dead_code)] // Kit API for later stages.
    pub fn set_selected(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.options.len() && index != self.selected {
            self.selected = index;
            cx.notify();
        }
    }

    #[allow(dead_code)] // Kit API for later stages.
    pub fn set_options(&mut self, options: Vec<SharedString>, cx: &mut Context<Self>) {
        self.selected = self.selected.min(options.len().saturating_sub(1));
        self.options = options;
        self.highlighted = None;
        cx.notify();
    }

    #[allow(dead_code)] // Kit API for later stages.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.focus_handle = self.focus_handle.clone().tab_stop(!disabled);
        if disabled {
            self.close(cx);
        }
        cx.notify();
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.options.len() {
            return;
        }
        let changed = index != self.selected;
        self.selected = index;
        self.close(cx);
        if changed {
            cx.emit(Changed(index));
        }
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(cx);
        } else {
            self.open(window, cx);
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.options.is_empty() {
            return;
        }
        self.focus_handle.focus(window, cx);
        self.open = true;
        self.highlighted = Some(self.selected);
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            self.highlighted = None;
            cx.emit(DismissEvent);
            cx.notify();
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let len = self.options.len();
        if keystroke.modifiers.alt && matches!(key, "down" | "up") {
            self.toggle(window, cx);
            cx.stop_propagation();
            return;
        }
        match (self.open, key) {
            (false, "enter" | "space" | "down") => self.open(window, cx),
            (true, "escape") => self.close(cx),
            (true, "enter" | "space") => {
                if let Some(index) = self.highlighted {
                    self.choose(index, cx);
                }
            }
            (true, "tab") => {
                // Close, and let Tab move focus on.
                self.close(cx);
                return;
            }
            _ => {
                if let Some(movement) = focus::group_key(event, Axis::Vertical) {
                    let layout = Layout {
                        page: VISIBLE_ROWS,
                        ..Layout::LIST
                    };
                    if self.open {
                        self.highlighted =
                            focus::step(self.highlighted, len, movement, layout, |_| true);
                        if let Some(index) = self.highlighted {
                            self.scroll.scroll_to_item(index);
                        }
                        cx.notify();
                    } else if movement != Move::Next {
                        // Up/Home/End while closed change the value, like a native
                        // select.
                        if let Some(index) =
                            focus::step(Some(self.selected), len, movement, layout, |_| true)
                            && index != self.selected
                        {
                            self.selected = index;
                            cx.emit(Changed(index));
                            cx.notify();
                        }
                    }
                } else if let Some(ch) = keystroke.key_char.as_deref()
                    && !ch.trim().is_empty()
                    && !keystroke.modifiers.control
                    && !keystroke.modifiers.platform
                {
                    self.type_to_select(ch, cx);
                } else {
                    return;
                }
            }
        }
        cx.stop_propagation();
    }

    fn type_to_select(&mut self, ch: &str, cx: &mut Context<Self>) {
        let query = self.type_ahead.push(ch, Instant::now());
        let labels: Vec<&str> = self.options.iter().map(|o| o.as_ref()).collect();
        let current = if self.open {
            self.highlighted
        } else {
            Some(self.selected)
        };
        let Some(index) = find_match(&labels, &query, current, |_| true) else {
            return;
        };
        if self.open {
            self.highlighted = Some(index);
            self.scroll.scroll_to_item(index);
        } else if index != self.selected {
            self.selected = index;
            cx.emit(Changed(index));
        }
        cx.notify();
    }
}

impl Render for Dropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let value = self.options.get(self.selected).cloned().unwrap_or_default();
        let hover = p.button_hover;
        let enabled = !self.disabled;

        let trigger = div()
            .id("dropdown")
            .relative()
            .h(px(metrics::FIELD_HEIGHT))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .pl(px(14.))
            .pr(px(12.))
            .rounded(px(metrics::RADIUS + 2.))
            .bg(p.control)
            .border_2()
            .border_color(if self.open { p.focus_ring } else { p.well })
            .text_size(px(metrics::TEXT))
            .text_color(p.text_muted)
            .track_focus(&self.focus_handle)
            .role(accesskit::Role::ComboBox)
            .aria_label(self.label.clone())
            .aria_expanded(self.open)
            .aria_value(value.clone())
            .on_key_down(cx.listener(Self::on_key_down))
            .child(self.trigger.element())
            .child(div().min_w_0().truncate().child(value))
            .child(icon(
                if self.open {
                    icons::CHEVRON_UP
                } else {
                    icons::CHEVRON_DOWN
                },
                20.,
                p.icon,
            ))
            .when(enabled, |t| {
                t.cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx)))
            })
            .when(!enabled, |t| t.opacity(0.4));
        let trigger = focus::ring(trigger, &p);

        let list = self
            .open
            .then(|| self.trigger.bounds())
            .flatten()
            .map(|bounds| {
                let rows = self.options.iter().enumerate().map(|(i, option)| {
                    OptionRow::new(i, option.clone())
                        .height(ROW_HEIGHT)
                        .checked(i == self.selected)
                        .highlighted(self.highlighted == Some(i))
                        .into_any_element()
                });
                let rows = rows.enumerate().map(|(i, row)| {
                    div()
                        .id(("option", i))
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered && this.highlighted != Some(i) {
                                this.highlighted = Some(i);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| this.choose(i, cx)))
                        .child(row)
                });
                let list_height = (ROW_HEIGHT * VISIBLE_ROWS as f32 + 12.)
                    .min(f32::from(popover::max_height(window)));
                let tracker = self.list.clone();
                popover::attached(
                    bounds,
                    Side::Below,
                    4.,
                    1,
                    popover::surface(&p, window)
                        .id("dropdown-list")
                        .relative()
                        .w(bounds.size.width)
                        .max_h(px(list_height))
                        .p(px(6.))
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .role(accesskit::Role::ListBox)
                        .on_mouse_down_out(cx.listener(
                            |this, event: &gpui::MouseDownEvent, _, cx| {
                                // The trigger toggles on its own click.
                                if !this.trigger.contains(&event.position) {
                                    this.close(cx);
                                }
                            },
                        ))
                        .child(tracker.element())
                        .children(rows),
                )
            });

        div().w_full().child(trigger).children(list)
    }
}

/// Type-to-select: letters typed in quick succession form a prefix query.
#[derive(Default)]
pub struct TypeAhead {
    query: String,
    last: Option<Instant>,
}

impl TypeAhead {
    /// Pause after which typing starts a new query.
    pub const RESET_AFTER: Duration = Duration::from_millis(1000);

    /// Adds typed text and returns the query to search for.
    pub fn push(&mut self, text: &str, now: Instant) -> String {
        if self
            .last
            .is_none_or(|last| now.duration_since(last) > Self::RESET_AFTER)
        {
            self.query.clear();
        }
        self.last = Some(now);
        self.query.push_str(&text.to_lowercase());
        self.query.clone()
    }
}

/// The item to jump to for `query`, searching after `current`.
///
/// A query of one repeated letter ("a", "aa", "aaa") cycles through the items
/// starting with that letter; anything else finds the first item starting with
/// the query, from `current` on, wrapping. Matching ignores case.
pub fn find_match(
    labels: &[&str],
    query: &str,
    current: Option<usize>,
    enabled: impl Fn(usize) -> bool,
) -> Option<usize> {
    let len = labels.len();
    if len == 0 || query.is_empty() {
        return None;
    }
    let mut chars = query.chars();
    let first = chars.next()?;
    let repeated = chars.all(|c| c == first);
    let (needle, start) = if repeated {
        // Cycle: start after the current item.
        (first.to_string(), current.map_or(0, |c| c + 1))
    } else {
        // Refine: the current item still counts if it matches the longer prefix.
        (query.to_owned(), current.unwrap_or(0))
    };
    (0..len)
        .map(|offset| (start + offset) % len)
        .find(|&i| enabled(i) && labels[i].to_lowercase().starts_with(&needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABELS: &[&str] = &["Auto", "Bass", "Bright", "Classical", "Club", "Dance"];

    fn all(_: usize) -> bool {
        true
    }

    #[test]
    fn prefix_match_ignores_case() {
        assert_eq!(find_match(LABELS, "cla", None, all), Some(3));
        assert_eq!(find_match(LABELS, "clu", Some(3), all), Some(4));
        assert_eq!(find_match(LABELS, "x", None, all), None);
    }

    #[test]
    fn repeating_a_letter_cycles() {
        assert_eq!(find_match(LABELS, "b", None, all), Some(1));
        assert_eq!(find_match(LABELS, "bb", Some(1), all), Some(2));
        // Wraps back to the first "B" item.
        assert_eq!(find_match(LABELS, "bbb", Some(2), all), Some(1));
    }

    #[test]
    fn refining_keeps_the_current_item() {
        // "Bright" is current and still matches "br".
        assert_eq!(find_match(LABELS, "br", Some(2), all), Some(2));
    }

    #[test]
    fn skips_disabled_items() {
        assert_eq!(find_match(LABELS, "b", None, |i| i != 1), Some(2));
    }

    #[test]
    fn type_ahead_resets_after_a_pause() {
        let mut ahead = TypeAhead::default();
        let t0 = Instant::now();
        assert_eq!(ahead.push("C", t0), "c");
        assert_eq!(ahead.push("l", t0 + Duration::from_millis(300)), "cl");
        let later = t0 + Duration::from_millis(300) + TypeAhead::RESET_AFTER * 2;
        assert_eq!(ahead.push("d", later), "d");
    }
}
