//! How the Stream dialog looks: Media, then either where to watch the stream running, or
//! Protocol, Who can watch (or Send to), Port, Quality and what Settings → Recording sends, in
//! the shared frame ([`crate::main_window::device_dialog`]).

use gpui::{
    ClickEvent, Context, Div, Entity, IntoElement, SharedString, Window, div, prelude::*, px,
};

use super::{Reach, StreamDialog};
use crate::engine::stream_out::{Contents, Local, Protocol, StreamTarget};
use crate::i18n::DirectionalStyle;
use crate::main_window::device_dialog::{Frame, controls, section};
use crate::settings::{self, RecordingSettings};
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::metrics;
use crate::ui::segmented::Segmented;
use crate::ui::text_input::TextInput;

/// The quality list's width.
const QUALITY_WIDTH: f32 = 320.;
/// The port field's and the address field's widths.
const PORT_WIDTH: f32 = 90.;
const ADDRESS_WIDTH: f32 = 220.;
/// What letting the network watch means (S8.12 edge case).
const NETWORK_WARNING: &str = tr_noop!(
    "Anyone on the networks this computer is on can watch, a café's \
     or hotel's Wi-Fi included. Use it on networks you trust."
);

impl Render for StreamDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let mut sections = vec![self.media_section(&p)];
        let buttons = if let Some(running) = self.running {
            sections.push(Self::running_section(&running, &p, cx));
            (tr!("Close"), tr!("Stop"))
        } else {
            sections.push(self.protocol_section(&p, cx));
            sections.push(self.address_section(&p, cx));
            sections.push(self.quality_section(&p));
            sections.push(contents_section(&settings::get(cx).prefs.recording, &p));
            (tr!("Cancel"), tr!("Stream"))
        };
        if let Some(problem) = &self.problem {
            sections.push(div().text_color(p.danger).child(problem.clone()));
        }
        let frame = Frame { id: "stream", title: tr!("Stream"), sections, can_play: true, buttons };
        div().track_focus(&self.focus).child(frame.render(&p, window, cx, Self::confirm))
    }
}

impl StreamDialog {
    /// Media: what streams.
    fn media_section(&self, p: &Palette) -> Div {
        let name = div().min_w_0().truncate().child(self.media.display_name());
        section(tr!("Media"), tr!("Pausing, seeking and the tracks picked reach viewers too."), p)
            .child(controls(name))
    }

    /// Streaming: where to watch the stream running, and Copy Link.
    fn running_section(running: &StreamTarget, p: &Palette, cx: &mut Context<Self>) -> Div {
        let description = match running.protocol {
            Protocol::Udp => {
                tr!("Sending to {address}. Open this in a player there:", address = running.address)
            }
            _ => tr!("Open this link in a player, such as VLC's Open Network Stream:").to_owned(),
        };
        let copy = Button::new("stream-copy-link", tr!("Copy Link"))
            .style(ButtonStyle::Filled)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.copy_link(cx)));
        let link = div().min_w_0().truncate().text_color(p.text_strong).child(running.url());
        let row = div().flex_row_start().items_center().gap(px(12.)).child(copy).child(link);
        let mut streaming = section(tr!("Streaming"), description, p).child(controls(row));
        if running.is_open_to_network() {
            streaming = streaming.child(warning(p));
        }
        streaming
    }

    /// Protocol: HTTP, RTSP or UDP, and what each suits.
    fn protocol_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let names = Protocol::ALL.map(Protocol::name);
        let selected = Protocol::ALL.iter().position(|&each| each == self.protocol);
        let choice =
            Segmented::new("stream-protocol", tr!("Protocol"), names, selected.unwrap_or(0))
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.set_protocol(Protocol::ALL.get(*index).copied().unwrap_or_default(), cx);
                }));
        let description = match self.protocol {
            Protocol::Http => {
                tr!("Viewers open a link to this computer. Works with nearly every player.")
            }
            Protocol::Rtsp => tr!("Viewers open an rtsp:// link: suits cameras' and TVs' players."),
            Protocol::Udp => tr!("Sends to one address, which a player there listens on."),
        };
        section(tr!("Protocol"), description, p).child(controls(choice))
    }

    /// Who can watch and Port, or Send to and Port for UDP.
    fn address_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let port = labelled(tr!("Port"), &self.port, PORT_WIDTH);
        if !self.protocol.listens() {
            let send_to = labelled(tr!("Address"), &self.send_to, ADDRESS_WIDTH);
            let row = div().flex_row_start().items_center().gap(px(16.)).child(send_to).child(port);
            let description = tr!("Where the stream goes. 127.0.0.1 is this computer.");
            return section(tr!("Send to"), description, p).child(controls(row));
        }
        let selected = usize::from(self.reach == Reach::Network);
        let reach = [tr!("Only this computer"), tr!("This network")];
        let choice = Segmented::new("stream-reach", tr!("Who can watch"), reach, selected)
            .on_select(cx.listener(|this, index: &usize, _, cx| {
                this.set_reach(if *index == 0 { Reach::ThisComputer } else { Reach::Network }, cx);
            }));
        let mut who = section(tr!("Who can watch"), tr!("Who can open the stream's link."), p)
            .child(controls(choice))
            .child(controls(port));
        if self.reach == Reach::Network {
            who = who.child(warning(p));
        }
        who
    }

    /// Quality: how much picture, for the network's speed.
    fn quality_section(&self, p: &Palette) -> Div {
        let description =
            tr!("If viewers stutter, the network can't keep up: pick a lower quality.");
        section(tr!("Quality"), description, p)
            .child(controls(div().w(px(QUALITY_WIDTH)).child(self.qualities.clone())))
    }
}

/// What Settings → Recording's Streaming rows send, and where to change them.
fn contents_section(recording: &RecordingSettings, p: &Palette) -> Div {
    let description = tr!("Set in Settings → Recording → Streaming, with how it's encoded.");
    section(tr!("Sent"), description, p).child(controls(
        div().text_size(text_px(metrics::TEXT_SMALL + 1.)).child(sent_summary(recording)),
    ))
}

/// What a stream sends, as Settings → Recording's Streaming rows say, and whether it plays here.
fn sent_summary(recording: &RecordingSettings) -> SharedString {
    let sent = match Contents::from_settings(recording) {
        Some(Contents::Everything { subtitles: true }) => {
            tr!("Sound and picture, subtitles burned in.")
        }
        Some(Contents::Everything { subtitles: false }) => tr!("Sound and picture, no subtitles."),
        Some(Contents::PictureOnly { subtitles: true }) => {
            tr!("Picture only, subtitles burned in.")
        }
        Some(Contents::PictureOnly { subtitles: false }) => tr!("Picture only."),
        Some(Contents::SoundOnly) => tr!("Sound only."),
        None => tr!("Nothing: neither audio nor video is included."),
    };
    let here = match super::local(recording) {
        Local::PlaysHere => tr!("Its picture shows here too, without sound."),
        Local::StreamOnly => tr!("It doesn't play here meanwhile."),
    };
    format!("{sent} {here}").into()
}

/// The warning about letting the network watch.
fn warning(p: &Palette) -> Div {
    div()
        .pt(px(10.))
        .text_size(text_px(metrics::TEXT_SMALL))
        .text_color(p.warning)
        .child(crate::i18n::text(NETWORK_WARNING))
}

/// `field` after its `label`, `width` wide.
fn labelled(label: &'static str, field: &Entity<TextInput>, width: f32) -> Div {
    div()
        .flex_row_start()
        .items_center()
        .gap(px(10.))
        .text_size(text_px(metrics::TEXT_SMALL + 1.))
        .child(label)
        .child(div().w(px(width)).child(field.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_says_what_is_sent_and_whether_it_plays_here() {
        let streaming = |stream_audio, stream_video, stream_subtitles| RecordingSettings {
            stream_audio,
            stream_video,
            stream_subtitles,
            ..RecordingSettings::default()
        };
        let cases = [
            (streaming(true, true, true), "Sound and picture, subtitles burned in."),
            (streaming(true, true, false), "Sound and picture, no subtitles."),
            (streaming(false, true, true), "Picture only, subtitles burned in."),
            (streaming(false, true, false), "Picture only."),
            (streaming(true, false, true), "Sound only."),
            (streaming(false, false, true), "Nothing: neither audio nor video is included."),
        ];
        for (recording, sent) in cases {
            let summary = sent_summary(&recording);
            assert_eq!(summary, format!("{sent} Its picture shows here too, without sound."));
        }
        let elsewhere = RecordingSettings {
            play_locally_while_streaming: false,
            ..streaming(true, false, false)
        };
        assert_eq!(sent_summary(&elsewhere), "Sound only. It doesn't play here meanwhile.");
    }
}
