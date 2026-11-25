//! The Stream dialog (S8.12.1), not designed, in Open Disc's look
//! ([`crate::main_window::device_dialog`]): **Protocol** (HTTP, RTSP or UDP), **Who can watch**
//! (this computer only, by default, or the devices on its networks, with a warning) and **Port**
//! for HTTP and RTSP, or **Send to** an address and port for UDP, and **Quality**, for the
//! network's speed. What the stream carries, whether it also plays here, and how it is encoded
//! come from Settings → Recording's Streaming rows, which the dialog sums up.
//!
//! A port another program holds is refused before anything starts. While the media streams,
//! the dialog shows where to watch it, with Copy Link and Stop.

mod open;
mod render;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::rc::Rc;

use gpui::{
    App, AppContext as _, ClipboardItem, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    Focusable, SharedString, Subscription, Window,
};

use crate::app::media::Media;
use crate::engine::stream_out::{
    self, AccessCode, Contents, Encoding, Local, Protocol, StreamQuality, StreamTarget,
};
use crate::main_window::device_dialog::{Then, close_with};
use crate::settings::{self, RecordingSettings};
use crate::ui::dropdown::Dropdown;
use crate::ui::text_input::{self, TextInput};

/// Where UDP sends unless told otherwise: this computer, for a player here to test it.
const DEFAULT_SEND_TO: &str = "127.0.0.1";
/// The most characters a port takes.
const PORT_DIGITS: usize = 5;
/// The most characters an address takes (an IPv6 address in full).
const ADDRESS_LENGTH: usize = 45;

/// Who can watch a stream viewers connect to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Reach {
    /// Only this computer: it listens on loopback.
    #[default]
    ThisComputer,
    /// Anyone on the networks this computer is on.
    Network,
}

/// Runs with the stream to start, or `None` to stop the one running, once the dialog closed.
type OnChoose = Then<Option<StreamTarget>>;

/// The Stream dialog's view.
pub(super) struct StreamDialog {
    /// What streams.
    media: Media,
    /// The stream `media` goes to now, if it streams.
    running: Option<StreamTarget>,
    protocol: Protocol,
    reach: Reach,
    send_to: Entity<TextInput>,
    port: Entity<TextInput>,
    qualities: Entity<Dropdown>,
    /// Why Stream didn't start, until a field changes.
    problem: Option<SharedString>,
    on_choose: OnChoose,
    focus: FocusHandle,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<DismissEvent> for StreamDialog {}

impl Focusable for StreamDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StreamDialog {
    /// The dialog streaming `media`, which streams to `running` now if it does, handing what
    /// was chosen to `on_choose`.
    pub(super) fn new(
        media: Media,
        running: Option<StreamTarget>,
        on_choose: impl Fn(Option<StreamTarget>, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let protocol = Protocol::default();
        let send_to = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(DEFAULT_SEND_TO)
                .with_label(tr!("Send to"))
                .with_max_len(ADDRESS_LENGTH)
        });
        let port = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(protocol.default_port().to_string())
                .with_label(tr!("Port"))
                .with_max_len(PORT_DIGITS)
        });
        let qualities = cx.new(|cx| {
            let names = StreamQuality::ALL.iter().map(|quality| quality.name().into()).collect();
            let medium = StreamQuality::ALL.iter().position(|&q| q == StreamQuality::default());
            Dropdown::new(tr!("Quality"), names, medium.unwrap_or_default(), window, cx)
        });
        let edited = |this: &mut Self,
                      _: Entity<TextInput>,
                      _: &text_input::Event,
                      cx: &mut Context<Self>| {
            this.problem = None;
            cx.notify();
        };
        let subscriptions = [cx.subscribe(&send_to, edited), cx.subscribe(&port, edited)];
        Self {
            media,
            running,
            protocol,
            reach: Reach::default(),
            send_to,
            port,
            qualities,
            problem: None,
            on_choose: Rc::new(on_choose),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// Streams with `protocol`, moving the port to its usual one if it was the last one's.
    fn set_protocol(&mut self, protocol: Protocol, cx: &mut Context<Self>) {
        let usual = self.protocol.default_port().to_string();
        if self.port.read(cx).text().trim() == usual {
            let port = protocol.default_port().to_string();
            self.port.update(cx, |input, cx| input.set_text(port, cx));
        }
        self.protocol = protocol;
        self.problem = None;
        cx.notify();
    }

    /// Lets `reach` watch.
    fn set_reach(&mut self, reach: Reach, cx: &mut Context<Self>) {
        self.reach = reach;
        self.problem = None;
        cx.notify();
    }

    /// Start: checks the fields and the port, then closes and starts the stream; or Stop
    /// Streaming, while one runs.
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running.is_some() {
            close_with(None, self.on_choose.clone(), window, cx);
            return;
        }
        match self.target(cx) {
            Ok(target) => close_with(Some(target), self.on_choose.clone(), window, cx),
            Err(problem) => {
                self.problem = Some(problem.into());
                cx.notify();
            }
        }
    }

    /// The stream the fields and Settings → Recording describe, or why there's none.
    fn target(&self, cx: &App) -> Result<StreamTarget, String> {
        let recording = &settings::get(cx).prefs.recording;
        let contents = Contents::from_settings(recording).ok_or(tr!(
            "Settings → Recording includes neither audio nor video in streams. Turn one on there."
        ))?;
        let port = parse_port(self.port.read(cx).text())?;
        let (address, viewer_host) = self.addresses(port, cx)?;
        if self.protocol.listens() && !stream_out::is_port_free(address) {
            return Err(tr!("Port {port} is in use by another program. Try another one.", port));
        }
        let quality = StreamQuality::ALL.get(self.qualities.read(cx).selected()).copied();
        Ok(StreamTarget {
            protocol: self.protocol,
            address,
            viewer_host,
            quality: quality.unwrap_or_default(),
            contents,
            encoding: Encoding { preset: recording.encoding_preset, tuning: recording.tuning },
            local: local(recording),
            access: self.access_code(address)?,
        })
    }

    /// A new access code for a stream that listens on every network, so only those given its
    /// link can watch (S9.7.4); none for one on this computer alone or sent to one address.
    fn access_code(&self, address: SocketAddr) -> Result<Option<AccessCode>, String> {
        if !self.protocol.listens() || address.ip().is_loopback() {
            return Ok(None);
        }
        AccessCode::new()
            .map(Some)
            .ok_or_else(|| tr!("VLC couldn't make an access code for the stream.").to_owned())
    }

    /// Where the stream listens or sends to, and the address viewers reach it at.
    fn addresses(&self, port: u16, cx: &App) -> Result<(SocketAddr, IpAddr), String> {
        if !self.protocol.listens() {
            let text = self.send_to.read(cx).text().trim().to_owned();
            let ip: IpAddr = text
                .parse()
                .map_err(|_| tr!("“{text}” isn't an IP address, like 192.168.1.20", text))?;
            return Ok((SocketAddr::new(ip, port), ip));
        }
        Ok(match self.reach {
            Reach::ThisComputer => {
                let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
                (SocketAddr::new(loopback, port), loopback)
            }
            Reach::Network => {
                let every = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
                let own = stream_out::network_address().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
                (every, own)
            }
        })
    }

    /// Copies the running stream's link.
    fn copy_link(&self, cx: &mut App) {
        if let Some(running) = &self.running {
            cx.write_to_clipboard(ClipboardItem::new_string(running.url()));
        }
    }
}

/// Whether Settings → Recording plays streams here too.
fn local(recording: &RecordingSettings) -> Local {
    if recording.play_locally_while_streaming { Local::PlaysHere } else { Local::StreamOnly }
}

/// The port typed, from 1 to 65535.
fn parse_port(text: &str) -> Result<u16, String> {
    let text = text.trim();
    text.parse::<u16>()
        .ok()
        .filter(|&port| port > 0)
        .ok_or_else(|| tr!("“{text}” isn't a port: use a number from 1 to 65535", text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_are_numbers_in_range() {
        assert_eq!(parse_port(" 8080 "), Ok(8080));
        assert!(parse_port("0").is_err());
        assert!(parse_port("70000").is_err());
        assert!(parse_port("http").is_err());
    }

    #[test]
    fn streams_play_here_as_settings_say() {
        let mut recording = RecordingSettings::default();
        assert_eq!(local(&recording), Local::PlaysHere);
        recording.play_locally_while_streaming = false;
        assert_eq!(local(&recording), Local::StreamOnly);
    }
}
