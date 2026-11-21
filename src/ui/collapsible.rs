//! Collapsible panel sections (S0.4.10), as in the audio and video panels
//! (`player/panels/*/*-collapsed`, `*-expanded`): a 48px header with a muted title
//! and a chevron, then the section's rows. The header is a button (Enter/Space
//! toggles), and the section ends with a divider.

use std::rc::Rc;

use gpui::{AnyElement, App, ElementId, SharedString, Window, div, prelude::*, px};

use super::icon::icon;
use super::{focus, metrics};
use crate::assets::icons;
use crate::theme;

type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Collapsible {
    id: ElementId,
    title: SharedString,
    expanded: bool,
    content: Vec<AnyElement>,
    on_toggle: Option<ToggleHandler>,
}

impl Collapsible {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, expanded: bool) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            expanded,
            content: Vec::new(),
            on_toggle: None,
        }
    }

    /// Called with the new expanded state.
    pub fn on_toggle(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

impl ParentElement for Collapsible {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl RenderOnce for Collapsible {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let expanded = self.expanded;
        let hover = p.text;

        let header = div()
            .id("header")
            .h(px(metrics::SECTION_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(14.))
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .text_size(px(metrics::TEXT))
            .text_color(p.text_muted)
            .tab_index(0)
            .cursor_pointer()
            .aria_expanded(expanded)
            .aria_label(self.title.clone())
            .hover(move |s| s.text_color(hover))
            .when_some(self.on_toggle, |h, f| {
                h.on_click(move |_, window, cx| f(&!expanded, window, cx))
            })
            .child(div().truncate().child(self.title))
            .child(icon(
                if expanded {
                    icons::CHEVRON_UP
                } else {
                    icons::CHEVRON_DOWN
                },
                20.,
                p.icon,
            ));

        div()
            .id(self.id)
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(p.divider)
            .child(focus::ring(header, &p))
            .when(expanded, |section| {
                section.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .px(px(16.))
                        .pb(px(16.))
                        .children(self.content),
                )
            })
    }
}
