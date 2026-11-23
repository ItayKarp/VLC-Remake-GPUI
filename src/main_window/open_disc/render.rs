//! How the Open Disc dialog looks: Disc Type, Disc, then the kind's own section, in the shared
//! frame ([`crate::main_window::device_dialog`]).

use gpui::{AnyElement, ClickEvent, Context, Div, IntoElement, Window, div, prelude::*, px};

use super::{OpenDiscDialog, Tracks};
use crate::discs::DiscKind;
use crate::engine::discs::CdTrack;
use crate::main_window::device_dialog::{Frame, controls, note, section};
use crate::queue::row_duration;
use crate::theme::{self, Palette};
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::metrics;
use crate::ui::segmented::Segmented;
use crate::ui::toggle::Checkbox;

/// The title and chapter fields' width.
const NUMBER_WIDTH: f32 = 72.;
/// The most tracks listed before the rest are counted.
const LISTED_TRACKS: usize = 12;
/// What the dialog says of encrypted Blu-rays: libVLC can play them only with the user's own
/// AACS library and keys, and the app ships no way around the protection.
const BLU_RAY_NOTE: &str = "Encrypted Blu-rays play only with libaacs and decryption keys \
     installed by you; VLC includes neither. Menus need Java.";

impl Render for OpenDiscDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let mut sections = vec![self.kind_section(&p, cx), self.disc_section(&p, cx)];
        sections.push(self.options_section(&p, cx));
        let frame = Frame {
            id: "open-disc",
            title: "Open Disc",
            sections,
            can_play: !self.media(cx).is_empty(),
            buttons: ("Cancel", "Play"),
        };
        div().track_focus(&self.focus).child(frame.render(&p, window, cx, Self::play))
    }
}

impl OpenDiscDialog {
    /// Disc Type: DVD, Blu-ray, Audio CD, Video CD.
    fn kind_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let selected = DiscKind::ALL.iter().position(|&kind| kind == self.kind).unwrap_or(0);
        let kinds =
            Segmented::new("disc-kind", "Disc type", DiscKind::ALL.map(DiscKind::name), selected)
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.set_kind(DiscKind::ALL[*index], cx);
                }));
        section("Disc Type", "What kind of disc to play.", p).child(controls(kinds))
    }

    /// Disc: the list, Browse… and Refresh.
    fn disc_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let browse = Button::new("open-disc-browse", "Browse…")
            .style(ButtonStyle::Filled)
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| Self::browse(window, cx)));
        let refresh = Button::new("open-disc-refresh", "Refresh")
            .style(ButtonStyle::Filled)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.look_for_discs(cx)));
        let row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().min_w_0().child(self.disc_list.clone()))
            .child(refresh)
            .child(browse);
        let description = "A disc in a drive, or a disc's folder or image on this computer.";
        let mut disc = section("Disc", description, p).child(controls(row));
        if self.looked && self.discs.is_empty() {
            disc = disc
                .child(note("Insert a disc, then press Refresh, or choose a folder or image.", p));
        }
        disc
    }

    /// The kind's own section: where to start, or the tracks.
    fn options_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        match self.kind {
            DiscKind::AudioCd => self.tracks_section(p),
            DiscKind::Dvd | DiscKind::BluRay | DiscKind::Vcd => self.start_section(p, cx),
        }
    }

    /// Start: at the menus, or at a title (and chapter).
    fn start_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let mut start = section("Start", "Where playback begins.", p);
        if self.kind.has_menus() {
            let menus = Checkbox::new("open-disc-menus", self.menus, "Start at the disc's menu")
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    this.menus = *on;
                    cx.notify();
                }));
            start = start.child(controls(menus));
        }
        let mut numbers =
            div().flex().items_center().gap(px(10.)).child("Title").child(number(&self.title));
        if self.kind == DiscKind::Dvd {
            numbers = numbers.child("Chapter").child(number(&self.chapter));
        }
        start = start.child(controls(numbers.text_size(px(metrics::TEXT_SMALL + 1.))));
        if self.kind == DiscKind::BluRay {
            start = start.child(note(BLU_RAY_NOTE, p));
        }
        start
    }

    /// Tracks: the audio CD's tracks, or what is happening.
    fn tracks_section(&self, p: &Palette) -> Div {
        let tracks = section("Tracks", "Play queues every track.", p);
        match &self.tracks {
            Tracks::Unread => tracks.child(note("Pick an audio CD in a drive.", p)),
            Tracks::Reading => tracks.child(note("Reading the disc…", p)),
            Tracks::Read(read) if read.is_empty() => {
                tracks.child(note("No audio tracks were found on this disc.", p))
            }
            Tracks::Read(read) => tracks.child(controls(track_list(read, p))),
        }
    }
}

/// A title or chapter field.
fn number(field: &gpui::Entity<crate::ui::text_input::TextInput>) -> Div {
    div().w(px(NUMBER_WIDTH)).child(field.clone())
}

/// The first tracks, one line each, then how many more there are.
fn track_list(tracks: &[CdTrack], p: &Palette) -> Div {
    let rows = tracks
        .iter()
        .take(LISTED_TRACKS)
        .zip(1..)
        .map(|(track, number): (&CdTrack, u32)| track_row(number, track, p));
    let more = tracks.len().saturating_sub(LISTED_TRACKS);
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .children(rows)
        .children((more > 0).then(|| note(format!("and {more} more"), p)))
}

/// One track: its number, title, artist and length.
fn track_row(number: u32, track: &CdTrack, p: &Palette) -> AnyElement {
    let title = track.title.clone().unwrap_or_else(|| format!("Track {number}"));
    let artist = track.artist.as_ref().map_or_default(|artist| format!(" — {artist}"));
    let length = track.duration.map_or_default(row_duration);
    div()
        .flex()
        .gap(px(10.))
        .text_size(px(metrics::TEXT_SMALL + 1.))
        .child(div().w(px(22.)).text_color(p.text_muted).child(number.to_string()))
        .child(div().flex_1().min_w_0().truncate().child(format!("{title}{artist}")))
        .child(div().text_color(p.text_muted).child(length))
        .into_any_element()
}
