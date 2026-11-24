//! Tooltips (S0.4.2): a label and an optional shortcut, e.g. "Play (Space)".
//!
//! GPUI shows them after a hover delay and places them near the pointer, inside
//! the window. Attach with `.tooltip(tooltip::text("Play", Some("Space")))`.

use gpui::{AnyView, App, Context, SharedString, Window, div, prelude::*, px};

use crate::theme;
use crate::theme::text_px;

/// The tooltip view GPUI shows on hover.
pub struct Tooltip {
    label: SharedString,
    shortcut: Option<SharedString>,
}

/// The label with its shortcut in parentheses, as tooltips and accessible names
/// show it.
pub fn describe(label: &str, shortcut: Option<&str>) -> String {
    match shortcut {
        Some(shortcut) if !shortcut.is_empty() => tr!("{label} ({shortcut})", label, shortcut),
        _ => label.to_owned(),
    }
}

/// A tooltip builder for `.tooltip(...)`.
pub fn text(
    label: impl Into<SharedString>,
    shortcut: Option<impl Into<SharedString>>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label = label.into();
    let shortcut = shortcut.map(Into::into);
    move |_, cx| cx.new(|_| Tooltip { label: label.clone(), shortcut: shortcut.clone() }).into()
}

impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        // GPUI positions the tooltip's root at the pointer; the padding keeps the
        // bubble clear of the cursor.
        div().pl(px(10.)).pt(px(20.)).child(
            div()
                .max_w(px(320.))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .bg(p.tooltip)
                .border_1()
                .border_color(p.divider)
                .shadow_md()
                .text_size(text_px(12.))
                .line_height(text_px(16.))
                .text_color(p.tooltip_text)
                .flex()
                .flex_row()
                .gap(px(6.))
                .child(self.label.clone())
                .when_some(self.shortcut.clone(), |this, shortcut| {
                    this.child(div().text_color(p.text_muted).child(shortcut))
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::describe;

    #[test]
    fn shortcut_in_parentheses() {
        assert_eq!(describe("Play", Some("Space")), "Play (Space)");
        assert_eq!(describe("Play", None), "Play");
        assert_eq!(describe("Play", Some("")), "Play");
    }
}
