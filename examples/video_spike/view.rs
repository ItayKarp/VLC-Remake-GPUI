//! The spike window: the video, a GPUI overlay drawn on top, and the measurement clock.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedReceiver;
use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, InputEvent as _,
    InteractiveElement as _, IntoElement, Modifiers, MouseMoveEvent, ObjectFit, ParentElement as _,
    Render, SharedString, Styled as _, Window, WindowBounds, WindowHandle, WindowOptions, black,
    canvas, div, point, px, rgba, size, white,
};

use crate::libvlc::LibVlc;
use crate::options::Options;
use crate::player::Player;
use crate::presenter::{Pipeline, Presenter, letterbox};
use crate::sink::Size;
use crate::stats::Recorder;
use crate::track::VideoGeometry;

/// The window size the measurements use, in logical pixels.
const WINDOW_SIZE: (f32, f32) = (1280.0, 720.0);
/// How often the spike moves the mouse itself, so input-to-paint is measured unattended.
const SYNTHETIC_INPUT_EVERY: Duration = Duration::from_millis(100);
/// Where the synthetic mouse alternates: over the overlay button, then over the video.
const SYNTHETIC_INPUT_AT: [(f32, f32); 2] = [(1200.0, 684.0), (640.0, 360.0)];

/// Opens `options.file` and a window playing it.
pub fn open(options: &Options, vlc: Arc<LibVlc>, cx: &mut App) -> Result<()> {
    let mut player = Player::open(vlc, &options.file, &options.vlc_arguments)?;
    player.parse();
    let geometry = player.video_geometry();
    println!("video track: {geometry:?}");
    let picture_size = Size {
        display: geometry.map(VideoGeometry::display_size),
        max_texture: options.max_texture,
    };
    let pipeline = Pipeline::new(options.path, picture_size);
    player.attach(pipeline.sink);
    player.play()?;
    let name = options.file.file_name().unwrap_or_default().to_string_lossy();
    let label: SharedString = format!("{name} · path {}", options.path.name()).into();
    let seconds = options.seconds;
    let bounds = Bounds::centered(None, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
    let window_options =
        WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
    let display = picture_size.display;
    let window = cx.open_window(window_options, move |_, cx| {
        cx.new(|cx| {
            follow_frames(pipeline.woken, cx);
            report_every_second(seconds, cx);
            SpikeView {
                player: Some(player),
                presenter: pipeline.presenter,
                recorder: Rc::new(RefCell::new(Recorder::start())),
                label,
                display,
            }
        })
    })?;
    cx.activate(true);
    drive_input(window, cx);
    Ok(())
}

/// Moves the mouse inside `window` every [`SYNTHETIC_INPUT_EVERY`], through GPUI's own event
/// dispatch: this times GPUI's handling and painting, not the OS input queue.
fn drive_input(window: WindowHandle<SpikeView>, cx: &mut App) {
    // The untyped handle: the typed one would lease the view the event handler updates.
    let window: AnyWindowHandle = window.into();
    cx.spawn(async move |cx| {
        for (x, y) in SYNTHETIC_INPUT_AT.into_iter().cycle() {
            cx.background_executor().timer(SYNTHETIC_INPUT_EVERY).await;
            let event = MouseMoveEvent {
                position: point(px(x), px(y)),
                pressed_button: None,
                modifiers: Modifiers::default(),
            };
            let moved = cx.update_window(window, |_, window, cx| {
                window.dispatch_event(event.to_platform_input(), cx);
            });
            if moved.is_err() {
                return;
            }
        }
    })
    .detach();
}

/// Plays one file and measures it.
pub struct SpikeView {
    player: Option<Player>,
    presenter: Presenter,
    recorder: Rc<RefCell<Recorder>>,
    label: SharedString,
    /// The picture's display size in square pixels, once known.
    display: Option<(u32, u32)>,
}

impl SpikeView {
    fn report(&mut self, second: u64) {
        let Some(player) = &self.player else { return };
        if self.display.is_none() {
            // Tracks some containers hide until playback: pick up their shape late.
            let geometry = player.video_geometry();
            self.display = geometry.map(VideoGeometry::display_size);
            if geometry.is_some() {
                println!("video track found during playback: {geometry:?}");
            }
        }
        let vlc = player.stats();
        let (published, unseen) = self.presenter.counts();
        println!(
            "t={second}s pos={:.1}s shown_fps={:.1} vlc_displayed={} vlc_late={} published={} \
             unseen={unseen}",
            player.time_ms() as f64 / 1000.0,
            self.recorder.borrow().presented_fps(),
            vlc.displayed_pictures,
            vlc.lost_pictures,
            published,
        );
    }

    fn finish(&mut self) {
        let Some(player) = self.player.take() else { return };
        let summary = self.recorder.borrow().summary(&player.stats(), self.presenter.counts());
        println!("RESULT {} {summary}", self.label);
        drop(player);
    }
}

/// Re-renders whenever libVLC publishes a frame.
fn follow_frames(mut woken: UnboundedReceiver<()>, cx: &mut Context<SpikeView>) {
    cx.spawn(async move |this, cx| {
        while woken.next().await.is_some() {
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
    .detach();
}

/// Prints a progress line each second, then the summary, then quits.
fn report_every_second(seconds: u64, cx: &mut Context<SpikeView>) {
    cx.spawn(async move |this, cx| {
        for second in 1..=seconds {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            if this.update(cx, |this, _| this.report(second)).is_err() {
                return;
            }
        }
        this.update(cx, |this, cx| {
            this.finish();
            cx.quit();
        })
        .ok();
    })
    .detach();
}

impl Render for SpikeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(due) = self.presenter.advance(window) {
            self.recorder.borrow_mut().new_frame(due);
        }
        // Two empty canvases around the video time how long painting it takes (for the CPU
        // path that includes uploading the frame into GPUI's atlas).
        let paint_started = Rc::new(Cell::new(None));
        let started = paint_started.clone();
        let recorder = self.recorder.clone();
        div()
            .size_full()
            .relative()
            .bg(black())
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| {
                this.recorder.borrow_mut().input(Instant::now());
                cx.notify();
            }))
            .child(canvas(|_, _, _| {}, move |_, (), _, _| started.set(Some(Instant::now()))))
            .child(self.video_area(window))
            .child(canvas(
                |_, _, _| {},
                move |_, (), _, _| {
                    let cost = paint_started.get().map_or(Duration::ZERO, |at| at.elapsed());
                    recorder.borrow_mut().painted(cost);
                },
            ))
            .child(overlay(&self.label, self.recorder.borrow().presented_fps()))
    }
}

impl SpikeView {
    /// The video, letterboxed by its display aspect when known (anamorphic pictures are
    /// stretched to it), else fitted by its pixel size.
    fn video_area(&self, window: &Window) -> impl IntoElement {
        let Some(aspect) = self.display else {
            return div().absolute().size_full().child(self.presenter.element(ObjectFit::Contain));
        };
        let viewport = window.viewport_size();
        let (left, top, width, height) =
            letterbox((f32::from(viewport.width), f32::from(viewport.height)), aspect);
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(width))
            .h(px(height))
            .child(self.presenter.element(ObjectFit::Fill))
    }
}

/// A translucent control bar with a hover-able button: the GPUI UI that must stay on top of
/// the video and responsive while it plays.
fn overlay(label: &SharedString, fps: f64) -> impl IntoElement {
    div()
        .absolute()
        .bottom(px(16.0))
        .left(px(16.0))
        .right(px(16.0))
        .p(px(12.0))
        .rounded(px(10.0))
        .bg(rgba(0x000000aa))
        .text_color(white())
        .flex()
        .justify_between()
        .items_center()
        .child(format!("{label} · {fps:.1} fps shown"))
        .child(
            div()
                .id("overlay-button")
                .px(px(12.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .bg(rgba(0xffffff22))
                .hover(|style| style.bg(rgba(0xffffff66)))
                .child("Hover me"),
        )
}
