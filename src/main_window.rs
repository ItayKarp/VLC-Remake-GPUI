//! The main window shell (S0.3.4): the player-style title bar, the control bar (S2.2), and an
//! empty state that opens or accepts dropped files, replaced by the video while media plays
//! (S1.4). The bars hide by themselves, in Hide UI mode and in fullscreen (S2.8). The `…` menu
//! and the video's right-click menu hold the commands, and About VLC opens over it (S2.9). Clicks
//! and scrolling on the video, on-screen messages and the Keyboard Shortcuts dialog are S2.10.
//! The List button opens the queue (S3.2), and the queue outlives the app (S3.4). Playlists are
//! saved from the New Playlist dialog and the video menu (S3.5). Properties & Metadata shows what
//! the library read from the playing file (S4.3.5).
//! Onboarding hands off to it (S0.2); the library (Stage 4) fills the content area later.

mod about;
mod audio;
mod auto_hide;
mod bar_panel;
mod control_bar;
mod diagnostics;
mod empty_state;
mod flash;
mod frame_step;
mod fullscreen;
mod hide_ui;
mod history;
mod library_status;
mod main_menu;
mod new_playlist;
mod notice;
mod now_playing;
mod opening;
mod osd;
mod play_order;
mod player_keys;
mod properties;
mod queue_panel;
mod queue_play;
mod saved_playlists;
mod seek_bar;
mod seeking;
mod session;
mod settings_panel;
mod shortcuts_dialog;
mod speed;
#[cfg(test)]
mod tests;
mod title_bar;
mod track_popover;
mod tracks;
mod transport;
mod video_menu;
mod video_mouse;
mod viewport;
mod volume_slider;
mod wheel;

use gpui::{
    AnyElement, App, Context, DismissEvent, Div, Entity, FocusHandle, ScrollEvent, SharedString,
    Subscription, Window, div, prelude::*, px,
};

use self::auto_hide::rules::Layout;
use crate::app;
use crate::app::request::OpenRequest;
use crate::engine::player::Player;
use crate::keymap::actions::{CopyDebugInfo, OpenFile, OpenFolder, OpenFromClipboard};
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

/// The player title bar's height in the designs (`player/screens/*`).
pub const TITLE_BAR_HEIGHT: f32 = 40.;

/// The main window's view: what's been opened, settings notices, and the `…` menu.
pub struct MainWindow {
    /// What plays, in order, and which item the player has (S3.1).
    queue: Queue,
    /// Failed items skipped in a row (S3.1.4).
    skips: Skips,
    /// Opens still resolving (scanning folders, expanding playlists).
    jobs: Vec<opening::Job>,
    /// The id the next job gets.
    next_job: u64,
    /// The engine, once something was opened, and its event subscription.
    player: Option<(Entity<Player>, Subscription)>,
    /// The video area, created with the player.
    video: Option<Entity<VideoView>>,
    /// The audio settings subscription and what was said about devices (S1.5).
    audio: audio::AudioWatch,
    /// The track settings subscription and the last track toast (S1.6).
    tracks: tracks::TrackWatch,
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
    /// The title last given to the OS window, so it's only set when it changes (S2.1.5).
    os_title: String,
    /// When the bars hide, Hide UI mode and the video's right-click menu (S2.8).
    auto_hide: auto_hide::AutoHide,
    /// Dialogs over the window (About VLC, S2.9.6).
    modals: Entity<ModalLayer>,
}

impl MainWindow {
    /// Builds the window's view, opening `request` and showing any pending settings
    /// notices.
    pub fn new(request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::follow_window(window, cx);
        let open_button = cx.focus_handle().tab_stop(true).tab_index(0);
        open_button.focus(window, cx);

        let mut this = Self {
            queue: Queue::default(),
            skips: Skips::default(),
            jobs: Vec::new(),
            next_job: 0,
            player: None,
            video: None,
            audio: audio::AudioWatch::default(),
            tracks: tracks::TrackWatch::default(),
            history: history::HistoryWatch::default(),
            session: session::SessionWatch::default(),
            display_sleep: DisplaySleep::default(),
            _visibility: Self::follow_visibility(window, cx),
            notices: Self::take_notices(cx),
            focus_handle: cx.focus_handle(),
            open_button,
            menu: None,
            menu_button: BoundsTracker::new(),
            repeat: RepeatGate::default(),
            toasts: cx.new(|_| Toasts::new()),
            control_bar: control_bar::ControlBar::default(),
            seek_bar: seek_bar::SeekBar::new(cx),
            volume_slider: volume_slider::VolumeSlider::new(cx),
            osd: osd::Osd::default(),
            frame_step: frame_step::FrameStepWait::default(),
            video_mouse: video_mouse::VideoMouse::default(),
            track_popover: track_popover::TrackPopover::new(cx),
            settings_panel: settings_panel::SettingsPanel::new(window, cx),
            queue_panel: queue_panel::QueuePanel::new(cx),
            os_title: String::new(),
            auto_hide: auto_hide::AutoHide::default(),
            modals: cx.new(|_| ModalLayer::new()),
        };
        this.start(request, window, cx);
        this
    }

    /// Starts recording history and the session, brings the last session back, and opens
    /// `request`.
    fn start(&mut self, request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) {
        self.set_up_history(cx);
        self.set_up_session(cx);
        self.restore_session(&request, window, cx);
        self.open(request, window, cx);
        self.show_pending_crash(cx);
    }

    /// Follows the window: the theme, redraws, its geometry, focus and the GPU it renders with.
    fn follow_window(window: &mut Window, cx: &mut Context<Self>) {
        theme::observe(window, cx);
        Self::redraw_on_changes(window, cx);
        app::save_geometry_on_change(window, cx, |s| &mut s.local.window.main);
        Self::release_keys_on_blur(window, cx);
        Self::close_popovers_on_deactivate(window, cx);
        crate::diagnostics::system::remember_gpu(window);
    }

    /// Redraws when Settings → Interface changes (the title bar, the controls' hiding), and when
    /// the window is maximized, restored or goes fullscreen (the Maximize/Restore icon, the
    /// layout).
    fn redraw_on_changes(window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_global::<settings::SettingsStore>(|_, cx| cx.notify()).detach();
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

    fn toggle_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        self.close_popovers(window, cx);
        let items = main_menu::items(&cx.entity().downgrade(), window, cx);
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        self.menu = Some((menu, subscription));
        cx.notify();
    }
}

impl MainWindow {
    /// Names the OS window (taskbar, Alt+Tab, Mission Control) like the title bar (S2.1.5).
    fn sync_os_title(&mut self, window: &mut Window, cx: &App) {
        let title = self.title(cx);
        if title != self.os_title {
            window.set_window_title(&title);
            self.os_title = title;
        }
    }

    /// Between the title bar and the control bar (the whole window when the bars float or
    /// hide, S2.8): the video while media is loaded, else the empty state.
    fn render_content(
        &self,
        p: &theme::Palette,
        layout: Layout,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let (top, bottom) = match layout {
            Layout::Docked => (TITLE_BAR_HEIGHT, control_bar::CONTROL_BAR_HEIGHT),
            Layout::Floating | Layout::Fullscreen => (0., 0.),
        };
        let area = div().absolute().top(px(top)).left_0().right_0().bottom(px(bottom));
        match self.showing_video(cx) {
            Some(video) => area
                .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                    this.scroll_on_video(event, window, cx);
                }))
                .child(self.video_area(viewport::framed(video, p, layout), window, cx))
                .children(self.render_osd(p, cx)),
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
    /// The menus that are open: `…`, the bar's overflow, the queue's sort and the video's.
    fn render_menus(&self) -> impl Iterator<Item = AnyElement> {
        let menus = [
            self.render_menu(),
            self.render_overflow_menu(),
            self.render_sort_menu(),
            self.render_video_menu(),
        ];
        menus.into_iter().flatten()
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_os_title(window, cx);
        self.release_previews(window);
        let p = theme::palette(window, cx);
        let chrome = self.sync_chrome(window, cx);

        let root = Self::player_keys(focus::tab_navigation(div()), cx);
        let root = Self::properties_actions(Self::history_actions(root, cx), cx);
        Self::menu_actions(Self::playlist_actions(root, cx), cx)
            .id("main-window")
            // The whole window is the player until Stage 2 gives it a view of its own.
            .key_context(focus::context(&[contexts::MAIN_WINDOW, contexts::PLAYER]))
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &OpenFile, window, cx| {
                Self::prompt_for_files(window, cx);
            }))
            .on_action(cx.listener(|_, _: &OpenFolder, window, cx| {
                Self::prompt_for_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFromClipboard, window, cx| {
                this.paste(window, cx);
            }))
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
            .child(self.render_content(&p, chrome.layout, window, cx))
            .children(chrome.title_bar.then(|| self.render_title_bar(&p, window, cx)))
            .children(chrome.controls.then(|| self.render_control_bar(&p, window, cx)))
            .children(self.render_track_popover(&p, window, cx))
            .children(self.render_settings_panel(&p, window, cx))
            .children(self.render_queue_panel(&p, window, cx))
            .children(chrome.controls.then(|| self.render_seek_card(&p, window, cx)).flatten())
            .child(Self::pointer_watcher(cx))
            .child(self.toasts.clone())
            .children(self.render_menus())
            .child(self.modals.clone())
            .child(drop_target::highlight(&p))
    }
}
