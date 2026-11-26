//! A libVLC instance and its media player, made with every option the app starts libVLC with:
//! the fixed [`ARGUMENTS`], audio and subtitle style, the Network and Performance settings and
//! the user's startup options. An option this libVLC doesn't know drops its group, not the rest.

use std::ffi::CString;

use anyhow::{Context as _, Result, bail};

use super::api::{Instance, LibVlc};
use super::worker::Handles;
use super::{certificates, options, system_options};

/// The arguments every libVLC instance starts with: the file name would otherwise be drawn into
/// the video when it starts, and the app finds subtitle files beside the video itself
/// ([`super::sidecar`]) to know which track came from which file.
pub(super) const ARGUMENTS: [&str; 2] = ["--no-video-title-show", "--no-sub-autodetect-file"];
/// Added without a video sink: decode and discard video.
pub(super) const NO_VIDEO_ARGUMENT: &str = "--vout=vdummy";

/// A new libVLC instance with [`ARGUMENTS`], the `setup` arguments (audio and subtitle style),
/// the Network and Performance settings ([`system_options`]) and the user's startup options
/// ([`options`]), and a media player on it; `with_video` false adds [`NO_VIDEO_ARGUMENT`].
pub(super) fn new_handles(vlc: &LibVlc, with_video: bool, setup: &[String]) -> Result<Handles> {
    let video = (!with_video).then_some(NO_VIDEO_ARGUMENT);
    let system = system_options::arguments(&system_options::current());
    let groups: [(&str, &[String]); 3] =
        [("audio", setup), ("network and performance", &system), ("startup", options::startup())];
    let instance = start_instance(vlc, video, &groups)?;
    if !system_options::current().verify_certificates {
        certificates::install(vlc, instance);
    }
    // SAFETY: the instance is live.
    let player = unsafe { (vlc.api.libvlc_media_player_new)(instance) };
    if player.is_null() {
        let error = vlc.last_error();
        // SAFETY: the instance is live and released once.
        unsafe { (vlc.api.libvlc_release)(instance) };
        bail!("libvlc_media_player_new: {error}");
    }
    Ok(Handles { instance, player })
}

/// A new libVLC instance with every group of options. An option this libVLC doesn't know fails
/// the instance, so it is retried without the last group, then the one before, and so on.
fn start_instance(
    vlc: &LibVlc,
    video: Option<&str>,
    groups: &[(&str, &[String])],
) -> Result<*mut Instance> {
    let mut count = groups.len();
    loop {
        let options = groups[..count].iter().flat_map(|(_, options)| options.iter());
        let arguments: Vec<&str> =
            ARGUMENTS.into_iter().chain(video).chain(options.map(String::as_str)).collect();
        let error = match new_instance(vlc, &arguments) {
            Ok(instance) => return Ok(instance),
            Err(error) => error,
        };
        let Some(last) = groups[..count].iter().rposition(|(_, options)| !options.is_empty())
        else {
            return Err(error);
        };
        tracing::warn!(
            "libvlc: {} options refused ({error:#}); starting without them",
            groups[last].0
        );
        count = last;
    }
}

/// A new libVLC instance started with `arguments`.
pub(super) fn new_instance(vlc: &LibVlc, arguments: &[&str]) -> Result<*mut Instance> {
    let arguments: Vec<CString> = arguments
        .iter()
        .map(|argument| CString::new(*argument))
        .collect::<Result<_, _>>()
        .context("an option holds a NUL character")?;
    let pointers: Vec<_> = arguments.iter().map(|argument| argument.as_ptr()).collect();
    // SAFETY: `pointers` holds NUL-terminated strings that outlive the call.
    let instance: *mut Instance =
        unsafe { (vlc.api.libvlc_new)(pointers.len() as i32, pointers.as_ptr()) };
    if instance.is_null() {
        bail!("libvlc_new: {}", vlc.last_error());
    }
    Ok(instance)
}
