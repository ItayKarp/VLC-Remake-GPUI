//! Music mode (S8.1, `screens/music/*`, D15): audio-only media gets its own player layout, the
//! cover large and centred over a blurred, enlarged copy of itself with a "title - artist"
//! caption ([`view`]), and its own control bar (`control_bar/layout.rs`) whose seek bar always
//! shows the waveform (S7.8.3).
//!
//! The media's type decides it, not a profile: media with sound and no moving picture, which
//! takes in a file whose only "video" is its cover (an MP3's attached picture) and video files
//! while Settings → Video turned video off. Until the media's tracks are known, a file's
//! extension stands in, so a song never flashes the video layout first. A visualizer that draws
//! (Spectrometer, Scope, Spectrum) still takes the picture's place; the bar stays music's.
//!
//! The cover is looked up off the UI thread ([`cover`]) each time the file changes, so a queue
//! that mixes songs and videos switches layout per item; the window keeps its size.

mod cover;
mod view;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{App, Context, Div, RenderImage, Task, Window};

use self::cover::{Cover, Lookup};
use super::MainWindow;
use super::auto_hide::rules::Chrome;
use super::viewport;
use crate::app::media::Media;
use crate::db::metadata::MediaKind;
use crate::effects;
use crate::engine::tracks::{Track, TrackKind, codec};
use crate::library::folder_view::kind_by_extension;
use crate::settings;

/// Music mode's state: whether it is on, and the cover of the file it shows.
#[derive(Default)]
pub(super) struct MusicMode {
    /// Whether the media playing is audio-only, as of the last frame.
    on: bool,
    /// The file whose cover is shown or looked up.
    media: Option<PathBuf>,
    /// What the lookup found so far.
    cover: Lookup,
    /// Backdrops replaced, to free from GPUI's atlas.
    retired: Vec<Arc<RenderImage>>,
    _lookup: Option<Task<()>>,
}

impl MainWindow {
    /// Whether the player shows music mode's layout.
    pub(super) fn is_music_mode(&self) -> bool {
        self.music.on
    }

    /// Whether music mode shows the cover: no visualizer draws in its place.
    fn shows_cover(&self, cx: &App) -> bool {
        self.music.on && settings::get(cx).prefs.audio.visualizer.effect().is_none()
    }

    /// In music mode, the cover across the content area for `chrome`, in the video's place;
    /// `None` while the video (or a visualizer) shows.
    pub(super) fn render_cover(&self, chrome: Chrome, window: &Window, cx: &App) -> Option<Div> {
        if !self.shows_cover(cx) {
            return None;
        }
        let (top, bottom) = viewport::content_insets(chrome);
        let size = window.viewport_size();
        Some(self.render_music(f32::from(size.width), f32::from(size.height) - top - bottom, cx))
    }

    /// Follows the media once a frame: switches music mode as the media's type says, and looks
    /// up the cover of a new file.
    pub(super) fn sync_music_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for image in self.music.retired.drain(..) {
            window.drop_image(image).ok();
        }
        let on = self.plays_audio_only(cx);
        if on != self.music.on {
            self.music.on = on;
            // The seek bar shows the waveform in music mode.
            self.sync_waveform(cx);
        }
        let wanted = on.then(|| self.playing_file(cx)).flatten();
        if wanted == self.music.media {
            return;
        }
        let lookup = wanted.clone().map(|path| Self::look_up_cover(path, cx));
        self.music.switch_to(wanted, lookup);
    }

    /// Whether what plays is audio-only (D15).
    fn plays_audio_only(&self, cx: &App) -> bool {
        let Some(player) = self.loaded_player(cx) else { return false };
        let player = player.read(cx);
        if !player.state().is_active() {
            return false;
        }
        let kind = player.media().and_then(Media::as_path).map(kind_by_extension);
        let video_on = effects::effective(cx).prefs.video.enabled;
        is_audio_only(&player.tracks().all, kind, video_on)
    }

    /// The local file loaded, if it is one.
    fn playing_file(&self, cx: &App) -> Option<PathBuf> {
        let player = self.loaded_player(cx)?;
        player.read(cx).media()?.as_path().map(Path::to_path_buf)
    }

    /// Starts looking up the cover of the file at `path`; the answer replaces the cover shown.
    fn look_up_cover(path: PathBuf, cx: &mut Context<Self>) -> Task<()> {
        let found = cover::find(path.clone(), cx);
        cx.spawn(async move |this, cx| {
            let found = found.await;
            this.update(cx, |this, cx| this.show_cover(&path, found, cx)).ok();
        })
    }

    /// Shows `found` as the cover of the file at `path`, unless another file plays by now.
    fn show_cover(&mut self, path: &Path, found: Option<Cover>, cx: &mut Context<Self>) {
        if self.music.media.as_deref() == Some(path) {
            self.music.cover = found.map_or(Lookup::Missing, Lookup::Found);
            cx.notify();
        }
    }
}

impl MusicMode {
    /// Shows the file at `media` (none: music mode is off), whose cover `lookup` looks up,
    /// freeing the old cover's backdrop at the next frame.
    fn switch_to(&mut self, media: Option<PathBuf>, lookup: Option<Task<()>>) {
        let mut retired = std::mem::take(&mut self.retired);
        if let Lookup::Found(Cover { backdrop: Some(image), .. }) = &self.cover {
            retired.push(image.clone());
        }
        *self = Self { on: self.on, media, cover: Lookup::Looking, retired, _lookup: lookup };
    }
}

/// Whether media with `tracks` (none known yet: a file of `kind`, if it is a file) is
/// audio-only: it has sound and no moving picture that shows. A still picture (a cover) isn't
/// one, nor is video while `video_on` is false.
fn is_audio_only(tracks: &[Track], kind: Option<MediaKind>, video_on: bool) -> bool {
    if tracks.is_empty() {
        return kind == Some(MediaKind::Audio);
    }
    let has_sound = tracks.iter().any(|track| track.kind == TrackKind::Audio);
    let moving = |track: &&Track| {
        track.kind == TrackKind::Video
            && !track.codec.as_deref().is_some_and(codec::is_still_picture)
    };
    has_sound && !(video_on && tracks.iter().any(|track| moving(&track)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tracks::TrackId;

    fn track(id: i32, kind: TrackKind, codec: &str) -> Track {
        Track { codec: Some(codec.to_owned()), ..Track::new(TrackId(id), kind) }
    }

    #[test]
    fn a_files_extension_decides_until_its_tracks_are_known() {
        assert!(is_audio_only(&[], Some(MediaKind::Audio), true));
        assert!(!is_audio_only(&[], Some(MediaKind::Video), true));
        assert!(!is_audio_only(&[], None, true));
    }

    #[test]
    fn sound_without_moving_pictures_is_audio_only() {
        let song = [track(1, TrackKind::Audio, "MP3")];
        let film = [track(1, TrackKind::Audio, "AAC"), track(2, TrackKind::Video, "H.264")];
        assert!(is_audio_only(&song, Some(MediaKind::Video), true));
        assert!(!is_audio_only(&film, Some(MediaKind::Audio), true));
    }

    #[test]
    fn a_cover_track_is_not_video() {
        let tagged = [track(1, TrackKind::Audio, "MP3"), track(2, TrackKind::Video, "JPEG")];
        assert!(is_audio_only(&tagged, Some(MediaKind::Audio), true));
    }

    #[test]
    fn video_turned_off_plays_as_music_but_silent_video_does_not() {
        let film = [track(1, TrackKind::Audio, "AAC"), track(2, TrackKind::Video, "H.264")];
        let silent = [track(1, TrackKind::Video, "H.264")];
        assert!(is_audio_only(&film, Some(MediaKind::Video), false));
        assert!(!is_audio_only(&silent, Some(MediaKind::Video), false));
    }
}
