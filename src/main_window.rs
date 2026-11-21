//! The main window shell (S0.3.4): the player-style title bar, the control bar (S2.2), and an
//! empty state that opens or accepts dropped files, replaced by the video while media plays
//! (S1.4). Onboarding hands off to it (S0.2); the player (Stage 2) and the library
//! (Stage 4) fill the content area later.

mod audio;
mod control_bar;
mod diagnostics;
mod empty_state;
mod flash;
mod fullscreen;
mod history;
mod main_menu;
mod notice;
mod now_playing;
mod opening;
mod osd;
mod player_keys;
mod seek_bar;
mod seeking;
#[cfg(test)]
mod tests;
mod title_bar;
mod tracks;
mod transport;
mod viewport;
mod volume_slider;

use gpui::{
    App, Context, DismissEvent, Div, Entity, FocusHandle, ScrollEvent, SharedString, Subscription,
    Window, div, prelude::*, px,
};

use crate::app;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::engine::player::Player;
use crate::keymap::actions::{CopyDebugInfo, OpenFile, OpenFolder, OpenFromClipboard};
use crate::keymap::contexts;
use crate::keymap::repeat::RepeatGate;
use crate::power::DisplaySleep;
use crate::settings;
use crate::theme;
use crate::ui::focus;
use crate::ui::popover::BoundsTracker;
use crate::ui::toast::{Toast, Toasts};
use crate::ui::{drop_target, menu::ContextMenu};
use crate::video::VideoView;

/// The player title bar's height in the designs (`player/screens/*`).
pub const TITLE_BAR_HEIGHT: f32 = 40.;

/// The main window's view: what's been opened, settings notices, and the `…` menu.
pub struct MainWindow {
    /// What was opened, in play order; the queue (Stage 3) takes over from this list.
    files: Vec<Media>,
    /// The index in `files` of what the player has.
    current: Option<usize>,
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
    /// The message over the video (S2.5.7).
    osd: osd::Osd,
    /// The title last given to the OS window, so it's only set when it changes (S2.1.5).
    os_title: String,
}

impl MainWindow {
    /// Builds the window's view, opening `request` and showing any pending settings
    /// notices.
    pub fn new(request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        // The title bar follows Settings → Interface.
        cx.observe_global::<settings::SettingsStore>(|_, cx| cx.notify()).detach();
        // Swap the Maximize/Restore icon when the window is maximized or restored.
        cx.observe_window_bounds(window, |_, _, cx| cx.notify()).detach();
        app::save_geometry_on_change(window, cx, |s| &mut s.local.window.main);
        Self::release_keys_on_blur(window, cx);
        crate::diagnostics::system::remember_gpu(window);

        let open_button = cx.focus_handle().tab_stop(true).tab_index(0);
        open_button.focus(window, cx);

        let mut this = Self {
            files: Vec::new(),
            current: None,
            jobs: Vec::new(),
            next_job: 0,
            player: None,
            video: None,
            audio: audio::AudioWatch::default(),
            tracks: tracks::TrackWatch::default(),
            history: history::HistoryWatch::default(),
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
            os_title: String::new(),
        };
        this.set_up_history(cx);
        this.open(request, window, cx);
        this.show_pending_crash(cx);
        this
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

    /// Between the title bar and the control bar (the whole window in fullscreen): the video
    /// while media is loaded, else the empty state.
    fn render_content(&self, p: &theme::Palette, fullscreen: bool, cx: &mut Context<Self>) -> Div {
        let (top, bottom) =
            if fullscreen { (0., 0.) } else { (TITLE_BAR_HEIGHT, control_bar::CONTROL_BAR_HEIGHT) };
        let area = div().absolute().top(px(top)).left_0().right_0().bottom(px(bottom));
        match self.showing_video(cx) {
            Some(video) => area
                .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                    this.scroll_on_video(event, window, cx);
                }))
                .child(viewport::framed(video, p, fullscreen))
                .children(self.render_osd(p)),
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

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_os_title(window, cx);
        self.release_previews(window);
        let p = theme::palette(window, cx);
        // Fullscreen is the video alone: no title or control bar (S2.8 brings auto-hiding
        // controls).
        let fullscreen = window.is_fullscreen();

        Self::history_actions(Self::player_keys(focus::tab_navigation(div()), cx), cx)
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
            .child(self.render_content(&p, fullscreen, cx))
            .children((!fullscreen).then(|| self.render_title_bar(&p, window, cx)))
            .children((!fullscreen).then(|| self.render_control_bar(&p, window, cx)))
            .children((!fullscreen).then(|| self.render_seek_card(&p, window, cx)).flatten())
            .child(self.toasts.clone())
            .children(self.render_menu())
            .children(self.render_overflow_menu())
            .child(drop_target::highlight(&p))
    }
}
