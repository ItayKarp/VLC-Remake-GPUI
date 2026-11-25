//! The Subtitles tab's **External Tracks** (S2.6.4, S6.1.5, `subtitle_one-*.png` and
//! `subtitle_multiple-*.png`): the subtitle files found beside the video or added to it, each a
//! file icon and its name, with an orange ✓ at the end of the one showing and, on hovering
//! another, the swap icon: clicking it switches to that file. The pencil opens the Add Subtitles
//! dialog on the file playing (S6.4.7), where linked files can be removed; "+" asks for files to
//! add (S6.1.4).

use std::path::PathBuf;

use gpui::{
    ClickEvent, Context, Div, PathPromptOptions, SharedString, Stateful, Window, accesskit, div,
    prelude::*, px,
};

use super::sections::{
    CHECK, HEADER_BUTTON, HEADER_ICON, ROW_HEIGHT, ROW_TEXT, header, note, section,
};
use crate::assets::icons;
use crate::engine::tracks::{SubtitleFile, TrackId, TrackKind, Tracks};
use crate::i18n::DirectionalStyle;
use crate::main_window::MainWindow;
use crate::opening::kind::{self, Kind};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::IconButton;
use crate::ui::focus;
use crate::ui::icon::icon;
use crate::ui::toast::{Level, Toast};

/// The file icon's size.
const FILE_ICON: f32 = 24.;
/// The hover group of each row, so its swap icon shows while the row is hovered.
const ROW_GROUP: &str = "subtitle-file-row";

impl MainWindow {
    /// The subtitle files of the media, with edit and add buttons.
    pub(super) fn render_files(
        files: &[SubtitleFile],
        tracks: &Tracks,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let edit = IconButton::new("edit-subtitles", icons::EDIT, tr!("Edit subtitle files"))
            .size(HEADER_BUTTON, HEADER_ICON)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.edit_playing_subtitles(window, cx);
            }));
        let add = IconButton::new("add-subtitles", icons::ADD, tr!("Add a subtitle file"))
            .size(HEADER_BUTTON, HEADER_ICON)
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                Self::prompt_for_subtitles(window, cx);
            }));
        let selected = tracks.selected(TrackKind::Subtitle);
        let rows = files.iter().enumerate().map(|(index, file)| {
            let track = file.track;
            file_row(index, file.name().into(), FileState::of(track, selected), p)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.select_file(track, cx);
                }))
                .into_any_element()
        });
        let empty = files.is_empty().then(|| note(tr!("No subtitle files."), p));
        section(p)
            .child(header(
                tr!("External Tracks"),
                vec![edit.into_any_element(), add.into_any_element()],
                p,
            ))
            .children(empty)
            .children(rows)
    }

    /// Shows the subtitle file whose track is `track`. A file whose track isn't listed yet
    /// has nothing to select.
    fn select_file(&self, track: Option<TrackId>, cx: &mut Context<Self>) {
        if track.is_some() {
            self.select_track(TrackKind::Subtitle, track, cx);
        }
    }

    /// Asks for subtitle files to add to the playing media.
    pub(in crate::main_window) fn prompt_for_subtitles(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr!("Add").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            this.update(cx, |this, cx| this.add_subtitle_files(paths, cx)).ok();
        })
        .detach();
    }

    /// Adds the subtitle files among `paths`, and says which ones aren't subtitles.
    fn add_subtitle_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let (subtitles, others): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|path| kind::by_extension(path) == Some(Kind::Subtitle));
        if let Some(other) = others.first() {
            let name = SubtitleFile::new(other.clone()).name();
            let text = tr!("{name} isn't a subtitle file.", name);
            self.show_toast(Toast::new(Level::Info, text), cx);
        }
        self.attach_subtitles(&subtitles, cx);
    }
}

/// Where a file stands against the subtitle showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileState {
    /// Its track shows.
    Shown,
    /// Its track is listed and can be switched to.
    Listed,
    /// The backend hasn't listed its track yet.
    Waiting,
}

impl FileState {
    fn of(track: Option<TrackId>, selected: Option<TrackId>) -> Self {
        match track {
            None => Self::Waiting,
            Some(_) if track == selected => Self::Shown,
            Some(_) => Self::Listed,
        }
    }
}

/// One file: its icon and name, then the ✓ when it shows or the swap icon on hover.
fn file_row(index: usize, name: SharedString, state: FileState, p: &Palette) -> Stateful<Div> {
    let hover = p.hover;
    let group = SharedString::from(format!("{ROW_GROUP}-{index}"));
    let end = match state {
        FileState::Shown => icon(icons::CHECK, CHECK, p.accent_fg).into_any_element(),
        FileState::Listed => div()
            .opacity(0.)
            .group_hover(group.clone(), |style| style.opacity(1.))
            .child(icon(icons::SWAP, CHECK, p.text_muted))
            .into_any_element(),
        FileState::Waiting => div().size(px(CHECK)).flex_none().into_any_element(),
    };
    let row = div()
        .id(("file", index))
        .debug_selector(move || format!("subtitle-file-{index}"))
        .group(group)
        .h(px(ROW_HEIGHT))
        .flex_none()
        .mx(px(8.))
        .px(px(14.))
        .flex_row_start()
        .items_center()
        .gap(px(14.))
        .rounded(px(6.))
        .border_2()
        .border_color(gpui::transparent_black())
        .text_size(text_px(ROW_TEXT))
        .tab_index(0)
        .when(state != FileState::Waiting, |row| row.cursor_pointer().hover(move |s| s.bg(hover)))
        .role(accesskit::Role::RadioButton)
        .aria_selected(state == FileState::Shown)
        .aria_label(name.clone())
        .child(icon(icons::SUBTITLE_FILE, FILE_ICON, p.text_muted))
        .child(div().flex_1().min_w_0().truncate().child(name))
        .child(end);
    focus::ring(row, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_shows_waits_or_can_be_switched_to() {
        let (one, two) = (Some(TrackId(1)), Some(TrackId(2)));
        assert_eq!(FileState::of(one, one), FileState::Shown);
        assert_eq!(FileState::of(two, one), FileState::Listed);
        assert_eq!(FileState::of(None, None), FileState::Waiting);
    }
}
