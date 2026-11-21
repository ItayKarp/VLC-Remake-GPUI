//! The main window shell (S0.3.4): the player-style title bar and an empty state
//! that opens or accepts dropped files. Onboarding hands off to it (S0.2); the
//! player (Stage 2) and the library (Stage 4) fill the content area later.

mod diagnostics;
mod empty_state;
mod main_menu;
mod notice;
mod player_keys;
mod title_bar;

use gpui::{
    Context, DismissEvent, Entity, ExternalPaths, FocusHandle, PathPromptOptions, SharedString,
    Subscription, Window, div, prelude::*, px,
};

use crate::app;
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::keymap::actions::{CopyDebugInfo, OpenFile};
use crate::keymap::contexts;
use crate::keymap::repeat::RepeatGate;
use crate::settings;
use crate::theme;
use crate::ui::focus;
use crate::ui::menu::ContextMenu;
use crate::ui::popover::BoundsTracker;
use crate::ui::toast::{Toast, Toasts};

/// The player title bar's height in the designs (`player/screens/*`).
pub const TITLE_BAR_HEIGHT: f32 = 40.;

const APP_TITLE: &str = "VLC Media Player";

/// The main window's view: what's been opened, settings notices, and the `…` menu.
pub struct MainWindow {
    /// Files and URLs the app was launched with, opened or dropped. Played once the
    /// engine exists (Stage 1); the queue (Stage 3) takes over from this list.
    files: Vec<Media>,
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

    /// Opens what a launch asked for (S0.6.2): replaces or extends the list, and
    /// switches to fullscreen if asked.
    pub fn open(&mut self, request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) {
        if request.placement == Placement::Replace && !request.items.is_empty() {
            self.files.clear();
        }
        self.add_files(request.items, window, cx);
        if request.fullscreen && !window.is_fullscreen() {
            window.toggle_fullscreen();
        }
    }

    fn add_files(&mut self, files: Vec<Media>, window: &mut Window, cx: &mut Context<Self>) {
        app::recent::add(&files, cx);
        self.files.extend(files);
        window.set_window_title(&self.title());
        cx.notify();
    }

    fn title(&self) -> String {
        match self.files.last() {
            Some(file) => file.display_name(),
            None => APP_TITLE.to_owned(),
        }
    }

    /// Asks for files to open, and adds them once the user picks some.
    fn prompt_for_files(window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                this.add_files(paths.into_iter().map(Media::from).collect(), window, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn on_drop(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        self.add_files(paths.paths().iter().cloned().map(Media::from).collect(), window, cx);
    }

    fn toggle_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        let items = main_menu::items(window, cx);
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        self.menu = Some((menu, subscription));
        cx.notify();
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);

        Self::player_keys(focus::tab_navigation(div()), cx)
            .id("main-window")
            // The whole window is the player until Stage 2 gives it a view of its own.
            .key_context(focus::context(&[contexts::MAIN_WINDOW, contexts::PLAYER]))
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &OpenFile, window, cx| {
                Self::prompt_for_files(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CopyDebugInfo, window, cx| {
                this.copy_debug_info(window, cx);
            }))
            .relative()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .child(
                div()
                    .id("content")
                    .absolute()
                    .top(px(TITLE_BAR_HEIGHT))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .overflow_y_scroll()
                    .on_drop(cx.listener(Self::on_drop))
                    .child(self.render_empty_state(&p, cx)),
            )
            .child(self.render_title_bar(&p, window, cx))
            .child(self.toasts.clone())
            .children(self.render_menu())
    }
}
