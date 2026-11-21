//! The track popover (S2.6, `controls-video.png`, `controls-audio.png`,
//! `controls-subtitles.png`): one panel with Video, Audio and Subtitles tabs above the bar's
//! right end. The Audio and Subtitles buttons open their tabs; the Video tab has no button and
//! shows only when the media has two or more video tracks (D22). The tabs' contents are in
//! `sections.rs`.
//!
//! It closes on an outside click, Escape, the same button again, opening another popover (the
//! `…` menus), the window losing focus, and when its media goes. Only one popover shows at a
//! time.
//!
//! **Show Preferences** opens the matching tab of the in-player settings panel (S2.7,
//! `settings_panel.rs`). **Open Advanced Settings**, in that panel's footer, opens the matching
//! category of the Settings window instead: the popover and the panel change the playing media,
//! the Settings window the defaults.

mod sections;

use gpui::{
    ClickEvent, Context, Div, FocusHandle, KeyDownEvent, MouseDownEvent, Pixels, Point, Stateful,
    Window, div, prelude::*, px,
};

use super::MainWindow;
use super::bar_panel;
use super::settings_panel::PanelTab;
use crate::assets::icons;
use crate::engine::tracks::TrackKind;
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::focus;
use crate::ui::popover::BoundsTracker;
use crate::ui::tabs::{Tab, Tabs};

/// The popover's height in the designs.
const DESIGN_HEIGHT: f32 = 524.;
/// The footer's padding around Show Preferences.
const FOOTER_PADDING: f32 = 20.;
/// The fewest video tracks that bring the Video tab (D22).
const VIDEO_TAB_TRACKS: usize = 2;

/// The popover's state: which tab is open, its focus, and where the buttons that open it are.
pub(super) struct TrackPopover {
    open: Option<TrackKind>,
    focus: FocusHandle,
    /// The Audio and Subtitles buttons, so pressing one doesn't count as an outside click.
    audio_button: BoundsTracker,
    subtitles_button: BoundsTracker,
}

impl TrackPopover {
    /// A closed popover.
    pub(super) fn new(cx: &mut Context<MainWindow>) -> Self {
        Self {
            open: None,
            focus: cx.focus_handle(),
            audio_button: BoundsTracker::new(),
            subtitles_button: BoundsTracker::new(),
        }
    }

    /// Whether the popover shows the tab of `kind`.
    pub(super) fn shows(&self, kind: TrackKind) -> bool {
        self.open == Some(kind)
    }

    /// The tracker to put inside the button that opens the tab of `kind`.
    pub(super) fn button(&self, kind: TrackKind) -> &BoundsTracker {
        match kind {
            TrackKind::Subtitle => &self.subtitles_button,
            _ => &self.audio_button,
        }
    }

    fn over_a_button(&self, position: Point<Pixels>) -> bool {
        self.audio_button.contains(position) || self.subtitles_button.contains(position)
    }
}

impl MainWindow {
    /// Opens the tab of `kind`, or closes the popover when that tab is open (its button pressed
    /// again).
    pub(super) fn toggle_track_popover(
        &mut self,
        kind: TrackKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pressed(self.track_popover.open, kind) {
            Some(kind) => self.open_track_popover(kind, window, cx),
            None => self.close_track_popover(window, cx),
        }
    }

    /// Opens the popover on the tab of `kind`, closing the `…` menus, and gives it the keyboard.
    pub(super) fn open_track_popover(
        &mut self,
        kind: TrackKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.control_bar.close_overflow();
        self.close_settings_panel(window, cx);
        self.track_popover.open = Some(kind);
        // After the click that opened it, which focuses the clicked button once its handlers ran.
        let handle = self.track_popover.focus.clone();
        window.defer(cx, move |window, cx| handle.focus(window, cx));
        cx.notify();
    }

    /// Closes the popover, handing the keyboard back to the player if the popover had it.
    pub(super) fn close_track_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.track_popover.open.take().is_none() {
            return;
        }
        if self.track_popover.focus.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// The popover over the video's bottom right, while open. It closes once its media is gone
    /// or the window goes fullscreen, which hides the bar it belongs to (until S2.8).
    pub(super) fn render_track_popover(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let open = self.track_popover.open?;
        if window.is_fullscreen() || !self.has_active_media(cx) {
            self.close_track_popover(window, cx);
            return None;
        }
        let snapshot = self.track_snapshot(cx)?;
        let tabs = tabs(snapshot.tracks.of(TrackKind::Video).count());
        let kind = if tabs.contains(&open) { open } else { TrackKind::Audio };
        let body = Self::render_track_tab(kind, &snapshot, p, cx);
        let footer = footer(kind, bar_panel::width(window), cx);
        Some(
            bar_panel::surface("track-popover", DESIGN_HEIGHT, window, p)
                .track_focus(&self.track_popover.focus)
                .key_context(focus::context(&["TrackPopover", focus::CONTROL]))
                .on_key_down(cx.listener(Self::on_track_popover_key))
                .on_mouse_down_out(cx.listener(Self::on_mouse_down_out_of_tracks))
                .child(Self::render_track_tabs(&tabs, kind, p, cx))
                .child(body)
                .child(footer),
        )
    }

    /// Whether media is open (opening, playing, paused or buffering): what the popovers act on.
    pub(super) fn has_active_media(&self, cx: &gpui::App) -> bool {
        self.loaded_player(cx).is_some_and(|player| player.read(cx).state().is_active())
    }

    fn render_track_tabs(
        tabs: &[TrackKind],
        shown: TrackKind,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let items = tabs.iter().map(|kind| Tab::new(Some(tab_icon(*kind)), tab_label(*kind)));
        let selected = tabs.iter().position(|kind| *kind == shown).unwrap_or_default();
        let tabs = tabs.to_vec();
        let bar = Tabs::new("track-tabs", items.collect(), selected).on_select(cx.listener(
            move |this, index: &usize, window, cx| {
                if let Some(kind) = tabs.get(*index) {
                    this.open_track_popover(*kind, window, cx);
                }
            },
        ));
        div().flex_none().bg(p.well).child(bar)
    }

    fn on_track_popover_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            self.close_track_popover(window, cx);
        }
    }

    fn on_mouse_down_out_of_tracks(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Its own buttons toggle it on click; closing here would reopen it there.
        if !self.track_popover.over_a_button(event.position) {
            self.close_track_popover(window, cx);
        }
    }
}

/// The footer: Show Preferences, which opens the settings panel on the tab of `kind`.
fn footer(kind: TrackKind, width: f32, cx: &mut Context<MainWindow>) -> Div {
    let tab = PanelTab::of(kind);
    let button = Button::new("show-preferences", "Show Preferences")
        .width(width - 2. * FOOTER_PADDING)
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.open_settings_panel(tab, window, cx);
        }));
    div().flex_none().p(px(FOOTER_PADDING)).child(button)
}

/// The popover's tab after the button of `kind` is pressed: closed when that tab was open.
fn pressed(open: Option<TrackKind>, kind: TrackKind) -> Option<TrackKind> {
    (open != Some(kind)).then_some(kind)
}

/// The tabs for media with `video_tracks` video tracks: Video only with two or more (D22).
fn tabs(video_tracks: usize) -> Vec<TrackKind> {
    let video = (video_tracks >= VIDEO_TAB_TRACKS).then_some(TrackKind::Video);
    video.into_iter().chain([TrackKind::Audio, TrackKind::Subtitle]).collect()
}

/// The tab's icon, shared with the settings panel's matching tab.
pub(super) fn tab_icon(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => icons::VIDEO,
        TrackKind::Audio => icons::AUDIO,
        TrackKind::Subtitle => icons::SUBTITLES,
    }
}

fn tab_label(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "Video",
        TrackKind::Audio => "Audio",
        TrackKind::Subtitle => "Subtitles",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_button_closes_and_another_switches_tabs() {
        assert_eq!(pressed(None, TrackKind::Audio), Some(TrackKind::Audio));
        assert_eq!(pressed(Some(TrackKind::Audio), TrackKind::Audio), None);
        assert_eq!(pressed(Some(TrackKind::Audio), TrackKind::Subtitle), Some(TrackKind::Subtitle));
    }

    #[test]
    fn the_video_tab_needs_two_video_tracks() {
        assert_eq!(tabs(1), [TrackKind::Audio, TrackKind::Subtitle]);
        assert_eq!(tabs(2)[0], TrackKind::Video);
    }
}
