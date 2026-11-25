//! Toasts and inline banners (S0.4.12).
//!
//! - [`Toasts`]: a per-window stack at the bottom right for transient news ("Saved
//!   snapshot", "Subtitles downloaded"). Info and success toasts go away on their
//!   own; errors stay until dismissed. At most [`MAX_VISIBLE`] show; older ones
//!   drop off. A toast may hold a thumbnail and several buttons (a snapshot's, S8.4.2).
//! - [`banner`]: an inline message inside a screen, for problems tied to it
//!   ("Settings couldn't be saved"), with an optional action.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Context, Div, ElementId, Hsla, ImageSource, RenderImage, SharedString, Stateful, Window,
    div, img, prelude::*, px,
};

use super::announcer::{Politeness, live_region};
use super::button::{Button, ButtonStyle, IconButton};
use super::icon::two_tone;
use super::metrics;
use crate::assets::{icons, two_tone as icons2};
use crate::i18n::DirectionalStyle;
use crate::theme::text_px;
use crate::theme::{self, Palette};

/// The most toasts shown at once; older ones drop off.
pub const MAX_VISIBLE: usize = 4;
/// How long info and success toasts stay.
const AUTO_DISMISS: Duration = Duration::from_secs(6);
/// A toast card's width, when the window has room for it.
const TOAST_WIDTH: f32 = 360.;
/// The size of the level icon in toasts and banners.
const ICON_SIZE: f32 = 18.;
/// The most room a toast's thumbnail takes, in points: 16:9 at a glance.
const THUMBNAIL_SIZE: (f32, f32) = (96., 54.);

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

    /// Errors and warnings interrupt a screen reader; news waits its turn (S9.1.2).
    fn politeness(self) -> Politeness {
        if self.auto_dismiss() { Politeness::Polite } else { Politeness::Assertive }
    }
}

/// Runs when the user presses a toast's or banner's button.
type Action = Rc<dyn Fn(&mut Window, &mut App)>;

/// A transient message for a [`Toasts`] stack.
pub struct Toast {
    id: u64,
    level: Level,
    message: SharedString,
    actions: Vec<ToastAction>,
    /// A picture shown at the toast's start, in BGRA.
    thumbnail: Option<Arc<RenderImage>>,
}

impl Toast {
    /// A toast showing `message` at `level`.
    pub fn new(level: Level, message: impl Into<SharedString>) -> Self {
        Self { id: 0, level, message: message.into(), actions: Vec::new(), thumbnail: None }
    }

    /// Adds a button labelled `label`, after any added before, that dismisses the toast and
    /// runs `f`.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.actions.push(ToastAction { label: label.into(), run: Rc::new(f), dismisses: true });
        self
    }

    /// Adds a button labelled `label`, after any added before, that runs `f` and leaves the
    /// toast up, for buttons worth pressing one after another (Open, then Copy).
    pub fn lasting_action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.actions.push(ToastAction { label: label.into(), run: Rc::new(f), dismisses: false });
        self
    }

    /// Shows `thumbnail` in place of the level icon, its buttons under the message.
    pub fn thumbnail(mut self, thumbnail: Arc<RenderImage>) -> Self {
        self.thumbnail = Some(thumbnail);
        self
    }
}

/// A toast's button.
struct ToastAction {
    label: SharedString,
    run: Action,
    /// Whether pressing it dismisses the toast.
    dismisses: bool,
}

/// A window's stack of toasts, drawn at its bottom right.
#[derive(Default)]
pub struct Toasts {
    #[allow(clippy::struct_field_names, reason = "the stack's toasts; `items` would say less")]
    toasts: Vec<Toast>,
    next_id: u64,
    /// The thumbnails drawn last time, freed from GPUI's atlas once their toast is gone.
    drawn: Vec<Arc<RenderImage>>,
}

#[cfg(test)]
impl Toasts {
    /// The messages showing, oldest first.
    pub fn messages(&self) -> Vec<String> {
        self.toasts.iter().map(|toast| toast.message.to_string()).collect()
    }

    /// What the button labelled `label` on the newest toast that has one runs.
    pub fn action(&self, label: &str) -> Option<Action> {
        let actions = self.toasts.iter().rev().flat_map(|toast| &toast.actions);
        actions.into_iter().find(|action| action.label == label).map(|action| action.run.clone())
    }
}

impl Toasts {
    /// An empty stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows `toast`, dropping old ones past [`MAX_VISIBLE`], and schedules its
    /// dismissal if its level goes away alone. Returns its id, for [`Self::dismiss`].
    pub fn push(&mut self, mut toast: Toast, cx: &mut Context<Self>) -> u64 {
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
        id
    }

    /// A button of the toast with `id` was pressed: dismisses it if the button `dismisses`.
    fn press(&mut self, id: u64, dismisses: bool, cx: &mut Context<Self>) {
        if dismisses {
            self.dismiss(id, cx);
        }
    }

    /// Removes the toast with `id`, if it's still showing.
    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }
}

impl Toasts {
    /// Frees what GPUI keeps for the thumbnails of toasts that are gone.
    fn free_thumbnails(&mut self, window: &mut Window) {
        let shown: Vec<_> =
            self.toasts.iter().filter_map(|toast| toast.thumbnail.clone()).collect();
        for old in self.drawn.drain(..) {
            if !shown.iter().any(|kept| Arc::ptr_eq(kept, &old)) {
                window.drop_image(old).ok();
            }
        }
        self.drawn = shown;
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
    /// One toast card: icon or thumbnail, message, the buttons and a dismiss button. With a
    /// thumbnail or several buttons, the buttons go under the message, so it keeps its width.
    fn render_toast(toast: &Toast, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = toast.id;
        let buttons = toast.actions.iter().enumerate().map(|(index, action)| {
            let (run, dismisses) = (action.run.clone(), action.dismisses);
            let button_id = ElementId::NamedInteger("toast-action".into(), id * 8 + index as u64);
            Button::new(button_id, action.label.clone()).style(ButtonStyle::Ghost).on_click(
                cx.listener(move |this, _, window, cx| {
                    this.press(id, dismisses, cx);
                    run(window, cx);
                }),
            )
        });
        // Long paths wrap anywhere rather than overflow.
        let message = message(toast.message.clone(), p.text_strong).pt(px(4.)).overflow_hidden();
        let card = live_region(toast_card(id, p), toast.message.clone(), toast.level.politeness());
        let close = IconButton::new(("toast-close", id as usize), icons::CLOSE, tr!("Dismiss"))
            .on_click(cx.listener(move |this, _, _, cx| this.dismiss(id, cx)));
        let lead = match toast.thumbnail.clone() {
            Some(thumbnail) => thumbnail_image(thumbnail, p),
            None => div().pt(px(4.)).child(two_tone(
                icons2::INFO,
                ICON_SIZE,
                p.icon,
                toast.level.color(p),
            )),
        };
        if toast.thumbnail.is_none() && toast.actions.len() <= 1 {
            return card.child(lead).child(message).children(buttons).child(close);
        }
        let body = div()
            .flex_1()
            .min_w_0()
            .flex_row_start()
            .flex_col()
            .child(message)
            .child(div().flex_row_start().flex_wrap().ms(px(-8.)).children(buttons));
        card.child(lead).child(body).child(close)
    }
}

/// An empty toast card for the toast with `id`.
fn toast_card(id: u64, p: &Palette) -> Stateful<Div> {
    div()
        .id(("toast", id as usize))
        .occlude()
        .w(px(TOAST_WIDTH))
        .max_w_full()
        .flex_row_start()
        .items_start()
        .gap(px(10.))
        .ps(px(12.))
        .pe(px(4.))
        .py(px(8.))
        .rounded(px(metrics::RADIUS_LARGE))
        .bg(p.menu)
        .border_1()
        .border_color(p.divider)
        .shadow_lg()
}

/// A toast's thumbnail, fitted into [`THUMBNAIL_SIZE`] on black.
fn thumbnail_image(thumbnail: Arc<RenderImage>, p: &Palette) -> Div {
    let (width, height) = THUMBNAIL_SIZE;
    div()
        .mt(px(4.))
        .flex_none()
        .w(px(width))
        .h(px(height))
        .rounded(px(metrics::RADIUS))
        .overflow_hidden()
        .border_1()
        .border_color(p.divider)
        .bg(gpui::black())
        .child(img(ImageSource::Render(thumbnail)).size_full().object_fit(gpui::ObjectFit::Contain))
}

impl Render for Toasts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.free_thumbnails(window);
        let p = theme::palette(window, cx);
        let toasts = self.toasts.iter().map(|toast| Self::render_toast(toast, &p, cx));

        div()
            .absolute()
            .inset_end(px(16.))
            .bottom(px(16.))
            .flex_row_start()
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
            IconButton::new("banner-dismiss", icons::CLOSE, tr!("Dismiss"))
                .on_click(move |_, window, cx| f(window, cx))
        });

        let region = div().id(self.id);
        live_region(region, self.message.clone(), self.level.politeness())
            .w_full()
            .max_w(px(640.))
            .flex_row_start()
            .items_center()
            .gap(px(10.))
            .ps(px(12.))
            .pe(px(6.))
            .py(px(6.))
            .min_h(px(44.))
            .rounded(px(metrics::RADIUS))
            .bg(p.well)
            .border_1()
            .border_color(color)
            .child(two_tone(icons2::INFO, ICON_SIZE, p.icon, color))
            // Long paths wrap anywhere rather than overflow.
            .child(message(self.message, p.text).overflow_hidden())
            .children(action)
            .children(dismiss)
    }
}

/// The message text of a toast or banner, taking the space the buttons leave.
fn message(text: SharedString, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .text_size(text_px(14.))
        .line_height(text_px(20.))
        .text_color(color)
        .child(text)
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
    fn errors_and_warnings_interrupt_screen_readers() {
        assert_eq!(Level::Error.politeness(), Politeness::Assertive);
        assert_eq!(Level::Warning.politeness(), Politeness::Assertive);
        assert_eq!(Level::Info.politeness(), Politeness::Polite);
    }

    #[test]
    fn only_info_and_success_dismiss_themselves() {
        assert!(Level::Info.auto_dismiss());
        assert!(Level::Success.auto_dismiss());
        assert!(!Level::Warning.auto_dismiss());
        assert!(!Level::Error.auto_dismiss());
    }
}
