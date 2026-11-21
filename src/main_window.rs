//! The main window shell (S0.3.4): the player-style title bar and an empty state
//! that opens or accepts dropped files, replaced by the video while media plays
//! (S1.4). Onboarding hands off to it (S0.2); the player (Stage 2) and the library
//! (Stage 4) fill the content area later.

mod diagnostics;
mod empty_state;
mod fullscreen;
mod main_menu;
mod notice;
mod now_playing;
mod opening;
mod player_keys;
#[cfg(test)]
mod tests;
mod title_bar;

use gpui::{
    Context, DismissEvent, Div, Entity, FocusHandle, SharedString, Subscription, Window, div,
    prelude::*, px,
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

const APP_TITLE: &str = "VLC Media Player";

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
}

impl MainWindow {
    /// Builds the window's view, opening `request` and showing any pending settings
    /// notices.
    pub fn new(request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        // Swap the Maximize/Restore icon when the window is maximized or restored.
        cx.observe_window_bounds(window, |_, _, cx| cx.notify()).detach();
        app::save_geometry_on_change(window, cx, |s| &mut s.local.window.main);
        Self::release_keys_on_blur(window, cx);
        crate::diagnostics::system::remember_gpu(window);

        let open_button = cx.focus_handle().tab_stop(true).tab_index(0);
        open_button.focus(window, cx);

        let notices = settings::take_notices(cx)
            .into_iter()
            .map(|notice| notice::text(&notice).into())
            .collect();
        let mut this = Self {
            files: Vec::new(),
            current: None,
            jobs: Vec::new(),
            next_job: 0,
            player: None,
            video: None,
            display_sleep: DisplaySleep::default(),
            _visibility: Self::follow_visibility(window, cx),
            notices,
            focus_handle: cx.focus_handle(),
            open_button,
            menu: None,
            menu_button: BoundsTracker::new(),
            repeat: RepeatGate::default(),
            toasts: cx.new(|_| Toasts::new()),
        };
        this.open(request, window, cx);
        this.show_pending_crash(cx);
        this
    }

    /// Shows `toast` at the bottom right of the window.
    pub fn show_toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toasts.update(cx, |toasts, cx| toasts.push(toast, cx));
    }

    /// The title bar's text: what's playing, or the app's name.
    fn title(&self) -> String {
        match self.current.and_then(|index| self.files.get(index)) {
            Some(file) => file.display_name(),
            None => APP_TITLE.to_owned(),
        }
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
    /// Below the title bar (the whole window in fullscreen): the video while media is loaded,
    /// else the empty state.
    fn render_content(&self, p: &theme::Palette, fullscreen: bool, cx: &mut Context<Self>) -> Div {
        let top = if fullscreen { 0. } else { TITLE_BAR_HEIGHT };
        let area = div().absolute().top(px(top)).left_0().right_0().bottom_0();
        match self.showing_video(cx) {
            Some(video) => area.child(video),
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
        let p = theme::palette(window, cx);
        // Fullscreen is the video alone: no title bar (S2.8 brings auto-hiding controls).
        let fullscreen = window.is_fullscreen();

        Self::player_keys(focus::tab_navigation(div()), cx)
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
            .child(self.toasts.clone())
            .children(self.render_menu())
            .child(drop_target::highlight(&p))
    }
}
