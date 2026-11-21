//! Toasts and inline banners (S0.4.12).
//!
//! - [`Toasts`]: a per-window stack at the bottom right for transient news ("Saved
//!   snapshot", "Subtitles downloaded"). Info and success toasts go away on their
//!   own; errors stay until dismissed. At most [`MAX_VISIBLE`] show; older ones
//!   drop off.
//! - [`banner`]: an inline message inside a screen, for problems tied to it
//!   ("Settings couldn't be saved"), with an optional action.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, Context, Div, ElementId, Hsla, SharedString, Stateful, Window, accesskit, div, prelude::*,
    px,
};

use super::button::{Button, ButtonStyle, IconButton};
use super::icon::two_tone;
use super::metrics;
use crate::assets::{icons, two_tone as icons2};
use crate::theme::{self, Palette};

/// The most toasts shown at once; older ones drop off.
pub const MAX_VISIBLE: usize = 4;
/// How long info and success toasts stay.
const AUTO_DISMISS: Duration = Duration::from_secs(6);
/// A toast card's width, when the window has room for it.
const TOAST_WIDTH: f32 = 360.;
/// The size of the level icon in toasts and banners.
const ICON_SIZE: f32 = 18.;

/// How important a message is, which sets its color and whether it goes away alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// News that needs no action.
    Info,
    /// Something finished as asked.
    Success,
    /// Something the user may want to act on.
    Warning,
    /// Something failed.
    Error,
}

impl Level {
    /// The accent color for this level's icon and border.
    fn color(self, p: &Palette) -> Hsla {
        match self {
            Level::Info => p.accent_fg,
            Level::Success => p.success,
            Level::Warning => p.warning,
            Level::Error => p.danger,
        }
    }

    /// Errors and warnings need reading and maybe acting on, so they stay.
    pub fn auto_dismiss(self) -> bool {
        matches!(self, Level::Info | Level::Success)
    }
}

/// Runs when the user presses a toast's or banner's button.
type Action = Rc<dyn Fn(&mut Window, &mut App)>;

/// A transient message for a [`Toasts`] stack.
pub struct Toast {
    id: u64,
    level: Level,
    message: SharedString,
    action: Option<(SharedString, Action)>,
}

impl Toast {
    /// A toast showing `message` at `level`.
    pub fn new(level: Level, message: impl Into<SharedString>) -> Self {
        Self { id: 0, level, message: message.into(), action: None }
    }

    /// Adds a button labelled `label` that dismisses the toast and runs `f`.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(f)));
        self
    }
}

/// A window's stack of toasts, drawn at its bottom right.
#[derive(Default)]
pub struct Toasts {
    toasts: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    /// An empty stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows `toast`, dropping old ones past [`MAX_VISIBLE`], and schedules its
    /// dismissal if its level goes away alone.
    pub fn push(&mut self, mut toast: Toast, cx: &mut Context<Self>) {
        self.next_id += 1;
        toast.id = self.next_id;
        let id = toast.id;
        let auto = toast.level.auto_dismiss();
        self.toasts.push(toast);
        trim(&mut self.toasts, MAX_VISIBLE);
        if auto {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(AUTO_DISMISS).await;
                this.update(cx, |this, cx| this.dismiss(id, cx)).ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// Removes the toast with `id`, if it's still showing.
    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }
}

/// Drops the oldest auto-dismissing toasts first, then the oldest of any kind.
fn trim(toasts: &mut Vec<Toast>, max: usize) {
    while toasts.len() > max {
        let index = toasts.iter().position(|t| t.level.auto_dismiss()).unwrap_or(0);
        toasts.remove(index);
    }
}

impl Toasts {
    /// One toast card: icon, message, the optional action and a dismiss button.
    fn render_toast(toast: &Toast, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = toast.id;
        let action = toast.action.clone().map(|(label, action)| {
            Button::new(("toast-action", id as usize), label).style(ButtonStyle::Ghost).on_click(
                cx.listener(move |this, _, window, cx| {
                    this.dismiss(id, cx);
                    action(window, cx);
                }),
            )
        });
        div()
            .id(("toast", id as usize))
            .occlude()
            .w(px(TOAST_WIDTH))
            .max_w_full()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(10.))
            .pl(px(12.))
            .pr(px(4.))
            .py(px(8.))
            .rounded(px(metrics::RADIUS_LARGE))
            .bg(p.menu)
            .border_1()
            .border_color(p.divider)
            .shadow_lg()
            .role(accesskit::Role::Alert)
            .child(div().pt(px(4.)).child(two_tone(
                icons2::INFO,
                ICON_SIZE,
                p.icon,
                toast.level.color(p),
            )))
            .child(message(toast.message.clone(), p.text_strong).pt(px(4.)))
            .children(action)
            .child(
                IconButton::new(("toast-close", id as usize), icons::CLOSE, "Dismiss")
                    .on_click(cx.listener(move |this, _, _, cx| this.dismiss(id, cx))),
            )
    }
}

impl Render for Toasts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let toasts = self.toasts.iter().map(|toast| Self::render_toast(toast, &p, cx));

        div()
            .absolute()
            .right(px(16.))
            .bottom(px(16.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.))
            .children(toasts)
    }
}

/// An inline message, e.g. above a form or at the top of a screen.
#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    level: Level,
    message: SharedString,
    action: Option<(SharedString, Action)>,
    on_dismiss: Option<Action>,
}

/// A banner showing `message` at `level`.
pub fn banner(id: impl Into<ElementId>, level: Level, message: impl Into<SharedString>) -> Banner {
    Banner { id: id.into(), level, message: message.into(), action: None, on_dismiss: None }
}

impl Banner {
    /// Adds a button labelled `label` that runs `f`.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(f)));
        self
    }

    /// Adds a dismiss button that runs `f`; the owner hides the banner.
    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let color = self.level.color(&p);
        let action = self.action.map(|(label, f)| {
            Button::new("banner-action", label)
                .style(ButtonStyle::Filled)
                .on_click(move |_, window, cx| f(window, cx))
        });
        let dismiss = self.on_dismiss.map(|f| {
            IconButton::new("banner-dismiss", icons::CLOSE, "Dismiss")
                .on_click(move |_, window, cx| f(window, cx))
        });

        div()
            .id(self.id)
            .w_full()
            .max_w(px(640.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .pl(px(12.))
            .pr(px(6.))
            .py(px(6.))
            .min_h(px(44.))
            .rounded(px(metrics::RADIUS))
            .bg(p.well)
            .border_1()
            .border_color(color)
            .role(match self.level {
                Level::Error | Level::Warning => accesskit::Role::Alert,
                _ => accesskit::Role::Status,
            })
            .child(two_tone(icons2::INFO, ICON_SIZE, p.icon, color))
            // Long paths wrap anywhere rather than overflow.
            .child(message(self.message, p.text).overflow_hidden())
            .children(action)
            .children(dismiss)
    }
}

/// The message text of a toast or banner, taking the space the buttons leave.
fn message(text: SharedString, color: Hsla) -> Div {
    div().flex_1().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(color).child(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast(level: Level, id: u64) -> Toast {
        Toast { id, ..Toast::new(level, "") }
    }

    #[test]
    fn trimming_keeps_errors_over_info() {
        let mut toasts = vec![
            toast(Level::Error, 1),
            toast(Level::Info, 2),
            toast(Level::Success, 3),
            toast(Level::Error, 4),
        ];
        trim(&mut toasts, 2);
        let ids: Vec<u64> = toasts.iter().map(|t| t.id).collect();
        assert_eq!(ids, [1, 4]);
        // With only errors left, the oldest goes.
        trim(&mut toasts, 1);
        assert_eq!(toasts[0].id, 4);
    }

    #[test]
    fn only_info_and_success_dismiss_themselves() {
        assert!(Level::Info.auto_dismiss());
        assert!(Level::Success.auto_dismiss());
        assert!(!Level::Warning.auto_dismiss());
        assert!(!Level::Error.auto_dismiss());
    }
}
