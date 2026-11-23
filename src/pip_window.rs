//! The Picture in Picture window (S8.5): the miniplayer popped out of the main window into a
//! small window of its own above the other windows, with the docked card's hover controls (✕,
//! pop-in, Previous, Play/Pause and Next) and a progress bar that seeks.
//!
//! The main window opens and closes it (`main_window/pip.rs`), lends it its [`VideoView`] and
//! does what its controls ask ([`PipCommand`]). It has no frame: dragging it anywhere moves it,
//! its edges resize it with the video's aspect ratio kept, and a double click goes back to the
//! player. Keys go to the main window, so the player's keys work from here too. Where it was
//! left is remembered per display ([`placement`]); a window left on a display that goes away
//! comes back onto one that is there.

pub mod placement;

use gpui::{
    AnyElement, AnyWindowHandle, App, Context, Div, Entity, FocusHandle, Hsla, KeyDownEvent,
    KeyUpEvent, MouseButton, MouseDownEvent, Stateful, Subscription, WeakEntity, Window,
    WindowControlArea, WindowHandle, div, prelude::*, px, relative, size,
};

use crate::app::geometry::Rect;
use crate::assets::icons;
use crate::db::thumbnails::SizeClass;
use crate::keymap::actions::PictureInPicture;
use crate::main_window::{MainWindow, PipCommand, PipView};
use crate::native_window;
use crate::settings;
use crate::theme;
use crate::ui::player_overlay::{self, Handlers, OnClick, WindowButton};
use crate::ui::popover::BoundsTracker;
use crate::ui::thumbnail::{Placeholder, ThumbnailTile};
use crate::video::VideoView;
use crate::window_chrome;

/// What hovering the window lights up together.
const GROUP: &str = "pip-window";
/// The progress bar's height and its distance from the window's edges.
const PROGRESS_HEIGHT: f32 = 4.;
const PROGRESS_INSET: f32 = 12.;
/// How far the height may be off the aspect ratio before the window is resized to it.
const ASPECT_SLACK: f32 = 1.;

/// The Picture in Picture window.
pub struct PipWindow {
    video: Entity<VideoView>,
    /// The main window, which plays the media and gets the keys, and its view.
    main: WindowHandle<MainWindow>,
    main_view: WeakEntity<MainWindow>,
    focus_handle: FocusHandle,
    /// The progress bar's bounds, for seeking where it is clicked.
    progress: BoundsTracker,
    /// The picture's width over its height.
    aspect: f32,
    /// The OS keeps the aspect ratio while the window is resized; else this view does after.
    os_keeps_aspect: bool,
    _subscriptions: Vec<Subscription>,
}

impl PipWindow {
    /// The window showing `video` at `aspect`, run by `main`, whose view is `main_view`.
    pub fn new(
        video: Entity<VideoView>,
        (main, main_view): (WindowHandle<MainWindow>, WeakEntity<MainWindow>),
        aspect: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        native_window::make_floating(window);
        let os_keeps_aspect = native_window::lock_aspect_ratio(window, (aspect, 1.));
        video.update(cx, |video, cx| video.arrive(window, cx));
        let mut subscriptions = vec![cx.observe_window_bounds(window, Self::bounds_changed)];
        if let Some(entity) = main_view.upgrade() {
            subscriptions.push(cx.observe(&entity, |_, _, cx| cx.notify()));
        }
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            video,
            main,
            main_view,
            focus_handle,
            progress: BoundsTracker::new(),
            aspect,
            os_keeps_aspect,
            _subscriptions: subscriptions,
        }
    }

    /// The video's aspect ratio changed (the next item, a new track): the window takes it,
    /// keeping its width and top-left corner.
    pub fn set_aspect(&mut self, aspect: f32, window: &mut Window, cx: &mut Context<Self>) {
        if (aspect - self.aspect).abs() < f32::EPSILON {
            return;
        }
        self.aspect = aspect;
        self.os_keeps_aspect = native_window::lock_aspect_ratio(window, (aspect, 1.));
        let now = Rect::from_gpui(window.bounds());
        let area = work_area(window, cx);
        let (w, h) = placement::fitted_size(now.w, aspect, &area);
        window.resize(size(px(w), px(h)));
        let placed = crate::window_prefs::kept_inside(Rect::new(now.x, now.y, w, h), &area);
        // AppKit keeps a resized window's bottom-left corner; the top-left one stays here.
        native_window::move_to(window, placed.to_gpui().origin, cx);
    }

    /// Gives the video back: frees what this window keeps for its picture.
    pub fn release_video(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.video.update(cx, |video, _| video.leave(window));
    }

    /// Keeps the window on a display and at the aspect ratio, and remembers where it was left.
    fn bounds_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = Rect::from_gpui(window.bounds());
        let areas: Vec<Rect> =
            cx.displays().iter().map(|display| Rect::from_gpui(display.visible_bounds())).collect();
        if let Some(moved) = placement::rescue(bounds, &areas) {
            native_window::move_to(window, moved.to_gpui().origin, cx);
            return;
        }
        let height = bounds.w / self.aspect;
        if !self.os_keeps_aspect && (height - bounds.h).abs() > ASPECT_SLACK {
            window.resize(size(px(bounds.w), px(height)));
            return;
        }
        let Some(display) = window.display(cx) else { return };
        let display = Rect::from_gpui(display.bounds());
        if settings::get(cx).prefs.interface.remember_window_geometry {
            settings::update(cx, |s| placement::remember(&mut s.local.pip_places, display, bounds));
        }
        cx.notify();
    }

    /// Asks the main window to do `command`. Deferred: the main window may be drawing.
    fn send(&self, command: PipCommand, cx: &mut App) {
        let main = self.main;
        cx.defer(move |cx| {
            main.update(cx, |main, window, cx| main.pip_command(command, window, cx)).ok();
        });
    }

    /// A click on a control sends `command`.
    fn on_click(command: PipCommand, cx: &mut Context<Self>) -> OnClick {
        let this = cx.entity().downgrade();
        std::rc::Rc::new(move |_, _, cx| {
            this.update(cx, |this, cx| this.send(command, cx)).ok();
        })
    }

    /// A key was released: the main window's held-key gate must hear of it.
    fn forward_key_up(&mut self, _: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.main.update(cx, |main, _, _| main.release_keys()).ok();
    }

    /// Sends a key the main window's player understands there.
    fn forward_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = event.keystroke.clone();
        // Through the untyped handle: the typed one holds the main view while the key dispatches.
        let main: AnyWindowHandle = self.main.into();
        let sent = main.update(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        if sent.is_ok() {
            cx.stop_propagation();
        }
    }

    /// A double click on the picture goes back to the player.
    fn press(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.click_count >= 2 {
            self.send(PipCommand::PopIn, cx);
        }
    }

    /// Seeks to where the progress bar was pressed; the press doesn't move the window.
    fn seek(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let Some(bounds) = self.progress.bounds() else { return };
        let width = f32::from(bounds.size.width).max(1.);
        let fraction = (f32::from(event.position.x - bounds.left()) / width).clamp(0., 1.);
        self.send(PipCommand::Seek(fraction), cx);
    }

    /// The video, or for media without a picture the placeholder tile.
    fn render_picture(&self, view: &PipView, window: &Window) -> AnyElement {
        if view.video {
            return div().size_full().child(self.video.clone()).into_any_element();
        }
        let placeholder = if view.song { Placeholder::Song } else { Placeholder::Video };
        let size = window.viewport_size();
        ThumbnailTile::new(SizeClass::Grid, placeholder)
            .size(f32::from(size.width), f32::from(size.height))
            .into_any_element()
    }

    /// The hover controls, with the progress bar along the bottom.
    fn render_controls(
        &self,
        view: &PipView,
        accent: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pop_in = WindowButton { icon: icons::PIP, label: "Back to the player".into() };
        let handlers = Handlers {
            close: Self::on_click(PipCommand::Stop, cx),
            window: Self::on_click(PipCommand::PopIn, cx),
            previous: Self::on_click(PipCommand::Previous, cx),
            play: Self::on_click(PipCommand::PlayPause, cx),
            next: Self::on_click(PipCommand::Next, cx),
        };
        let bar = div()
            .id("pip-progress")
            .absolute()
            .left(px(PROGRESS_INSET))
            .right(px(PROGRESS_INSET))
            .bottom(px(PROGRESS_INSET))
            .h(px(PROGRESS_HEIGHT))
            .rounded(px(PROGRESS_HEIGHT / 2.))
            .bg(gpui::white().opacity(0.3))
            .overflow_hidden()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::seek))
            .child(self.progress.element())
            .child(
                div().absolute().left_0().top_0().h_full().w(relative(view.progress)).bg(accent),
            );
        player_overlay::overlay(GROUP, view.transport, pop_in, &handlers, true).child(bar)
    }
}

impl Render for PipWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Through the view: the main window may be the one opening this window.
        let view = self.main_view.upgrade().and_then(|main| main.read(cx).pip_view(cx));
        let root = div()
            .id("pip-window")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(gpui::black())
            .group(GROUP)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &PictureInPicture, _, cx| {
                this.send(PipCommand::PopIn, cx);
            }))
            .on_key_down(cx.listener(Self::forward_key))
            .on_key_up(cx.listener(Self::forward_key_up))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::press));
        let root = drags(root);
        let Some(view) = view else { return root };
        let picture = self.render_picture(&view, window);
        let accent = theme::palette(window, cx).accent;
        root.child(picture).child(self.render_controls(&view, accent, cx))
    }
}

/// Dragging the window anywhere moves it: GPUI moves it on macOS and Linux, Windows through its
/// drag area.
fn drags(root: Stateful<Div>) -> Stateful<Div> {
    if cfg!(target_os = "windows") {
        root.window_control_area(WindowControlArea::Drag)
    } else {
        window_chrome::drags_window(root)
    }
}

/// The work area of the display `window` is on.
fn work_area(window: &Window, cx: &App) -> Rect {
    window
        .display(cx)
        .or_else(|| cx.primary_display())
        .map_or(Rect::new(0., 0., 1280., 800.), |display| Rect::from_gpui(display.visible_bounds()))
}
