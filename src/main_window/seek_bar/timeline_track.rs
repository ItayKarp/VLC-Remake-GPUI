//! Drawing the subtitle timeline (S6.6.1, `controls-sub_timeline.png`): `<` and `−` at the bar's
//! left end, `+` and `>` at its right, and between them a block for each line in the window, the
//! part already played tinted with the accent and the rest grey, over a track tinted as played up
//! to the playhead. Only the lines in the window are drawn, however many the file has.

use gpui::{
    App, ClickEvent, Context, CursorStyle, Div, Hsla, Stateful, Window, accesskit, div, prelude::*,
    px, relative,
};

use super::cue_drag::MIN_BLOCK;
use super::subtitle_timeline::Frame;
use super::timeline::{Pan, Zoom};
use crate::assets::icons;
use crate::engine::player::CueSpan;
use crate::main_window::MainWindow;
use crate::theme::{Palette, blend};
use crate::ui::icon::icon;
use crate::ui::metrics::{self, MIN_TARGET};
use crate::ui::{focus, tooltip};

/// How much of the accent a played line mixes into the track, and how much of the text an
/// upcoming one.
const PLAYED_LINE_TINT: f32 = 0.45;
const UPCOMING_LINE_TINT: f32 = 0.2;
/// The space above and below the lines' blocks.
const LINE_INSET: f32 = 3.;
/// The buttons' size, their icons' and the space around them.
const BUTTON_WIDTH: f32 = 16.;
const BUTTON_HEIGHT: f32 = 20.;
const BUTTON_ICON: f32 = 14.;
const BUTTON_GAP: f32 = 2.;
/// How faint a button that would do nothing is.
const DISABLED_OPACITY: f32 = 0.4;

/// The inside of the track: the played tint up to the playhead, the lines, and the handle at
/// `handle` (0–1) while it is in the window.
pub(super) fn lines(p: &Palette, frame: &Frame, handle: Option<f32>, played_tint: f32) -> Div {
    let track = p.sunken;
    let played = frame.visible.fraction_of(frame.position);
    let colors = LineColors {
        played: blend(p.accent, track, PLAYED_LINE_TINT),
        upcoming: blend(p.text, track, UPCOMING_LINE_TINT),
    };
    let blocks = frame.spans.iter().map(|span| line_block(frame, *span, colors));
    let handle = handle.map(|fraction| super::render::handle(p, fraction));
    div()
        .absolute()
        .inset_0()
        .rounded(px(3.))
        .overflow_hidden()
        .child(div().h_full().w(relative(played)).bg(blend(p.accent, track, played_tint)))
        .children(blocks)
        .children(handle)
}

/// The colors of a line's played and upcoming parts.
#[derive(Clone, Copy)]
struct LineColors {
    played: Hsla,
    upcoming: Hsla,
}

/// One line's block, its played part drawn over it.
fn line_block(frame: &Frame, span: CueSpan, colors: LineColors) -> Div {
    let shown = frame.visible;
    let left = shown.fraction_of(span.start);
    let block = |right: f32, color: Hsla| {
        div()
            .absolute()
            .top(px(LINE_INSET))
            .bottom(px(LINE_INSET))
            .left(relative(left))
            .w(relative(right - left))
            .rounded(px(2.))
            .bg(color)
    };
    let whole = block(shown.fraction_of(span.end), colors.upcoming).min_w(px(MIN_BLOCK));
    let played = (frame.position > span.start).then(|| {
        let played_to = shown.fraction_of(span.end.min(frame.position));
        let part = block(played_to, colors.played);
        if span.end <= frame.position { part.min_w(px(MIN_BLOCK)) } else { part }
    });
    div().absolute().inset_0().cursor(CursorStyle::OpenHand).child(whole).children(played)
}

impl MainWindow {
    /// Puts the timeline's `<` `−` buttons before `track` in `bar` and `+` `>` after it.
    pub(super) fn timeline_ends(
        &self,
        bar: Stateful<Div>,
        track: Stateful<Div>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let can = self.timeline_moves(cx);
        let earlier =
            button("timeline-earlier", icons::CHEVRON_LEFT, tr!("Show earlier"), can.earlier);
        let zoom_out = button("timeline-zoom-out", icons::REMOVE, tr!("Zoom out"), can.zoom_out);
        let zoom_in = button("timeline-zoom-in", icons::ADD, tr!("Zoom in"), can.zoom_in);
        let later = button("timeline-later", icons::CHEVRON_RIGHT, tr!("Show later"), can.later);
        bar.gap(px(BUTTON_GAP))
            .px(px(BUTTON_GAP))
            .child(
                earlier.click(p, cx.listener(|this, _, _, cx| this.pan_timeline(Pan::Earlier, cx))),
            )
            .child(
                zoom_out.click(p, cx.listener(|this, _, _, cx| this.zoom_timeline(Zoom::Out, cx))),
            )
            .child(track)
            .child(zoom_in.click(p, cx.listener(|this, _, _, cx| this.zoom_timeline(Zoom::In, cx))))
            .child(later.click(p, cx.listener(|this, _, _, cx| this.pan_timeline(Pan::Later, cx))))
    }
}

/// One of the timeline's small accent buttons, before its click is attached.
struct EndButton {
    id: &'static str,
    icon: &'static str,
    label: &'static str,
    enabled: bool,
}

fn button(id: &'static str, icon: &'static str, label: &'static str, enabled: bool) -> EndButton {
    EndButton { id, icon, label, enabled }
}

impl EndButton {
    /// The button, running `on_click` when pressed while it does something.
    fn click(
        self,
        p: &Palette,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let face = div()
            .w(px(BUTTON_WIDTH))
            .h(px(BUTTON_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .border_1()
            .border_color(p.accent)
            .child(icon(self.icon, BUTTON_ICON, p.accent));
        // The drawn face is smaller than a finger or a shaky hand needs, so the button around
        // it takes [`MIN_TARGET`] each way, overhanging the track's height rather than
        // growing it (S9.1.4).
        let overhang = (MIN_TARGET - BUTTON_HEIGHT) / 2.;
        let button = div()
            .id(self.id)
            .flex_none()
            .size(px(MIN_TARGET))
            .my(px(-overhang))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(metrics::RADIUS))
            .role(accesskit::Role::Button)
            .aria_label(self.label)
            .tooltip(tooltip::text(self.label, None::<&str>))
            .child(face);
        if !self.enabled {
            return button.opacity(DISABLED_OPACITY);
        }
        focus::outline_ring(button.tab_index(0).key_context(focus::CONTROL).cursor_pointer(), p)
            .on_click(on_click)
    }
}

#[cfg(test)]
mod tests;
