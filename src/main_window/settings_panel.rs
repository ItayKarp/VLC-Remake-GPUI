//! The settings panel (S2.7, `controls-playback.png`, `controls-video_settings.png`): the gear
//! button's panel over the bar's right end, with Playback, Video, Audio and Subtitles tabs. It
//! changes the playing media; the Settings window holds the defaults.
//!
//! How the two relate (S5.8.4, D24): a panel value lasts the session, carried from item to item,
//! and is never saved; it wins over its default changed in Settings meanwhile until the panel
//! resets it; and Save as Default (S7.1.5) writes a tab's values into its Settings page. Speed
//! already works this way (S2.7.4), its default applying at the next launch.
//!
//! The Playback tab holds the speed (`playback.rs`). Its other designed rows (Show Subtitle
//! Timeline on Seekbar, Show Statistics, Reverse, Sleep Timer, Audio Visualizer) come with their
//! stories (S6.6, S8.13, S7.8) and are hidden until then, as are the Video, Audio and Subtitles
//! tabs' settings (Stages 6 and 7). Those three tabs have a footer that stays put while they
//! scroll: **Default Preset**, disabled until presets exist (S5.11), and **Open Advanced
//! Settings**, which opens the Settings window on the tab's page (S5.2.1).
//!
//! It closes like the track popover: an outside click, Escape, the gear again, opening another
//! popover or menu, the window losing focus, its media going, and fullscreen. The track popover's
//! Show Preferences opens it on the matching tab.

mod playback;

use gpui::{
    Context, Div, FocusHandle, KeyDownEvent, MouseDownEvent, SharedString, Stateful, Window, div,
    prelude::*, px,
};

pub(super) use playback::SpeedControls;

use super::MainWindow;
use super::bar_panel;
use super::track_popover::tab_icon;
use crate::assets::icons;
use crate::engine::tracks::TrackKind;
use crate::settings::registry::Category;
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::focus;
use crate::ui::popover::{self, BoundsTracker};
use crate::ui::tabs::{Tab, Tabs};

/// The panel's height in the designs.
const DESIGN_HEIGHT: f32 = 945.;
/// The footer's padding, and the space between its two controls.
const FOOTER_PADDING: f32 = 20.;
const FOOTER_GAP: f32 = 8.;
/// The side padding of the tabs' contents, and the muted notes' text size.
const INSET: f32 = 24.;
const NOTE_TEXT: f32 = 14.;

/// The panel's tabs, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelTab {
    Playback,
    Video,
    Audio,
    Subtitles,
}

impl PanelTab {
    const ALL: [Self; 4] = [Self::Playback, Self::Video, Self::Audio, Self::Subtitles];

    /// The tab that holds the settings of tracks of `kind` (Show Preferences).
    pub(super) fn of(kind: TrackKind) -> Self {
        match kind {
            TrackKind::Video => Self::Video,
            TrackKind::Audio => Self::Audio,
            TrackKind::Subtitle => Self::Subtitles,
        }
    }

    /// The Settings window's page with the tab's defaults.
    fn category(self) -> Category {
        match self {
            Self::Playback => Category::Playback,
            Self::Video => Category::Video,
            Self::Audio => Category::Audio,
            Self::Subtitles => Category::Subtitles,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Playback => "Playback",
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Subtitles => "Subtitles",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Playback => icons::PLAY,
            Self::Video => tab_icon(TrackKind::Video),
            Self::Audio => tab_icon(TrackKind::Audio),
            Self::Subtitles => tab_icon(TrackKind::Subtitle),
        }
    }
}

/// The panel's state: the open tab, its focus, the gear's bounds and the speed controls.
pub(super) struct SettingsPanel {
    open: Option<PanelTab>,
    focus: FocusHandle,
    /// The gear button, so pressing it doesn't count as an outside click.
    button: BoundsTracker,
    speed: SpeedControls,
}

impl SettingsPanel {
    /// A closed panel.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        Self {
            open: None,
            focus: cx.focus_handle(),
            button: BoundsTracker::new(),
            speed: SpeedControls::new(window, cx),
        }
    }

    /// Whether the panel is open.
    pub(super) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The tracker to put inside the gear button.
    pub(super) fn button(&self) -> &BoundsTracker {
        &self.button
    }
}

impl MainWindow {
    /// Opens the panel on its Playback tab, or closes it (the gear pressed again).
    pub(super) fn toggle_settings_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_panel.is_open() {
            self.close_settings_panel(window, cx);
        } else {
            self.open_settings_panel(PanelTab::Playback, window, cx);
        }
    }

    /// Opens the panel on its Playback tab (the overflow menu's gear entry).
    pub(super) fn open_playback_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings_panel(PanelTab::Playback, window, cx);
    }

    /// Opens the panel on `tab`, closing the other popover and the `…` menus, and gives it the
    /// keyboard.
    pub(super) fn open_settings_panel(
        &mut self,
        tab: PanelTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.control_bar.close_overflow();
        self.close_track_popover(window, cx);
        self.close_queue_panel(window, cx);
        self.settings_panel.open = Some(tab);
        // After the click that opened it, which focuses the clicked button once its handlers ran.
        let handle = self.settings_panel.focus.clone();
        window.defer(cx, move |window, cx| handle.focus(window, cx));
        cx.notify();
    }

    /// Closes the panel, handing the keyboard back to the player if the panel had it.
    pub(super) fn close_settings_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_panel.open.take().is_none() {
            return;
        }
        if self.settings_panel.focus.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// Closes the track popover and the settings panel, for a menu about to open.
    pub(super) fn close_popovers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_track_popover(window, cx);
        self.close_settings_panel(window, cx);
    }

    /// The panel over the video's bottom right, while open. It closes once its media is gone, or
    /// with the bar it belongs to (S2.8; it keeps the bar up while open).
    pub(super) fn render_settings_panel(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let tab = self.settings_panel.open?;
        if !self.has_active_media(cx) {
            self.close_settings_panel(window, cx);
            return None;
        }
        let body = match tab {
            PanelTab::Playback => self.render_playback_tab(p, window, cx),
            _ => note("Nothing to adjust here yet.", p),
        };
        let body = div()
            .id(SharedString::from(format!("settings-tab-{tab:?}")))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(body);
        let footer = (tab != PanelTab::Playback).then(|| footer(tab, bar_panel::width(window), p));
        Some(
            bar_panel::surface("settings-panel", DESIGN_HEIGHT, window, p)
                .track_focus(&self.settings_panel.focus)
                .key_context(focus::context(&["SettingsPanel", focus::CONTROL]))
                .on_key_down(cx.listener(Self::on_settings_panel_key))
                .on_mouse_down_out(cx.listener(Self::on_mouse_down_out_of_settings))
                .child(Self::render_panel_tabs(tab, p, cx))
                .child(body)
                .children(footer),
        )
    }

    fn render_panel_tabs(shown: PanelTab, p: &Palette, cx: &mut Context<Self>) -> Div {
        let items = PanelTab::ALL.iter().map(|tab| Tab::new(Some(tab.icon()), tab.label()));
        let selected = PanelTab::ALL.iter().position(|tab| *tab == shown).unwrap_or_default();
        let bar = Tabs::new("settings-tabs", items.collect(), selected).on_select(cx.listener(
            |this, index: &usize, window, cx| {
                if let Some(tab) = PanelTab::ALL.get(*index) {
                    this.open_settings_panel(*tab, window, cx);
                }
            },
        ));
        div().flex_none().bg(p.well).child(bar)
    }

    fn on_settings_panel_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            self.close_settings_panel(window, cx);
        }
    }

    fn on_mouse_down_out_of_settings(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The gear toggles it on click; closing here would reopen it there.
        if !self.settings_panel.button.contains(event.position) {
            self.close_settings_panel(window, cx);
        }
    }

    /// Closes the popovers when the window loses focus, as menus do (so a file dialog the
    /// track popover's "+" opens closes it).
    pub(super) fn close_popovers_on_deactivate(window: &mut Window, cx: &mut Context<Self>) {
        popover::close_on_deactivate(window, cx, Self::close_popovers);
    }
}

/// The footer of the Video, Audio and Subtitles tabs. Default Preset waits for presets (S5.11);
/// Open Advanced Settings opens `tab`'s page in the Settings window.
fn footer(tab: PanelTab, width: f32, p: &Palette) -> Div {
    let inner = width - 2. * FOOTER_PADDING;
    let preset = Button::new("default-preset", "Default Preset").width(inner).disabled(true);
    let advanced = Button::new("open-advanced-settings", "Open Advanced Settings")
        .icon(icons::SETTINGS)
        .style(ButtonStyle::Ghost)
        .width(inner)
        .on_click(move |_, _, cx| crate::app::open_settings(Some(tab.category()), cx));
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(FOOTER_GAP))
        .p(px(FOOTER_PADDING))
        .bg(p.well)
        .child(preset)
        .child(advanced)
}

/// A muted line of explanation at the top of a tab.
fn note(text: &'static str, p: &Palette) -> Div {
    div().p(px(INSET)).text_size(px(NOTE_TEXT)).text_color(p.text_muted).child(text)
}
