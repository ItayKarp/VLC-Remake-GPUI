//! [`SystemOptions`] as the player instance's libVLC arguments, and what the worker reads of
//! them (whether to decode in hardware). Set once at startup ([`set`]); later calls are ignored,
//! since these options apply at the next launch.

use std::sync::OnceLock;

use crate::engine::system_options::{Ciphers, SystemOptions};

/// The options [`set`] set.
static SYSTEM: OnceLock<SystemOptions> = OnceLock::new();

/// Sets the options the player instance starts with; only the first call counts.
pub fn set(options: SystemOptions) {
    SYSTEM.set(options).ok();
}

/// The options in use: those [`set`], or the defaults.
pub(super) fn current() -> SystemOptions {
    SYSTEM.get().cloned().unwrap_or_default()
}

/// Whether the GPU may decode video.
pub(super) fn hardware_decoding() -> bool {
    SYSTEM.get().is_none_or(|options| options.hardware_decoding)
}

/// The arguments `options` add; defaults add nothing.
pub fn arguments(options: &SystemOptions) -> Vec<String> {
    let defaults = SystemOptions::default();
    let mut arguments = caching(options, &defaults);
    arguments.extend(network(options));
    arguments.extend(decoding(options));
    arguments.extend(timing(options, &defaults));
    arguments
}

fn caching(options: &SystemOptions, defaults: &SystemOptions) -> Vec<String> {
    let (want, usual) = (options.caching, defaults.caching);
    [
        ("file-caching", want.local_files, usual.local_files),
        ("disc-caching", want.discs, usual.discs),
        ("network-caching", want.network, usual.network),
        ("live-caching", want.live_capture, usual.live_capture),
    ]
    .into_iter()
    .filter(|(_, want, usual)| want != usual)
    .map(|(name, ms, _)| format!("--{name}={ms}"))
    .collect()
}

fn network(options: &SystemOptions) -> Vec<String> {
    let mut arguments = Vec::new();
    if let Some(proxy) = &options.proxy {
        arguments.push(format!("--http-proxy={}", proxy.url));
        if let Some(password) = &proxy.password {
            arguments.push(format!("--http-proxy-pwd={password}"));
        }
    }
    if options.ciphers != Ciphers::Normal {
        arguments.push(format!("--gnutls-priorities={}", gnutls_priorities(options.ciphers)));
    }
    arguments
}

/// GnuTLS's priority string for `ciphers`.
fn gnutls_priorities(ciphers: Ciphers) -> &'static str {
    match ciphers {
        Ciphers::Performance => "PERFORMANCE",
        Ciphers::Normal => "NORMAL",
        Ciphers::Secure128 => "SECURE128",
        Ciphers::Secure256 => "SECURE256",
    }
}

fn decoding(options: &SystemOptions) -> Vec<String> {
    let mut arguments = Vec::new();
    if options.post_processing > 0 {
        arguments.push(format!("--postproc-q={}", options.post_processing));
    }
    if options.skip_loop_filter > 0 {
        arguments.push(format!("--avcodec-skiploopfilter={}", options.skip_loop_filter));
    }
    if options.decoding_threads > 0 {
        arguments.push(format!("--avcodec-threads={}", options.decoding_threads));
    }
    if let Some(decoder) = options.decoder {
        arguments.push(format!("--codec={decoder},any"));
    }
    if let Some(encoder) = options.encoder {
        arguments.push(format!("--encoder={encoder},any"));
    }
    if options.rescan_plugins {
        arguments.push("--reset-plugins-cache".to_owned());
    }
    arguments
}

fn timing(options: &SystemOptions, defaults: &SystemOptions) -> Vec<String> {
    let mut arguments = Vec::new();
    if !options.drop_late_frames {
        arguments.extend(["--no-drop-late-frames".to_owned(), "--no-skip-frames".to_owned()]);
    }
    if options.real_time_priority {
        arguments.extend(real_time(options.priority_offset));
    }
    if let Some(on) = options.clock_synchronisation {
        arguments.push(format!("--clock-synchro={}", i32::from(on)));
    }
    if options.clock_jitter_ms != defaults.clock_jitter_ms {
        arguments.push(format!("--clock-jitter={}", options.clock_jitter_ms));
    }
    if options.clock_average != defaults.clock_average {
        arguments.push(format!("--cr-average={}", options.clock_average));
    }
    arguments
}

/// Real-time priority: Windows' high priority class, elsewhere real-time threads `offset` above
/// libVLC's usual priorities. macOS's libVLC has neither option, and refuses to start with one.
fn real_time(offset: i32) -> Vec<String> {
    if cfg!(target_os = "macos") {
        return Vec::new();
    }
    if cfg!(windows) {
        return vec!["--high-priority".to_owned()];
    }
    let mut arguments = vec!["--rt-priority".to_owned()];
    if offset != 0 {
        arguments.push(format!("--rt-offset={offset}"));
    }
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::system_options::{Caching, Proxy};

    #[test]
    fn defaults_add_nothing() {
        assert_eq!(arguments(&SystemOptions::default()), Vec::<String>::new());
    }

    #[test]
    fn changed_options_become_arguments() {
        let options = SystemOptions {
            caching: Caching { network: 3000, ..Caching::default() },
            proxy: Some(Proxy {
                url: "http://me@proxy:8080/".to_owned(),
                password: Some("p w".to_owned()),
            }),
            ciphers: Ciphers::Secure256,
            post_processing: 6,
            decoding_threads: 4,
            decoder: Some("avcodec"),
            drop_late_frames: false,
            clock_synchronisation: Some(false),
            clock_average: 80,
            ..SystemOptions::default()
        };
        assert_eq!(
            arguments(&options),
            [
                "--network-caching=3000",
                "--http-proxy=http://me@proxy:8080/",
                "--http-proxy-pwd=p w",
                "--gnutls-priorities=SECURE256",
                "--postproc-q=6",
                "--avcodec-threads=4",
                "--codec=avcodec,any",
                "--no-drop-late-frames",
                "--no-skip-frames",
                "--clock-synchro=0",
                "--cr-average=80",
            ]
        );
    }

    #[test]
    fn the_proxy_password_never_prints() {
        let proxy = Proxy { url: "http://proxy/".to_owned(), password: Some("secret".to_owned()) };
        assert!(!format!("{proxy:?}").contains("secret"));
    }
}
