//! [`VideoView`]: the video area. It redraws when the engine delivers a picture, paints it
//! letterboxed on black at Settings → Video's aspect ratio and crop (S5.8.1), shows a placeholder
//! for audio-only media, saying so when Settings turned video off, and toggles fullscreen on a
//! double-click (S1.4.1, S1.4.2, S1.4.4). Over the picture it draws the subtitle files the app
//! draws itself (S6.2), clear of the control bar while that covers the video's bottom.

use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedReceiver;
use gpui::{
    App, ClickEvent, ContentMask, Context, Entity, Hsla, Subscription, Task, WeakEntity, Window,
    WindowVisibility, black, canvas, div, hsla, prelude::*, px,
};

use super::Output;
use super::frame::Frame;
use super::frame_slot::{Delivery, FrameSlot};
use super::framing::{self, AspectRatio, Crop};
use super::subtitles::{self, Covered, Overlay, Placing};
use crate::engine::player::{Player, PlayerEvent};
use crate::keymap::actions::ToggleFullscreen;
use crate::settings::{self, SettingsStore};

/// What the placeholder for audio-only media says until music mode (Stage 8) shows artwork.
const AUDIO_ONLY: &str = "♪";
const PLACEHOLDER_SIZE: f32 = 64.;
/// The placeholder's grey: readable on the black video area in both themes.
const PLACEHOLDER_COLOR: Hsla = hsla(0., 0., 1., 0.35);
/// Under the placeholder while Settings → Video turned video off, so a black screen has a reason.
const VIDEO_OFF: &str = "Video is off in Settings → Video";
const VIDEO_OFF_SIZE: f32 = 14.;

/// The video area of the player.
pub struct VideoView {
    slot: Arc<FrameSlot<Frame>>,
    /// The picture on screen.
    shown: Option<Frame>,
    player: Entity<Player>,
    /// What the bars floating over the view cover, which subtitles keep clear of.
    covered: Covered,
    _follow: Task<()>,
    /// The visibility of the window the view is in.
    visibility: Subscription,
    _changes: Subscription,
    /// Redraws when the aspect ratio or crop changes.
    _settings: Subscription,
}

impl VideoView {
    /// A view of `output`'s pictures, for the media `player` plays.
    pub fn new(
        output: Output,
        player: Entity<Player>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let Output { slot, woken, .. } = output;
        slot.set_accepting(window.is_visible());
        let visibility = Self::follow_visibility(window, cx);
        // The aspect, the audio-only placeholder and the subtitles depend on what the player
        // knows; subtitles change with the position and their sync while paused, when no frame comes.
        let changes = cx.subscribe(&player, |_, _, event: &PlayerEvent, cx| {
            let redraw = matches!(
                event,
                PlayerEvent::VideoSizeChanged(_)
                    | PlayerEvent::StateChanged(_)
                    | PlayerEvent::PositionChanged(_)
                    | PlayerEvent::TracksChanged
                    | PlayerEvent::SubtitleSyncChanged
            );
            if redraw {
                cx.notify();
            }
        });
        Self {
            slot,
            shown: None,
            player,
            covered: Covered::default(),
            _follow: cx.spawn(async move |this, cx| follow_frames(woken, this, cx).await),
            visibility,
            _changes: changes,
            _settings: cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
        }
    }

    /// Lets go of `window`, which the view is leaving for another (the video's own window,
    /// S5.4.1): frees what that window keeps for the picture on screen.
    pub fn leave(&mut self, window: &mut Window) {
        if let Some(shown) = self.shown.take() {
            shown.release(window);
        }
    }

    /// The view now shows in `window`: takes pictures while that window shows.
    pub fn arrive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.slot.set_accepting(window.is_visible());
        self.visibility = Self::follow_visibility(window, cx);
        cx.notify();
    }

    /// Keeps subtitles clear of what `covered` says the control bar and title bar cover while
    /// they float over the video (S6.2.5); nothing when they don't.
    pub fn set_covered(&mut self, covered: Covered, cx: &mut Context<Self>) {
        if self.covered != covered {
            self.covered = covered;
            cx.notify();
        }
    }

    /// Takes pictures only while the window shows (S1.4.4): a minimized or covered window
    /// uploads and draws nothing, and redraws at once when it shows again.
    fn follow_visibility(window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.observe_window_visibility(window, |this, visibility, _, cx| {
            let visible = visibility == WindowVisibility::Visible;
            this.slot.set_accepting(visible);
            if visible {
                cx.notify();
            }
        })
    }

    /// Swaps in the newest delivery, freeing what GPUI kept for the replaced picture.
    fn advance(&mut self, window: &mut Window) {
        let next = match self.slot.take() {
            Some(Delivery::Frame(frame)) => Some(frame),
            Some(Delivery::Cleared) => None,
            None => return,
        };
        if let Some(old) = std::mem::replace(&mut self.shown, next) {
            old.release(window);
        }
    }

    /// The picture's display aspect: the player's (squares anamorphic pixels that arrived
    /// late) or else the picture's own.
    fn aspect(&self, frame: &Frame, cx: &App) -> (u32, u32) {
        self.player
            .read(cx)
            .video_size()
            .map_or_else(|| frame.size(), |size| (size.width, size.height))
    }

    /// The subtitle cues the app draws now, if any show.
    fn overlay(&self, cx: &App) -> Option<Overlay> {
        let player = self.player.read(cx);
        let cues: Vec<_> = player.drawn_subtitles(Instant::now())?.into_iter().cloned().collect();
        if cues.is_empty() {
            return None;
        }
        let subtitles = &settings::get(cx).prefs.subtitles;
        Some(Overlay {
            cues,
            style: crate::main_window::subtitle_style::style(subtitles),
            file_colors: subtitles.file_styles,
            area: subtitles.area,
        })
    }

    fn is_audio_only(&self, cx: &App) -> bool {
        let player = self.player.read(cx);
        player.state().is_active() && player.video_size().is_none() && self.shown.is_none()
    }
}

impl Render for VideoView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.advance(window);
        let video = &settings::get(cx).prefs.video;
        let (framing, video_on) = ((video.aspect_ratio, video.crop), video.enabled);
        let aspect = self.shown.as_ref().map(|frame| self.aspect(frame, cx));
        let picture =
            self.shown.clone().zip(aspect).map(|(frame, aspect)| picture(frame, aspect, framing));
        let placeholder = self.is_audio_only(cx).then(|| placeholder(video_on));
        let video_height = aspect.map_or(0, |(_, height)| height);
        let subtitles = self.overlay(cx).map(|overlay| {
            let scene = Scene { aspect, framing, video_height, covered: self.covered };
            drawn_subtitles(overlay, scene)
        });
        div()
            .id("video")
            .relative()
            .size_full()
            // The bars are black whatever the theme, like the picture's own black.
            .bg(black())
            .flex()
            .items_center()
            .justify_center()
            // The window it is in decides which screen fullscreen goes to (S5.4.1).
            .on_click(|event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    window.dispatch_action(Box::new(ToggleFullscreen), cx);
                }
            })
            .children(picture)
            .children(placeholder)
            .children(subtitles)
    }
}

/// Where drawn subtitles go: the picture's aspect and framing, its height in the video's pixels,
/// and what the bars over the view cover.
#[derive(Clone, Copy)]
struct Scene {
    aspect: Option<(u32, u32)>,
    framing: (AspectRatio, Crop),
    video_height: u32,
    covered: Covered,
}

/// Paints `overlay` over the picture `scene` frames, or over the whole view without a picture.
fn drawn_subtitles(overlay: Overlay, scene: Scene) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            let scale = window.scale_factor();
            let picture = scene.aspect.map_or(bounds, |aspect| {
                framing::frame(bounds, aspect, scene.framing.0, scene.framing.1, scale).shown
            });
            let Scene { video_height, covered, .. } = scene;
            let placing = Placing { view: bounds, picture, video_height, covered };
            subtitles::paint(&overlay, &placing, window, cx);
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Paints `frame`, of display `aspect`, letterboxed at the aspect ratio and crop of `framing`.
fn picture(frame: Frame, aspect: (u32, u32), framing: (AspectRatio, Crop)) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let scale = window.scale_factor();
            let framed = framing::frame(bounds, aspect, framing.0, framing.1, scale);
            let mask = ContentMask { bounds: framed.shown };
            window.with_content_mask(Some(mask), |window| frame.paint(framed.picture, window));
        },
    )
    .size_full()
}

/// The audio-only placeholder, saying why when Settings turned video off.
fn placeholder(video_on: bool) -> impl IntoElement {
    let note = (!video_on).then(|| div().text_size(px(VIDEO_OFF_SIZE)).child(VIDEO_OFF));
    div()
        .flex()
        .flex_col()
        .items_center()
        .text_color(PLACEHOLDER_COLOR)
        .child(div().text_size(px(PLACEHOLDER_SIZE)).child(AUDIO_ONLY))
        .children(note)
}

/// Redraws the view whenever the engine delivers, until the view is gone.
async fn follow_frames(
    mut woken: UnboundedReceiver<()>,
    this: WeakEntity<VideoView>,
    cx: &mut gpui::AsyncApp,
) {
    while woken.next().await.is_some() {
        if this.update(cx, |_, cx| cx.notify()).is_err() {
            break;
        }
    }
}
