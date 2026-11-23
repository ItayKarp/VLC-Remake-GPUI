//! The settings panel (S2.7, `controls-playback.png`, `controls-video_settings.png`): the gear
//! button's panel over the bar's right end, with Playback, Video, Audio and Subtitles tabs. It
//! changes the playing media; the Settings window holds the defaults.
//!
//! How the two relate (S5.8.4, D24): a panel value lasts the session, carried from item to item,
//! and is never saved; it wins over its default changed in Settings meanwhile until the panel
//! resets it; and Save as Default (S7.1.5) writes a tab's values into its Settings page. Speed
//! already works this way (S2.7.4), its default applying at the next launch.
//!
//! The Playback tab holds Show Subtitle Timeline on Seekbar (S6.6) and the speed (`playback.rs`).
//! Its other designed rows (Show Statistics, Reverse, Sleep Timer, Audio Visualizer) come with
//! their stories (S8.13, S7.8) and are hidden until then, as are the Audio tab's settings (Stage
//! 7). The Video tab holds its Defaults (`video_defaults.rs`, S7.2), the Crop section
//! (`crop_section.rs`) and the effect sections (`video.rs`, `effect_section.rs`, S7.1). The
//! Subtitles tab holds the subtitle defaults and the showing subtitle's synchronization
//! (`subtitles.rs`, S6.3). Those three tabs have a footer that stays put while they scroll: the
//! preset picker (S5.11), the same as at the foot of the Settings pages, opening upward; **Save
//! as Default** on a tab with effect sections, which writes the tab's session values into its
//! Settings page and is dimmed while there are none (S7.1.5, not in the design); and **Open
//! Advanced Settings**, which opens the Settings window on the tab's page (S5.2.1). Picking a
//! preset changes the defaults the session starts from, as changing them in Settings does; a
//! value the panel changed stays as it is.
//!
//! It closes like the track popover: an outside click, Escape, the gear again, opening another
//! popover or menu, the window losing focus, its media going, and fullscreen. The track popover's
//! Show Preferences opens it on the matching tab.

mod crop_section;
mod custom_ratio;
mod effect_section;
mod number_row;
mod playback;
mod subtitle_defaults;
mod subtitles;
mod sync_rules;
mod video;
mod video_defaults;

use gpui::{
    App, AppContext as _, Context, Div, Entity, FocusHandle, KeyDownEvent, MouseDownEvent,
    SharedString, Stateful, Window, div, prelude::*, px,
};

pub(super) use playback::SpeedControls;

use self::crop_section::CropControls;
use self::effect_section::EffectControls;
use self::subtitles::SubtitleControls;
use self::video_defaults::DefaultsControls;

use super::MainWindow;
use super::bar_panel;
use super::track_popover::tab_icon;
use crate::assets::icons;
use crate::effects::{self, EffectSection, VIDEO_CROP, VIDEO_DEFAULTS, VIDEO_ESSENTIAL};
use crate::engine::tracks::TrackKind;
use crate::settings::registry::Category;
use crate::settings_view::PresetPicker;
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
/// The sections' padding, heading size, and the size and space of the line under a heading.
const SECTION_PADDING: f32 = 20.;
const HEADING_TEXT: f32 = 16.;
const SECTION_NOTE_TEXT: f32 = 12.;
const SECTION_NOTE_GAP: f32 = 4.;

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

    /// The tab's effect sections (S7.1), in order.
    fn sections(self) -> &'static [EffectSection] {
        match self {
            Self::Video => &[VIDEO_CROP, VIDEO_ESSENTIAL],
            Self::Playback | Self::Audio | Self::Subtitles => &[],
        }
    }

    /// The tab's session values outside its sections (the Video tab's Defaults, S7.2).
    fn defaults(self) -> &'static [&'static str] {
        match self {
            Self::Video => VIDEO_DEFAULTS,
            Self::Playback | Self::Audio | Self::Subtitles => &[],
        }
    }

    /// Every setting the tab holds for the session: its Defaults, then its sections'.
    fn session_keys(self) -> impl Iterator<Item = &'static str> {
        let sections = self.sections().iter().flat_map(EffectSection::keys);
        self.defaults().iter().copied().chain(sections)
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
    /// The Subtitles tab's defaults and synchronization.
    subtitles: SubtitleControls,
    /// Every tab's effect sections of sliders.
    effects: Vec<EffectControls>,
    /// The Video tab's Defaults rows and Crop section.
    video_defaults: DefaultsControls,
    crop: CropControls,
    /// The footer's preset picker.
    presets: Entity<PresetPicker>,
}

impl SettingsPanel {
    /// A closed panel.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        Self {
            open: None,
            focus: cx.focus_handle(),
            button: BoundsTracker::new(),
            speed: SpeedControls::new(window, cx),
            subtitles: SubtitleControls::new(window, cx),
            effects: PanelTab::ALL
                .iter()
                .flat_map(|tab| tab.sections())
                .filter(|section| **section != VIDEO_CROP)
                .map(|section| EffectControls::new(*section, window, cx))
                .collect(),
            video_defaults: DefaultsControls::new(window, cx),
            crop: CropControls::new(window, cx),
            presets: cx.new(|cx| PresetPicker::new(window, cx)),
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
            PanelTab::Video => self.render_video_tab(p, window, cx),
            PanelTab::Subtitles => self.render_subtitles_tab(p, window, cx),
            PanelTab::Audio => note("Nothing to adjust here yet.", p),
        };
        let body = div()
            .id(SharedString::from(format!("settings-tab-{tab:?}")))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(body);
        let presets = self.settings_panel.presets.clone();
        let footer = (tab != PanelTab::Playback).then(|| footer(tab, presets, window, p, cx));
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

/// The footer of the Video, Audio and Subtitles tabs: the preset picker, Save as Default where
/// the tab holds session values, and Open Advanced Settings, which opens `tab`'s page in the
/// Settings window.
fn footer(
    tab: PanelTab,
    presets: Entity<PresetPicker>,
    window: &Window,
    p: &Palette,
    cx: &App,
) -> Div {
    let inner = bar_panel::width(window) - 2. * FOOTER_PADDING;
    let save = tab.session_keys().next().is_some().then(|| save_as_default(tab, inner, cx));
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
        .child(presets)
        .children(save)
        .child(advanced)
}

/// Save as Default (S7.1.5): writes `tab`'s session values into its Settings page; dimmed while
/// the panel changed nothing there.
fn save_as_default(tab: PanelTab, width: f32, cx: &App) -> Button {
    let changed = tab.session_keys().any(|key| effects::is_changed(cx, key));
    let page = tab.label();
    Button::new("save-as-default", "Save as Default")
        .style(ButtonStyle::Ghost)
        .width(width)
        .disabled(!changed)
        .tooltip(format!("Keep these as Settings → {page}, for later sessions"), None::<&str>)
        .on_click(move |_, _, cx| effects::save_as_default(cx, tab.session_keys()))
}

/// A section: its heading, the line saying what its rows change, then the rows added to it.
fn section(heading: &'static str, note: impl Into<SharedString>, p: &Palette) -> Div {
    let title = div().text_size(px(HEADING_TEXT)).text_color(p.text).child(heading);
    let note = div()
        .pt(px(SECTION_NOTE_GAP))
        .text_size(px(SECTION_NOTE_TEXT))
        .text_color(p.text_muted)
        .child(note.into());
    div().flex().flex_col().p(px(SECTION_PADDING)).child(title).child(note)
}

/// A muted line of explanation at the top of a tab.
fn note(text: &'static str, p: &Palette) -> Div {
    div().p(px(INSET)).text_size(px(NOTE_TEXT)).text_color(p.text_muted).child(text)
}
