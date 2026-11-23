//! The main window shell (S0.3.4): the player-style title bar, the control bar (S2.2), and an
//! empty state that opens or accepts dropped files, replaced by the video while media plays
//! (S1.4). The bars hide by themselves, in Hide UI mode and in fullscreen (S2.8). The `…` menu
//! and the video's right-click menu hold the commands, and About VLC opens over it (S2.9). Clicks
//! and scrolling on the video, on-screen messages and the Keyboard Shortcuts dialog are S2.10.
//! The List button opens the queue (S3.2), and the queue outlives the app (S3.4). Playlists are
//! saved from the New Playlist dialog and the video menu (S3.5). Properties & Metadata shows what
//! the library read from the playing file (S4.3.5). The sidebar toggle and L show the library
//! sidebar over the content (S4.5), whose pop-out switches to full library mode with the
//! miniplayer (S4.10). Audio-only media plays in music mode, its cover in the video's place and
//! a control bar of its own (S8.1). Shift+S saves the picture on screen (S8.4), and Shift+R
//! records what plays (S8.6). Picture in
//! Picture pops the player out into a small window above the others (S8.5). The bars show what
//! the player layout says, which Customize Player edits (S8.7).
//! Onboarding hands off to it (S0.2).

mod about;
mod add_subtitles;
mod audio;
mod audio_delay;
mod audio_tracks;
mod auto_hide;
mod bar_panel;
mod control_bar;
mod customize;
mod diagnostics;
mod empty_state;
mod flash;
mod frame_step;
mod fullscreen;
mod hdr_note;
mod hide_ui;
mod history;
mod library_panel;
mod library_status;
mod linked_subtitles;
mod main_menu;
mod miniplayer;
mod music_mode;
mod network_stream;
mod new_playlist;
mod notice;
mod now_playing;
mod now_playing_notice;
mod opening;
mod osd;
mod picture;
mod pip;
mod play_order;
mod player_keys;
mod properties;
mod queue_panel;
mod queue_play;
mod recording;
mod saved_playlists;
mod seek_bar;
mod seeking;
mod session;
mod settings_panel;
mod shortcuts_dialog;
mod snapshot;
mod speed;
pub(crate) mod subtitle_style;
mod subtitle_sync;
#[cfg(test)]
mod tests;
mod title_bar;
mod track_popover;
mod tracks;
mod transport;
mod video_menu;
mod video_mouse;
mod video_window;
mod viewport;
mod volume_slider;
mod wheel;

use gpui::{
    AnyElement, App, Context, Div, Entity, FocusHandle, ScrollEvent, SharedString, Subscription,
    Window, div, prelude::*, px,
};

use self::auto_hide::rules::Chrome;
use crate::app;
use crate::app::request::OpenRequest;
use crate::engine::player::Player;
use crate::keymap::actions::CopyDebugInfo;
use crate::keymap::contexts;
use crate::keymap::repeat::RepeatGate;
use crate::power::DisplaySleep;
use crate::queue::{Queue, Skips};
use crate::settings;
use crate::theme;
use crate::ui::focus;
use crate::ui::modal::ModalLayer;
use crate::ui::popover::BoundsTracker;
use crate::ui::toast::{Toast, Toasts};
use crate::ui::{drop_target, menu::ContextMenu};
use crate::video::VideoView;

pub use self::now_playing_notice::NOW_PLAYING_TAG;
pub use self::pip::{PipCommand, PipView};
pub use self::shortcuts_dialog::edit_keyboard_shortcuts;

/// The player title bar's height in the designs (`player/screens/*`).
pub const TITLE_BAR_HEIGHT: f32 = 40.;

/// The main window's view: what's been opened, settings notices, and the `…` menu.
pub struct MainWindow {
    /// What plays, in order, and which item the player has (S3.1).
    queue: Queue,
    /// Failed items skipped in a row (S3.1.4).
    skips: Skips,
    /// Opens still resolving (scanning folders, expanding playlists).
    jobs: opening::Jobs,
    /// The engine, once something was opened, and its event subscription.
    player: Option<(Entity<Player>, Subscription)>,
    /// The video area, created with the player.
    video: Option<Entity<VideoView>>,
    /// The audio settings subscription and what was said about devices (S1.5).
    audio: audio::AudioWatch,
    /// The track and media settings subscription (S1.6), and the picture's adjustments (S7.1).
    tracks: tracks::TrackWatch,
    /// Audio files waiting for their media to open (S4.8.1).
    audio_tracks: audio_tracks::PendingAudio,
    /// The playback being recorded and the resume offer (S1.7).
    history: history::HistoryWatch,
    /// Saving the session for the next launch (S3.4).
    session: session::SessionWatch,
    /// Keeps the display awake while video plays where it shows (S1.4.3).
    display_sleep: DisplaySleep,
    _visibility: Subscription,
    /// Settings notices (corrupt file recovered, changes not saved, …).
    notices: Vec<SharedString>,
    focus_handle: FocusHandle,
    open_button: FocusHandle,
    /// The `…` menu, while it's open.
    menu: Option<(Entity<ContextMenu>, Subscription)>,
    menu_button: BoundsTracker,
    /// Player commands whose key may still be held (S0.5.6).
    repeat: RepeatGate,
    /// Transient messages at the bottom right.
    toasts: Entity<Toasts>,
    /// The control bar's layout and overflow menu (S2.2).
    control_bar: control_bar::ControlBar,
    /// The seek bar's drag, hover and previews (S2.3).
    seek_bar: seek_bar::SeekBar,
    /// The volume wedge's drag, wheel and value bubble (S2.5).
    volume_slider: volume_slider::VolumeSlider,
    /// The message over the video (S2.5.7, S2.10.2).
    osd: osd::Osd,
    /// The recording being made (S8.6).
    recording: recording::RecordingWatch,
    /// Whether frame steps are taking a while (S2.10.3).
    frame_step: frame_step::FrameStepWait,
    /// Clicks and sideways scrolling on the video (S2.10.1).
    video_mouse: video_mouse::VideoMouse,
    /// The Video | Audio | Subtitles popover (S2.6).
    track_popover: track_popover::TrackPopover,
    /// The gear's Playback | Video | Audio | Subtitles panel (S2.7).
    settings_panel: settings_panel::SettingsPanel,
    /// The List button's Queue | Bookmarks | Chapters panel (S3.2).
    queue_panel: queue_panel::QueuePanel,
    /// The library sidebar over the player (S4.5), and full library mode (S4.10).
    library_panel: library_panel::LibraryPanel,
    /// Full library mode's miniplayer card (S4.10.3).
    miniplayer: miniplayer::Miniplayer,
    /// Music mode for audio-only media, and its cover (S8.1).
    music: music_mode::MusicMode,
    /// The title last given to the OS window, so it's only set when it changes (S2.1.5).
    os_title: String,
    /// When the bars hide, Hide UI mode and the video's right-click menu (S2.8).
    auto_hide: auto_hide::AutoHide,
    /// Dialogs over the window (About VLC, S2.9.6).
    modals: Entity<ModalLayer>,
    /// Settings → Interface's Window section and notification for this window (S5.4).
    windowing: video_window::Windowing,
    /// Customize Player, while it's open (S8.7).
    customizing: Option<customize::Customizing>,
}

impl MainWindow {
    /// Builds the window's view, opening `request` and showing any pending settings
    /// notices.
    #[allow(clippy::too_many_lines, reason = "one struct literal, a line per field")]
    pub fn new(request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::follow_window(window, cx);
        let mut this = Self {
            queue: Queue::default(),
            skips: Skips::default(),
            jobs: opening::Jobs::default(),
            player: None,
            video: None,
            audio: audio::AudioWatch::default(),
            tracks: tracks::TrackWatch::default(),
            audio_tracks: audio_tracks::PendingAudio::default(),
            history: history::HistoryWatch::default(),
            session: session::SessionWatch::default(),
            display_sleep: DisplaySleep::default(),
            _visibility: Self::follow_visibility(window, cx),
            notices: Self::take_notices(cx),
            focus_handle: cx.focus_handle(),
            open_button: cx.focus_handle().tab_stop(true).tab_index(0),
            menu: None,
            menu_button: BoundsTracker::new(),
            repeat: RepeatGate::default(),
            toasts: cx.new(|_| Toasts::new()),
            control_bar: control_bar::ControlBar::default(),
            seek_bar: seek_bar::SeekBar::new(cx),
            volume_slider: volume_slider::VolumeSlider::new(cx),
            osd: osd::Osd::default(),
            recording: recording::RecordingWatch::default(),
            frame_step: frame_step::FrameStepWait::default(),
            video_mouse: video_mouse::VideoMouse::default(),
            track_popover: track_popover::TrackPopover::new(cx),
            settings_panel: settings_panel::SettingsPanel::new(window, cx),
            queue_panel: queue_panel::QueuePanel::new(cx),
            library_panel: library_panel::LibraryPanel::new(cx),
            miniplayer: miniplayer::Miniplayer::default(),
            music: music_mode::MusicMode::default(),
            os_title: String::new(),
            auto_hide: auto_hide::AutoHide::default(),
            modals: cx.new(|_| ModalLayer::new()),
            windowing: video_window::Windowing::default(),
            customizing: None,
        };
        this.start(request, window, cx);
        this
    }

    /// Gives the Open button the keyboard, starts recording history and the session, brings the
    /// last session back, and opens `request`.
    fn start(&mut self, request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) {
        self.open_button.focus(window, cx);
        self.control_bar.sync_layout(cx);
        self.plan_compact_start(cx);
        self.set_up_history(cx);
        self.set_up_session(cx);
        self.restore_session(&request, window, cx);
        self.open(request, window, cx);
        self.show_pending_crash(cx);
    }

    /// Follows the window: the theme, redraws, its geometry, focus, the GPU it renders with, and
    /// whether the library is used.
    fn follow_window(window: &mut Window, cx: &mut Context<Self>) {
        theme::observe(window, cx);
        Self::redraw_on_changes(window, cx);
        app::save_geometry_on_change(window, cx, |s| &mut s.local.window.main);
        Self::release_keys_on_blur(window, cx);
        Self::close_popovers_on_deactivate(window, cx);
        Self::follow_library_switch(window, cx);
        crate::diagnostics::system::remember_gpu(window);
    }

    /// Redraws when Settings → Interface changes (the title bar, the controls' hiding), and when
    /// the window is maximized, restored or goes fullscreen (the Maximize/Restore icon, the
    /// layout).
    fn redraw_on_changes(window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_global_in::<settings::SettingsStore>(window, |this, window, cx| {
            this.sync_video_window(window, cx);
            this.control_bar.sync_layout(cx);
            cx.notify();
        })
        .detach();
        cx.observe_global::<crate::library::Library>(|_, cx| cx.notify()).detach();
        cx.observe_window_bounds(window, |_, _, cx| cx.notify()).detach();
    }

    /// The settings notices waiting to be shown, as text.
    fn take_notices(cx: &mut Context<Self>) -> Vec<SharedString> {
        let notices = settings::take_notices(cx).into_iter();
        notices.map(|notice| notice::text(&notice).into()).collect()
    }

    /// Shows `toast` at the bottom right of the window.
    pub fn show_toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toasts.update(cx, |toasts, cx| toasts.push(toast, cx));
    }

    /// Names the OS window (taskbar, Alt+Tab, Mission Control) like the title bar (S2.1.5).
    fn sync_os_title(&mut self, window: &mut Window, cx: &App) {
        let title = self.title(cx);
        if title != self.os_title {
            window.set_window_title(&title);
            self.os_title = title;
        }
    }

    /// Between the title bar and the control bar (the whole window when the bars float or
    /// hide, S2.8): the video while media is loaded, or for audio-only media its cover (S8.1),
    /// else the empty state.
    fn render_content(
        &self,
        p: &theme::Palette,
        chrome: Chrome,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let (top, bottom) = viewport::content_insets(chrome, self.control_bar_height());
        let area = div().absolute().top(px(top)).left_0().right_0().bottom(px(bottom));
        match self.showing_video(cx) {
            Some(video) => {
                let shown = self.render_cover(chrome, window, cx);
                let shown = shown.unwrap_or_else(|| viewport::framed(video, p, chrome));
                area.on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                    this.scroll_on_video(event, window, cx);
                }))
                .child(self.video_area(shown, window, cx))
                .children(self.render_osd(p, cx))
                .children(self.render_recording_badge(p, cx))
            }
            None => area.child(
                div()
                    .id("content")
                    .size_full()
                    .overflow_y_scroll()
                    .child(self.render_empty_state(p, cx)),
            ),
        }
    }
}

impl MainWindow {
    /// The menus that are open: `…`, the bar's overflow, the queue's sort, the video's and the
    /// library sidebar's.
    fn render_menus(&self) -> impl Iterator<Item = AnyElement> {
        let menus = [
            self.render_menu(),
            self.render_overflow_menu(),
            self.render_sort_menu(),
            self.render_video_menu(),
        ];
        menus.into_iter().flatten().chain(self.render_library_menus())
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_os_title(window, cx);
        self.windowing.prefs.sync(window, cx);
        self.release_previews(window);
        self.sync_music_mode(window, cx);
        let p = theme::palette(window, cx);
        let chrome = self.sync_chrome(window, cx);
        self.fit_library(window, cx);
        self.place_miniplayer(chrome.title_bar, window, cx);
        let full = self.library_panel.is_full();

        let root = Self::player_keys(focus::tab_navigation(div()), cx);
        let root = Self::properties_actions(Self::history_actions(root, cx), cx);
        let root = Self::menu_actions(Self::playlist_actions(root, cx), cx);
        Self::open_actions(Self::network_actions(root, cx), cx)
            .id("main-window")
            // The whole window is the player until Stage 2 gives it a view of its own.
            .key_context(focus::context(&[contexts::MAIN_WINDOW, contexts::PLAYER]))
            .track_focus(&self.focus_handle)
            // Drops land anywhere in the window, over menus and toasts too (S1.3.2).
            .group(drop_target::GROUP)
            .on_drop(cx.listener(Self::on_drop))
            .on_action(cx.listener(|this, _: &CopyDebugInfo, window, cx| {
                this.copy_debug_info(window, cx);
            }))
            .relative()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            // Full library mode takes the content area; what plays shows in the miniplayer.
            .children((!full).then(|| self.render_content(&p, chrome, window, cx)))
            .children(self.render_library(&p, chrome, cx))
            .children(chrome.title_bar.then(|| self.render_title_bar(&p, window, cx)))
            .children(chrome.controls.then(|| self.render_control_bar(&p, window, cx)))
            .children(self.render_customize_palette(chrome.title_bar, &p, cx))
            .children(self.render_track_popover(&p, window, cx))
            .children(self.render_settings_panel(&p, window, cx))
            .children((!full).then(|| self.render_queue_panel(&p, window, cx)).flatten())
            .children(self.render_miniplayer(&p, cx))
            .children(chrome.controls.then(|| self.render_seek_card(&p, window, cx)).flatten())
            .child(Self::pointer_watcher(cx))
            .child(self.toasts.clone())
            .children(self.render_menus())
            .child(self.modals.clone())
            .child(drop_target::highlight(&p))
    }
}
