//! Recording (S8.6): Shift+R, the macOS Playback menu and the video menu's Record start
//! recording the media playing into Settings → Recording's folder (`recording` picks it), and
//! stop it again. While it records, a red dot and the time recorded show in the video's
//! top-right corner, over the video in fullscreen and with the bars hidden too.
//!
//! The free space is checked as it records: below [`STOP_SPACE`] the recording is stopped, so
//! its file is finished and plays, and an error says why. Once a recording ends (stopped, or the
//! media ended, failed or was replaced) a toast says where its file is, with Open and Show in
//! Finder.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui::{AnyElement, App, AppContext as _, AsyncApp, Context, Div, Task, div, prelude::*, px};

use super::MainWindow;
use super::control_bar::clock;
use super::library_panel::show_in_folder_label;
use super::video_menu::NOTHING_PLAYING;
use crate::engine::player::PlayerEvent;
use crate::engine::recording::Recording;
use crate::recording::{self, Destination, STOP_SPACE, Unready};
use crate::settings::{self, display_path};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::toast::{Level, Toast};

/// How often the badge's time and the free space are brought up to date.
const TICK: Duration = Duration::from_secs(1);
/// How far the badge sits from the video's top and right edges, as the OSD does on the left.
const MARGIN: f32 = 24.;
/// The badge's dot and text.
const DOT: f32 = 10.;
const TEXT_SIZE: f32 = 14.;
/// How long, and how many times, the folder is looked in for a stopped recording's file.
const FIND_FOR: Duration = Duration::from_secs(3);
const FIND_TRIES: u32 = 12;
/// Why seeking does nothing while recording: libVLC 3 refuses it.
const SEEK_WHILE_RECORDING: &str = "Can't seek while recording";
/// Why Record can't start while a recording is being set up.
const STARTING: &str = "Starting to record";

/// The recording being made, from asking for it until its file is found.
#[derive(Default)]
pub(super) struct RecordingWatch {
    session: Option<Session>,
}

/// One recording.
struct Session {
    /// Where it goes; `None` while the folder is still being checked.
    destination: Option<Destination>,
    /// When it was asked for: its file changed since.
    since: SystemTime,
    /// Whether it was stopped because the disk was nearly full.
    disk_full: bool,
    /// Brings the badge and the free space up to date while it records.
    ticks: Option<Task<()>>,
}

impl MainWindow {
    /// Why Record can't start now; `None` when it can, or while recording (it stops).
    pub(super) fn why_no_recording(&self, cx: &App) -> Option<&'static str> {
        let Some(player) = self.loaded_player(cx) else { return Some(NOTHING_PLAYING) };
        let player = player.read(cx);
        if player.recorded().is_some() {
            return None;
        }
        if self.recording.session.is_some() {
            return Some(STARTING);
        }
        (!player.state().is_active()).then_some(NOTHING_PLAYING)
    }

    /// Whether a recording is running.
    pub(super) fn is_recording(&self, cx: &App) -> bool {
        self.loaded_player(cx).is_some_and(|player| player.read(cx).recorded().is_some())
    }

    /// Starts recording, or stops the running recording; says over the video why it can't.
    pub(super) fn toggle_recording(&mut self, cx: &mut Context<Self>) {
        if self.is_recording(cx) {
            self.stop_recording(cx);
            return;
        }
        if let Some(why) = self.why_no_recording(cx) {
            self.show_osd(why, cx);
            return;
        }
        let candidates = recording::candidates(&settings::get(cx).local.recordings_folder);
        let since = SystemTime::now();
        self.recording.session =
            Some(Session { destination: None, since, disk_full: false, ticks: None });
        let preparing = cx.background_spawn(async move { recording::prepare(&candidates) });
        cx.spawn(async move |this, cx| {
            let prepared = preparing.await;
            this.update(cx, |this, cx| this.folder_prepared(prepared, cx)).ok();
        })
        .detach();
    }

    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.loaded_player(cx) {
            player.update(cx, |player, _| player.stop_recording());
        }
    }

    /// Records into the folder found, or says why there is none.
    fn folder_prepared(&mut self, prepared: Result<Destination, Unready>, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else {
            self.recording.session = None;
            return;
        };
        let Some(session) = &mut self.recording.session else { return };
        match prepared {
            Ok(destination) => {
                player.update(cx, |player, _| player.start_recording(&destination.folder));
                session.destination = Some(destination);
            }
            Err(unready) => {
                self.recording.session = None;
                self.show_toast(unready_toast(&unready), cx);
            }
        }
    }

    /// Follows what the player says about recording, and says over the video that a seek
    /// waits for the recording to end.
    pub(super) fn on_recording_event(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        let recording = match event {
            PlayerEvent::RecordingChanged(recording) => *recording,
            PlayerEvent::SeekRefused => return self.show_osd(SEEK_WHILE_RECORDING, cx),
            _ => return,
        };
        match recording {
            Recording::Started => self.recording_started(cx),
            Recording::Stopped => self.recording_stopped(cx),
            Recording::Refused => {
                self.recording.session = None;
                let message = "This media can't be recorded.";
                self.show_toast(Toast::new(Level::Error, message), cx);
            }
        }
        cx.notify();
    }

    /// Starts bringing the badge and the free space up to date every [`TICK`].
    fn recording_started(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &mut self.recording.session else { return };
        session.ticks = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TICK).await;
            while this.update(cx, Self::recording_tick).is_ok() {
                cx.background_executor().timer(TICK).await;
            }
        }));
    }

    /// Redraws the badge, and stops the recording if the disk is nearly full. The space is
    /// read off the UI thread, as a network folder may be slow to answer.
    fn recording_tick(&mut self, cx: &mut Context<Self>) {
        cx.notify();
        let Some(folder) = self.recording_folder() else { return };
        let room = cx.background_spawn(async move { recording::has_room(&folder, STOP_SPACE) });
        cx.spawn(async move |this, cx| {
            if !room.await {
                this.update(cx, Self::disk_nearly_full).ok();
            }
        })
        .detach();
    }

    /// The folder the running recording goes to.
    fn recording_folder(&self) -> Option<PathBuf> {
        let destination = self.recording.session.as_ref()?.destination.as_ref()?;
        Some(destination.folder.clone())
    }

    fn disk_nearly_full(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &mut self.recording.session else { return };
        if !session.disk_full {
            session.disk_full = true;
            self.stop_recording(cx);
        }
    }

    /// Finds the finished file, off the UI thread, and says where it is. libVLC makes the file
    /// of a recording still waiting for its first keyframe only as it stops, a moment after it
    /// says so, so the folder is looked in again for up to [`FIND_FOR`].
    fn recording_stopped(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.recording.session.take() else { return };
        let Some(destination) = session.destination else { return };
        let (since, disk_full) = (session.since, session.disk_full);
        let folder = destination.folder.clone();
        cx.spawn(async move |this, cx| {
            let file = find_settled(folder, since, cx).await;
            if let Some(file) = &file {
                tracing::info!("recording: saved {}", crate::logging::path(file));
            }
            let toast = stopped_toast(file, &destination, disk_full);
            this.update(cx, |this, cx| this.show_toast(toast, cx)).ok();
        })
        .detach();
    }

    /// The red dot and the time recorded, while recording.
    pub(super) fn render_recording_badge(&self, p: &Palette, cx: &App) -> Option<AnyElement> {
        let recorded = self.loaded_player(cx)?.read(cx).recorded()?;
        let badge = badge(p).child(clock::format(recorded));
        Some(div().absolute().top(px(MARGIN)).right(px(MARGIN)).child(badge).into_any_element())
    }
}

/// The recording made in `folder` since `since`, looked for off the UI thread every so often
/// for up to [`FIND_FOR`].
async fn find_settled(folder: PathBuf, since: SystemTime, cx: &mut AsyncApp) -> Option<PathBuf> {
    for _ in 0..FIND_TRIES {
        let folder = folder.clone();
        let found = cx.background_spawn(async move { recording::find(&folder, since) }).await;
        if found.is_some() {
            return found;
        }
        cx.background_executor().timer(FIND_FOR / FIND_TRIES).await;
    }
    None
}

/// The badge: a red dot before the time, on the OSD's surface.
fn badge(p: &Palette) -> Div {
    let dot = div().size(px(DOT)).flex_none().rounded_full().bg(p.danger);
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .py(px(6.))
        .rounded(px(8.))
        .bg(p.panel)
        .shadow_lg()
        .text_size(text_px(TEXT_SIZE))
        .text_color(p.text_strong)
        .font_features(clock::tabular_digits())
        .child(dot)
}

/// The toast for a recording that couldn't start; the details go to the log.
fn unready_toast(unready: &Unready) -> Toast {
    let message = match unready {
        Unready::Unwritable(error) => {
            tracing::warn!("recording: no folder could be written: {error}");
            "Couldn't record: no folder could be written.".to_owned()
        }
        Unready::NoSpace(folder) => {
            format!("Couldn't record: there's too little space left in {}.", display_path(folder))
        }
    };
    Toast::new(Level::Error, message)
}

/// The toast for a finished recording: where its file is, with Open and Show in Finder; why it
/// stopped when the disk filled; or that nothing was recorded.
fn stopped_toast(file: Option<PathBuf>, destination: &Destination, disk_full: bool) -> Toast {
    let folder = display_path(&destination.folder);
    let Some(file) = file else {
        let message = if disk_full {
            format!("The disk is full, so nothing could be recorded in {folder}.")
        } else {
            "Nothing was recorded: stop a moment later, once the picture has started.".to_owned()
        };
        return Toast::new(Level::Error, message);
    };
    let (level, message) = match (&destination.instead_of, disk_full) {
        (_, true) => (
            Level::Error,
            format!("The disk is nearly full, so recording stopped. It's saved in {folder}"),
        ),
        (Some(wanted), false) => (
            Level::Warning,
            format!("Couldn't record in {}, so the recording is in {folder}", display_path(wanted)),
        ),
        (None, false) => (Level::Success, format!("Recording saved in {folder}")),
    };
    with_file_actions(Toast::new(level, message), &file)
}

/// Open and Show in Finder for `file`, which leave the toast up.
fn with_file_actions(toast: Toast, file: &Path) -> Toast {
    let (open, reveal) = (file.to_path_buf(), file.to_path_buf());
    toast
        .lasting_action("Open", move |_, cx| cx.open_with_system(&open))
        .lasting_action(show_in_folder_label(), move |_, cx| cx.reveal_path(&reveal))
}
