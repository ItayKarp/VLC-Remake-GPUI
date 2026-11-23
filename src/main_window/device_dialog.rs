//! The look Open Disc and Open Capture Device share (S8.9). Neither has a design, so both take
//! the Open Network Stream dialog's (`dialogs/open_network_stream`): a dark header with the
//! title and ✕, sections with a heading and a line about them, and Cancel at the left with Play
//! at the right.

use gpui::{
    AnyElement, App, ClickEvent, DismissEvent, Div, EventEmitter, FontWeight, SharedString, Window,
    accesskit, div, prelude::*, px,
};

use crate::assets::icons;
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::metrics;

/// The dialog's width when the window has room.
const WIDTH: f32 = 640.;
/// Space kept between the dialog and the window edges.
const WINDOW_MARGIN: f32 = 24.;
/// The header's height, and the space inside the dialog's edges.
const HEADER_HEIGHT: f32 = 50.;
const PADDING: f32 = 20.;
/// The space between the sections, above a section's controls, and above Cancel and Play.
const SECTION_GAP: f32 = 28.;
pub(super) const FIELD_GAP: f32 = 14.;
const FOOTER_GAP: f32 = 36.;
/// Cancel's and Play's width.
const FOOTER_BUTTON_WIDTH: f32 = 100.;
/// The section headings' and descriptions' text sizes.
const HEADING_TEXT: f32 = 17.;
const DESCRIPTION_TEXT: f32 = 14.;

/// A dialog's parts, top to bottom.
pub(super) struct Frame {
    /// Its id, which its parts' ids start with.
    pub id: &'static str,
    /// Its title, in the header and for screen readers.
    pub title: &'static str,
    /// Its sections.
    pub sections: Vec<Div>,
    /// Whether Play does something now.
    pub can_play: bool,
}

impl Frame {
    /// The whole dialog, Play running `on_play` and Cancel and ✕ closing it.
    pub(super) fn render<V: EventEmitter<DismissEvent>>(
        self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<V>,
        on_play: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
    ) -> impl IntoElement {
        let viewport = window.viewport_size();
        let body = div()
            .id(SharedString::from(format!("{}-body", self.id)))
            .flex_initial()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(PADDING))
            .pt(px(PADDING))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .children(self.sections);
        div()
            .id(self.id)
            .role(accesskit::Role::Dialog)
            .aria_label(self.title)
            .w(px(WIDTH))
            .max_w(viewport.width - px(2. * WINDOW_MARGIN))
            .max_h(viewport.height - px(2. * WINDOW_MARGIN))
            .flex()
            .flex_col()
            .rounded(px(metrics::RADIUS_LARGE + 4.))
            .overflow_hidden()
            .bg(p.popover)
            .border_1()
            .border_color(p.divider)
            .shadow_lg()
            .text_color(p.text)
            .child(header(self.id, self.title, p, cx))
            .child(body)
            .child(footer(self.id, self.can_play, cx, on_play))
    }
}

/// A section: `heading`, a muted line about it, then whatever is added to it.
pub(super) fn section(
    heading: &'static str,
    description: impl Into<SharedString>,
    p: &Palette,
) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(HEADING_TEXT))
                .font_weight(FontWeight::NORMAL)
                .text_color(p.text_strong)
                .child(heading),
        )
        .child(
            div()
                .pt(px(6.))
                .text_size(px(DESCRIPTION_TEXT))
                .text_color(p.text_muted)
                .child(description.into()),
        )
}

/// A section's controls, below its description.
pub(super) fn controls(child: impl IntoElement) -> Div {
    div().pt(px(FIELD_GAP)).child(child)
}

/// A small muted line, such as a note under a field.
pub(super) fn note(text: impl Into<SharedString>, p: &Palette) -> AnyElement {
    div()
        .pt(px(8.))
        .text_size(px(metrics::TEXT_SMALL))
        .text_color(p.text_muted)
        .child(text.into())
        .into_any_element()
}

/// The title and ✕ on a darker band.
fn header<V: EventEmitter<DismissEvent>>(
    id: &'static str,
    title: &'static str,
    p: &Palette,
    cx: &mut Context<V>,
) -> Div {
    let close = IconButton::new(SharedString::from(format!("{id}-close")), icons::CLOSE, "Close")
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DismissEvent)));
    div()
        .flex_none()
        .h(px(HEADER_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .pl(px(PADDING))
        .pr(px(PADDING - 6.))
        .bg(p.sunken)
        .text_size(px(DESCRIPTION_TEXT + 1.))
        .child(title)
        .child(close)
}

/// Cancel at the left, Play at the right.
fn footer<V: EventEmitter<DismissEvent>>(
    id: &'static str,
    can_play: bool,
    cx: &mut Context<V>,
    on_play: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> Div {
    let cancel = Button::new(SharedString::from(format!("{id}-cancel")), "Cancel")
        .style(ButtonStyle::Filled)
        .width(FOOTER_BUTTON_WIDTH)
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DismissEvent)));
    let play = Button::new(SharedString::from(format!("{id}-play")), "Play")
        .style(if can_play { ButtonStyle::Primary } else { ButtonStyle::Ghost })
        .width(FOOTER_BUTTON_WIDTH)
        .disabled(!can_play)
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| on_play(this, window, cx)));
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .px(px(PADDING))
        .pt(px(FOOTER_GAP))
        .pb(px(PADDING))
        .child(cancel)
        .child(play)
}

/// What a dialog hands what it chose to, once it has closed.
pub(super) type Then<T> = std::rc::Rc<dyn Fn(T, &mut Window, &mut App)>;

/// Closes a dialog and hands what it chose to `then`, once it has closed.
pub(super) fn close_with<V: EventEmitter<DismissEvent>, T: 'static>(
    chosen: T,
    then: Then<T>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    cx.emit(DismissEvent);
    window.defer(cx, move |window, cx| then(chosen, window, cx));
}
