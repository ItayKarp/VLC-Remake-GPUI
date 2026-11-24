//! The track popover's tabs: **Embedded Tracks** ("None" and each track, S2.6.2) on every tab,
//! **Output Device** on Audio (S2.6.3) and **External Tracks** on Subtitles (S2.6.4, in
//! `files.rs`).
//!
//! Each frame reads the player afresh, so a track list or device list that changes while the
//! popover is open updates in place; each tab scrolls on its own and keeps its place.

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, SharedString, Stateful, accesskit, div,
    prelude::*, px,
};

use crate::assets::icons;
use crate::engine::audio::{AudioDevice, OutputDevice};
use crate::engine::player::Player;
use crate::engine::tracks::{SubtitleFile, TrackId, TrackKind, Tracks};
use crate::main_window::MainWindow;
use crate::settings;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::IconButton;
use crate::ui::focus;
use crate::ui::icon::icon;

/// A choice's height, and its text size.
pub(super) const ROW_HEIGHT: f32 = 44.;
pub(super) const ROW_TEXT: f32 = 14.;
/// A section header's height, and its text size.
const HEADER_HEIGHT: f32 = 48.;
const HEADER_TEXT: f32 = 13.;
/// The header's and notes' distance from the popover's left edge.
const INSET: f32 = 28.;
/// The ✓'s size, and the space it keeps on unchecked rows.
pub(super) const CHECK: f32 = 24.;
/// The header buttons' side and icon size.
pub(super) const HEADER_BUTTON: f32 = 28.;
pub(super) const HEADER_ICON: f32 = 20.;
/// The divider between two sections.
const DIVIDER: f32 = 2.;

/// What the popover shows, read from the player before drawing.
pub(super) struct TrackSnapshot {
    pub(super) tracks: Tracks,
    files: Vec<SubtitleFile>,
    devices: Vec<AudioDevice>,
    /// The device the user chose, and the one sound goes to.
    preferred: OutputDevice,
    active: OutputDevice,
}

impl MainWindow {
    /// What the popover shows of the loaded media.
    pub(super) fn track_snapshot(&self, cx: &App) -> Option<TrackSnapshot> {
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        Some(TrackSnapshot {
            tracks: player.tracks().clone(),
            files: player.subtitle_files().to_vec(),
            devices: player.audio_devices().to_vec(),
            preferred: player.preferred_audio_device().clone(),
            active: player.active_audio_device().clone(),
        })
    }

    /// The tab of `kind`, scrolling under the tab bar.
    pub(super) fn render_track_tab(
        kind: TrackKind,
        snapshot: &TrackSnapshot,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let second = match kind {
            TrackKind::Audio => Some(Self::render_devices(snapshot, p, cx)),
            TrackKind::Subtitle => {
                Some(Self::render_files(&snapshot.files, &snapshot.tracks, p, cx))
            }
            TrackKind::Video => None,
        };
        let divider = second.is_some().then(|| div().h(px(DIVIDER)).flex_none().bg(p.border));
        div()
            .id(SharedString::from(format!("track-tab-{kind:?}")))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(Self::render_embedded(kind, snapshot, p, cx))
            .children(divider)
            .children(second)
    }

    /// "None" and the media's own tracks of `kind`, without the added subtitle files.
    fn render_embedded(
        kind: TrackKind,
        snapshot: &TrackSnapshot,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let tracks = &snapshot.tracks;
        let selected = tracks.selected(kind);
        let from_files: Vec<TrackId> =
            snapshot.files.iter().filter_map(|file| file.track).collect();
        let own = tracks.of(kind).filter(|track| !from_files.contains(&track.id));
        let choices = std::iter::once((None, "None".to_owned()))
            .chain(own.map(|track| (Some(track.id), tracks.label(track.id).unwrap_or_default())));
        let rows = choices.enumerate().map(|(index, (id, label))| {
            choice_row(("embedded", index), label.into(), id == selected, p)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.select_track(kind, id, cx);
                }))
                .into_any_element()
        });
        section(p).child(header("Embedded Tracks", Vec::new(), p)).children(rows)
    }

    /// The output devices, with a check on the one sound goes to and a refresh button.
    fn render_devices(snapshot: &TrackSnapshot, p: &Palette, cx: &mut Context<Self>) -> Div {
        let refresh = IconButton::new("refresh-devices", icons::REFRESH, "Look for devices again")
            .size(HEADER_BUTTON, HEADER_ICON)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh_devices(cx)));
        let devices = snapshot.devices.iter().map(|device| {
            (OutputDevice::Device(device.id.clone()), SharedString::from(device.name.clone()))
        });
        let choices =
            std::iter::once((OutputDevice::SystemDefault, "System default".into())).chain(devices);
        let rows = choices.enumerate().map(|(index, (device, name))| {
            let checked = device == snapshot.active;
            choice_row(("device", index), name, checked, p)
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    choose_device(&device, cx);
                }))
                .into_any_element()
        });
        // The chosen device is unplugged: the check sits on the system default instead.
        let missing = snapshot.preferred != snapshot.active;
        let note = missing.then(|| {
            note("The chosen device isn't connected. Sound plays through the system default.", p)
        });
        section(p)
            .child(header("Output Device", vec![refresh.into_any_element()], p))
            .children(note)
            .children(rows)
    }

    pub(super) fn select_track(
        &self,
        kind: TrackKind,
        track: Option<TrackId>,
        cx: &mut Context<Self>,
    ) {
        if let Some(player) = self.loaded_player(cx) {
            player.update(cx, |player, cx| player.select_track(kind, track, cx));
        }
    }

    fn refresh_devices(&self, cx: &mut Context<Self>) {
        if let Some((player, _)) = &self.player {
            player.update(cx, |player, _| Player::refresh_audio_devices(player));
        }
    }
}

/// Plays through `device` from now on: the settings observer passes it to the player.
fn choose_device(device: &OutputDevice, cx: &mut App) {
    settings::update(cx, |s| device.id().clone_into(&mut s.local.audio.output_device));
}

/// A section of the popover: a header and its choices.
pub(super) fn section(p: &Palette) -> Div {
    div().py(px(8.)).flex().flex_col().text_color(p.text)
}

/// A section's muted title, with its buttons at the right end.
pub(super) fn header(title: &'static str, buttons: Vec<AnyElement>, p: &Palette) -> Div {
    div()
        .h(px(HEADER_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(16.))
        .pl(px(INSET))
        .pr(px(26.))
        .text_size(text_px(HEADER_TEXT))
        .text_color(p.text_muted)
        .child(div().flex_1().child(title))
        .children(buttons)
}

/// A muted line of explanation under a header.
pub(super) fn note(text: &'static str, p: &Palette) -> Div {
    div()
        .px(px(INSET))
        .pb(px(8.))
        .text_size(text_px(HEADER_TEXT))
        .text_color(p.text_muted)
        .child(text)
}

/// One choice: a ✓ when it is the selected one, then its name.
fn choice_row(
    id: impl Into<ElementId>,
    label: SharedString,
    checked: bool,
    p: &Palette,
) -> Stateful<Div> {
    let hover = p.hover;
    let check = if checked {
        icon(icons::CHECK, CHECK, p.accent_fg).into_any_element()
    } else {
        div().size(px(CHECK)).flex_none().into_any_element()
    };
    let row = div()
        .id(id.into())
        .h(px(ROW_HEIGHT))
        .flex_none()
        .mx(px(8.))
        .px(px(14.))
        .flex()
        .items_center()
        .gap(px(14.))
        .rounded(px(6.))
        .border_2()
        .border_color(gpui::transparent_black())
        .text_size(text_px(ROW_TEXT))
        .tab_index(0)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .role(accesskit::Role::RadioButton)
        .aria_selected(checked)
        .aria_label(label.clone())
        .child(check)
        .child(div().flex_1().min_w_0().truncate().child(label));
    focus::ring(row, p)
}
