//! [`VideoView`]: the video area. It redraws when the engine delivers a picture, paints it
//! letterboxed on black, framed as Settings → Video and the player's panel say (aspect ratio,
//! crop, trimmed edges and zoom: S5.8.1, S7.2), shows a placeholder
//! for audio-only media, saying so when Settings turned video off, and toggles fullscreen on a
//! double-click (S1.4.1, S1.4.2, S1.4.4). Over the picture it draws the subtitle files the app
//! draws itself (S6.2), clear of the control bar while that covers the video's bottom.

use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedReceiver;
use gpui::{
    App, ClickEvent, ContentMask, Context, Entity, Hsla, Subscription, Task, WeakEntity, Window,
    WindowVisibility, black, canvas, div, hsla, prelude::*,
};

use super::Output;
use super::frame::Frame;
use super::frame_slot::{Delivery, FrameSlot};
use super::framing::{self, Framing, Source};
use super::still::Still;
use super::subtitles::{self, Covered, Overlay, Placing};
use crate::effects::{self, EffectSession};
use crate::engine::player::{Player, PlayerEvent};
use crate::engine::stream_out::Local;
use crate::keymap::actions::ToggleFullscreen;
use crate::settings::{self, SettingsStore};
use crate::theme::text_px;

/// What the placeholder for audio-only media says while music mode (S8.1) doesn't show its cover:
/// a visualizer draws there and hasn't drawn yet, or a stream's tracks aren't known yet.
const AUDIO_ONLY: &str = "♪";
const PLACEHOLDER_SIZE: f32 = 64.;
/// The placeholder's grey: readable on the black video area in both themes.
const PLACEHOLDER_COLOR: Hsla = hsla(0., 0., 1., 0.35);
/// Under the placeholder while Settings → Video turned video off, so a black screen has a reason.
const VIDEO_OFF: &str = tr_noop!("Video is off in Settings → Video");
const VIDEO_OFF_SIZE: f32 = 14.;
/// The size of what the view says while media plays on a cast device (S8.11).
const CASTING_SIZE: f32 = 20.;

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
    /// Redraw when the framing changes, in Settings or the player's panel.
    _settings: [Subscription; 2],
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
                    | PlayerEvent::CastChanged
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
            _settings: [
                cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
                cx.observe_global::<EffectSession>(|_, cx| cx.notify()),
            ],
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

    /// The picture as framing sees it: its display aspect, the player's (which squares
    /// anamorphic pixels that arrived late) or else its own, and its size in pixels.
    fn source(&self, frame: &Frame, cx: &App) -> Source {
        let pixels = frame.size();
        let aspect =
            self.player.read(cx).video_size().map_or(pixels, |size| (size.width, size.height));
        Source { aspect, pixels }
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

    /// The picture on screen and how it is framed, for a snapshot (S8.4); `None` while no
    /// picture shows.
    pub fn still(&self, cx: &App) -> Option<Still> {
        let frame = self.shown.clone()?;
        let source = self.source(&frame, cx);
        let framing = Framing::of(&effects::effective(cx).prefs.video);
        Some(Still::new(frame, source, framing))
    }

    /// Where the media plays when not here: "Playing on" a cast device (S8.11), or "Streaming
    /// to" a link while it streams without playing here (S8.12).
    fn elsewhere(&self, cx: &App) -> Option<String> {
        let player = self.player.read(cx);
        if let Some(device) = player.cast_target() {
            return Some(tr!("Playing on {device}", device = device.name));
        }
        let stream = player.stream_target().filter(|stream| stream.local == Local::StreamOnly)?;
        Some(tr!("Streaming to {link}", link = stream.url()))
    }

    fn is_audio_only(&self, cx: &App) -> bool {
        let player = self.player.read(cx);
        player.state().is_active() && player.video_size().is_none() && self.shown.is_none()
    }
}

impl Render for VideoView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.advance(window);
        let video = &effects::effective(cx).prefs.video;
        let (framing, video_on) = (Framing::of(video), video.enabled);
        // Nothing shows here while a cast device or only a stream gets the media, so the last
        // picture goes.
        let away = self.elsewhere(cx);
        let shown = self.shown.as_ref().filter(|_| away.is_none());
        let source = shown.map(|frame| self.source(frame, cx));
        let picture =
            shown.cloned().zip(source).map(|(frame, source)| picture(frame, source, framing));
        let placeholder = match away {
            Some(where_to) => Some(away_placeholder(where_to).into_any_element()),
            None => self.is_audio_only(cx).then(|| placeholder(video_on).into_any_element()),
        };
        let video_height = source.map_or(0, |source| source.aspect.1);
        let subtitles = self.overlay(cx).map(|overlay| {
            let scene = Scene { source, framing, video_height, covered: self.covered };
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

/// Where drawn subtitles go: the picture and its framing, its height in the video's pixels, and
/// what the bars over the view cover.
#[derive(Clone, Copy)]
struct Scene {
    source: Option<Source>,
    framing: Framing,
    video_height: u32,
    covered: Covered,
}

/// Paints `overlay` over the picture `scene` frames, or over the whole view without a picture.
fn drawn_subtitles(overlay: Overlay, scene: Scene) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            let scale = window.scale_factor();
            let picture = scene.source.map_or(bounds, |source| {
                framing::frame(bounds, source, scene.framing, scale).shown
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

/// Paints `frame`, which is `source`, letterboxed as `framing` says.
fn picture(frame: Frame, source: Source, framing: Framing) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let scale = window.scale_factor();
            let framed = framing::frame(bounds, source, framing, scale);
            let mask = ContentMask { bounds: framed.shown };
            window.with_content_mask(Some(mask), |window| frame.paint(framed.picture, window));
        },
    )
    .size_full()
}

/// The audio-only placeholder, saying why when Settings turned video off.
fn placeholder(video_on: bool) -> impl IntoElement {
    let note = (!video_on)
        .then(|| div().text_size(text_px(VIDEO_OFF_SIZE)).child(crate::i18n::text(VIDEO_OFF)));
    div()
        .flex()
        .flex_col()
        .items_center()
        .text_color(PLACEHOLDER_COLOR)
        .child(div().text_size(text_px(PLACEHOLDER_SIZE)).child(AUDIO_ONLY))
        .children(note)
}

/// What shows while the media plays elsewhere: where it plays.
fn away_placeholder(where_to: String) -> impl IntoElement {
    div().text_size(text_px(CASTING_SIZE)).text_color(PLACEHOLDER_COLOR).child(where_to)
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
