//! Settings → Network and Performance (S5.10.1, S5.10.2) as the player's libVLC startup options.
//! They are read once, before the player starts, so changes apply at the next launch; the page
//! offers to restart. "Rescan plugins" asks for one rescan, which this launch does and clears.

use gpui::App;

use super::real_time;
use crate::engine::libvlc::system_options;
use crate::engine::system_options::{Caching, Ciphers, Proxy, SystemOptions};
use crate::privacy::keychain::{self, Secret};
use crate::settings::{
    self, CipherPreference, ClockSynchronisation, Decoder, Encoder, HardwareDecoding,
    NetworkSettings, PostProcessing, Preferences, SkipLoopFilter,
};

/// Hands the settings to the player's libVLC instance, before it starts.
pub fn init(cx: &mut App) {
    let current = settings::get(cx);
    let rescan = current.local.rescan_plugins;
    let proxy = proxy(&current.prefs.network);
    system_options::set(options(&current.prefs, proxy, rescan));
    if rescan {
        settings::update(cx, |settings| settings.local.rescan_plugins = false);
    }
}

/// The startup options `prefs` ask for, with the proxy already read.
fn options(prefs: &Preferences, proxy: Option<Proxy>, rescan_plugins: bool) -> SystemOptions {
    let network = &prefs.network;
    let performance = &prefs.performance;
    let caching = network.caching();
    SystemOptions {
        caching: Caching {
            local_files: caching.local_files,
            discs: caching.discs,
            network: caching.network,
            live_capture: caching.live_capture,
        },
        proxy,
        verify_certificates: network.verify_certificates,
        ciphers: ciphers(network.ciphers),
        hardware_decoding: performance.hardware_decoding == HardwareDecoding::Automatic,
        post_processing: post_processing(performance.post_processing),
        skip_loop_filter: skip_loop_filter(performance.skip_loop_filter),
        decoding_threads: performance.decoding_threads,
        drop_late_frames: performance.drop_late_frames,
        real_time_priority: real_time_priority(performance.real_time_priority),
        priority_offset: performance.priority_offset,
        encoder: encoder(performance.encoder),
        decoder: (performance.decoder == Decoder::Ffmpeg).then_some("avcodec"),
        clock_synchronisation: clock(performance.clock_synchronisation),
        clock_jitter_ms: performance.clock_jitter_ms,
        clock_average: performance.clock_average,
        rescan_plugins,
    }
}

/// Whether to ask libVLC for real-time priority: `wanted`, where libVLC has the option (not
/// macOS) and the system allows it ([`real_time::refusal`]); the row says when it doesn't.
fn real_time_priority(wanted: bool) -> bool {
    wanted && !cfg!(target_os = "macos") && real_time::refusal().is_none()
}

/// The proxy "HTTP proxy" names, with its password from the keychain; `None` for the system's.
fn proxy(network: &NetworkSettings) -> Option<Proxy> {
    let url = settings::proxy_url(&network.http_proxy)?;
    if url.password().is_some() {
        tracing::warn!("network: the proxy address holds a password; ignoring the proxy");
        return None;
    }
    let password = keychain::load(Secret::ProxyPassword).unwrap_or_else(|error| {
        tracing::warn!("network: no proxy password: {}", error.message());
        None
    });
    Some(Proxy { url: url.to_string(), password })
}

fn ciphers(preference: CipherPreference) -> Ciphers {
    match preference {
        CipherPreference::Performance => Ciphers::Performance,
        CipherPreference::Balanced => Ciphers::Normal,
        CipherPreference::Secure128 => Ciphers::Secure128,
        CipherPreference::Secure256 => Ciphers::Secure256,
    }
}

/// The decoder's post-processing quality, 0 to 6.
fn post_processing(level: PostProcessing) -> u8 {
    match level {
        PostProcessing::Off => 0,
        PostProcessing::Low => 2,
        PostProcessing::Medium => 4,
        PostProcessing::High => 6,
    }
}

/// libVLC's `avcodec-skiploopfilter` value.
fn skip_loop_filter(skip: SkipLoopFilter) -> u8 {
    match skip {
        SkipLoopFilter::Never => 0,
        SkipLoopFilter::NonReference => 1,
        SkipLoopFilter::Bidirectional => 2,
        SkipLoopFilter::NonKey => 3,
        SkipLoopFilter::Always => 4,
    }
}

/// libVLC's module name for the encoder.
fn encoder(encoder: Encoder) -> Option<&'static str> {
    match encoder {
        Encoder::Automatic => None,
        Encoder::X264 => Some("x264"),
        Encoder::X265 => Some("x265"),
        Encoder::Ffmpeg => Some("avcodec"),
    }
}

fn clock(synchronisation: ClockSynchronisation) -> Option<bool> {
    match synchronisation {
        ClockSynchronisation::Default => None,
        ClockSynchronisation::On => Some(true),
        ClockSynchronisation::Off => Some(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Buffering;

    #[test]
    fn default_settings_are_libvlc_defaults() {
        assert_eq!(options(&Preferences::default(), None, false), SystemOptions::default());
    }

    #[test]
    fn custom_buffering_and_performance_reach_the_options() {
        let mut prefs = Preferences::default();
        prefs.network.buffering = Buffering::Custom;
        prefs.network.custom_caching.network = 4000;
        prefs.performance.hardware_decoding = HardwareDecoding::Off;
        prefs.performance.skip_loop_filter = SkipLoopFilter::NonKey;
        let options = options(&prefs, None, true);
        assert_eq!(options.caching.network, 4000);
        assert!(!options.hardware_decoding);
        assert_eq!(options.skip_loop_filter, 3);
        assert!(options.rescan_plugins);
    }

    #[test]
    fn a_proxy_without_a_scheme_is_http() {
        let url = settings::proxy_url("me@proxy.example.com:8080").expect("an address");
        assert_eq!(url.as_str(), "http://me@proxy.example.com:8080/");
        assert!(settings::proxy_url("").is_none());
        assert!(settings::proxy_url("socks://proxy:1080").is_none());
    }
}
