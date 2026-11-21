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
    AnyElement, App, Context, ElementId, Hsla, SharedString, Window, accesskit, div, prelude::*, px,
};

use super::button::{Button, ButtonStyle, IconButton};
use super::icon::two_tone;
use super::metrics;
use crate::assets::{icons, two_tone as icons2};
use crate::theme::{self, Palette};

pub const MAX_VISIBLE: usize = 4;
const AUTO_DISMISS: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

impl Level {
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

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct Toast {
    id: u64,
    level: Level,
    message: SharedString,
    action: Option<(SharedString, Action)>,
}

impl Toast {
    pub fn new(level: Level, message: impl Into<SharedString>) -> Self {
        Self {
            id: 0,
            level,
            message: message.into(),
            action: None,
        }
    }

    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(f)));
        self
    }
}

#[derive(Default)]
pub struct Toasts {
    toasts: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    pub fn new() -> Self {
        Self::default()
    }

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

    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }
}

/// Drops the oldest auto-dismissing toasts first, then the oldest of any kind.
fn trim(toasts: &mut Vec<Toast>, max: usize) {
    while toasts.len() > max {
        let index = toasts
            .iter()
            .position(|t| t.level.auto_dismiss())
            .unwrap_or(0);
        toasts.remove(index);
    }
}

impl Render for Toasts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let toasts = self.toasts.iter().map(|toast| {
            let id = toast.id;
            div()
                .id(("toast", id as usize))
                .occlude()
                .w(px(360.))
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
                    18.,
                    p.icon,
                    toast.level.color(&p),
                )))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pt(px(4.))
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(p.text_strong)
                        .child(toast.message.clone()),
                )
                .when_some(toast.action.clone(), |t, (label, action)| {
                    t.child(
                        Button::new(("toast-action", id as usize), label)
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.dismiss(id, cx);
                                action(window, cx);
                            })),
                    )
                })
                .child(
                    IconButton::new(("toast-close", id as usize), icons::CLOSE, "Dismiss")
                        .on_click(cx.listener(move |this, _, _, cx| this.dismiss(id, cx))),
                )
        });

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

pub fn banner(id: impl Into<ElementId>, level: Level, message: impl Into<SharedString>) -> Banner {
    Banner {
        id: id.into(),
        level,
        message: message.into(),
        action: None,
        on_dismiss: None,
    }
}

impl Banner {
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(f)));
        self
    }

    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let color = self.level.color(&p);
        let actions: Vec<AnyElement> = [
            self.action.map(|(label, f)| {
                Button::new("banner-action", label)
                    .style(ButtonStyle::Filled)
                    .on_click(move |_, window, cx| f(window, cx))
                    .into_any_element()
            }),
            self.on_dismiss.map(|f| {
                IconButton::new("banner-dismiss", icons::CLOSE, "Dismiss")
                    .on_click(move |_, window, cx| f(window, cx))
                    .into_any_element()
            }),
        ]
        .into_iter()
        .flatten()
        .collect();

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
            .child(two_tone(icons2::INFO, 18., p.icon, color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(p.text)
                    // Long paths wrap anywhere rather than overflow.
                    .overflow_hidden()
                    .child(self.message),
            )
            .children(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast(level: Level, id: u64) -> Toast {
        Toast {
            id,
            ..Toast::new(level, "")
        }
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
