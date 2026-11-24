//! Network ("Network Connections & Security", `network.png`, S5.10.1): how much is buffered,
//! the HTTP proxy, and how TLS connections are checked. They are libVLC startup options, so
//! changes apply at the next launch.

use super::super::def::{ApplyMode, Condition, Confirm, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, round, text, toggle};
use crate::network::proxy_test;
use crate::privacy::keychain::Secret;
use crate::settings::registry::Category;
use crate::settings::{self, Buffering, CipherPreference, Settings};

const BUFFERING: (Category, &str) = (Category::Network, tr_noop!("Buffering"));
const PROXY: (Category, &str) = (Category::Network, tr_noop!("Proxy"));
const SECURITY: (Category, &str) = (Category::Network, tr_noop!("Security"));

/// The key of "Buffering", which the per-source times depend on.
const BUFFERING_KEY: &str = "network.buffering";
/// A buffering time's field.
const MILLISECONDS: Control = Control::Number { unit: tr_noop!("ms") };
/// The longest buffering time a field takes: a minute.
const MAX_CACHING_MS: f64 = 60_000.;

impl SettingChoice for Buffering {
    const VALUES: &'static [Self] =
        &[Self::Lowest, Self::Low, Self::Normal, Self::High, Self::Highest, Self::Custom];
    const CHOICES: &'static [Choice] = &[
        Choice::new("lowest", tr_noop!("Lowest latency")),
        Choice::new("low", tr_noop!("Low latency")),
        Choice::new("normal", tr_noop!("Normal")),
        Choice::new("high", tr_noop!("High")),
        Choice::new("highest", tr_noop!("Highest")),
        Choice::new("custom", tr_noop!("Custom")),
    ];
}

impl SettingChoice for CipherPreference {
    const VALUES: &'static [Self] =
        &[Self::Performance, Self::Balanced, Self::Secure128, Self::Secure256];
    const CHOICES: &'static [Choice] = &[
        Choice::new("performance", tr_noop!("Fastest")),
        Choice::new("balanced", tr_noop!("Balanced")),
        Choice::new("secure128", tr_noop!("Secure (128-bit)")),
        Choice::new("secure256", tr_noop!("Most secure (256-bit)")),
    ];
}

/// "Buffering": choosing Custom starts the custom times from the preset that was in use.
const BUFFERING_BINDING: Binding = Binding {
    get: |s| Value::Choice(s.prefs.network.buffering.choice_id()),
    set: |s, value| {
        let Value::Choice(id) = value else { return };
        let Some(buffering) = Buffering::from_choice_id(id) else { return };
        let network = &mut s.prefs.network;
        if buffering == Buffering::Custom && network.buffering != Buffering::Custom {
            network.custom_caching = network.caching();
        }
        network.buffering = buffering;
    },
};

/// A custom per-source time's binding.
macro_rules! caching {
    ($field:ident) => {
        Binding {
            get: |s: &Settings| Value::Number(f64::from(s.prefs.network.custom_caching.$field)),
            set: |s: &mut Settings, value| {
                if let Value::Number(ms) = value {
                    s.prefs.network.custom_caching.$field = round::<u32>(ms);
                }
            },
        }
    };
}

/// A per-source time's row.
const fn caching_row(
    key: &'static str,
    label: &'static str,
    info: &'static str,
    binding: Binding,
) -> SettingDef {
    SettingDef::new(key, BUFFERING, label, MILLISECONDS, binding)
        .info(info)
        .range(0., MAX_CACHING_MS, 10.)
        .depends_on(BUFFERING_KEY, Condition::Is("custom"))
        .applies(ApplyMode::Restart)
}

/// Asked before certificates stop being verified.
const STOP_VERIFYING: Confirm = Confirm {
    title: tr_noop!("Stop verifying certificates?"),
    detail: tr_noop!(
        "VLC will accept any certificate, so anyone between you and a server could pretend to be \
         it and see or change what you stream."
    ),
    button: tr_noop!("Stop Verifying"),
};

/// The warning under "Verify certificates" while it is off.
fn unverified_hint(cx: &gpui::App) -> Option<String> {
    (!settings::get(cx).prefs.network.verify_certificates).then(|| {
        tr!("Certificates aren't verified: secure streams can be read or changed on the way.")
            .to_owned()
    })
}

/// The Network page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        BUFFERING_KEY,
        BUFFERING,
        tr_noop!("Buffering"),
        Control::Dropdown(Buffering::CHOICES),
        BUFFERING_BINDING,
    )
    .info(tr_noop!(
        "How long VLC loads ahead before playing. Less starts sooner and keeps live streams closer \
         to live; more plays smoothly on a slow or unsteady network. Custom sets each kind of \
         source below."
    ))
    .applies(ApplyMode::Restart)
    .keywords(&["caching", "cache", "latency", "buffer", "stutter"]),
    caching_row(
        "network.custom_caching.local_files",
        tr_noop!("Local files"),
        tr_noop!("How long files on this computer buffer."),
        caching!(local_files),
    )
    .keywords(&["file caching", "cache"]),
    caching_row(
        "network.custom_caching.discs",
        tr_noop!("Discs"),
        tr_noop!("How long DVDs, Blu-rays and audio CDs buffer."),
        caching!(discs),
    )
    .keywords(&["disc caching", "dvd", "blu-ray", "cd"]),
    caching_row(
        "network.custom_caching.network",
        tr_noop!("Network streams"),
        tr_noop!("How long streams from the network buffer."),
        caching!(network),
    )
    .keywords(&["network caching", "stream", "url"]),
    caching_row(
        "network.custom_caching.live_capture",
        tr_noop!("Live capture"),
        tr_noop!("How long capture devices, such as a webcam or a TV card, buffer."),
        caching!(live_capture),
    )
    .keywords(&["live caching", "webcam", "capture card"]),
    SettingDef::new(
        "network.http_proxy",
        PROXY,
        tr_noop!("HTTP proxy"),
        Control::Text,
        text!(prefs.network.http_proxy),
    )
    .info(tr_noop!(
        "The proxy streams go through, such as proxy.example.com:8080. Leave it empty to use the \
         system's proxy settings. Test checks that the saved address answers as a proxy."
    ))
    .placeholder(tr_noop!("System proxy"))
    .validate(proxy_address)
    .side_button(tr_noop!("Test"), proxy_test::run)
    .hint(proxy_test::hint)
    .applies(ApplyMode::Restart)
    .keywords(&["proxy server", "http", "connection"]),
    SettingDef::new(
        "http_proxy_password",
        PROXY,
        tr_noop!("HTTP proxy password"),
        Control::Secret(Secret::ProxyPassword),
        Binding::ACTION,
    )
    .info(tr_noop!(
        "Kept in your system's keychain, never in VLC's settings file. Put the user name in the \
         proxy's address, as in me@proxy.example.com:8080."
    ))
    .applies(ApplyMode::Restart)
    .keywords(&["proxy", "login", "keychain", "credentials"]),
    SettingDef::new(
        "network.verify_certificates",
        SECURITY,
        tr_noop!("Verify certificates against the system trust store"),
        Control::Toggle,
        toggle!(prefs.network.verify_certificates),
    )
    .info(tr_noop!(
        "Checks that secure (https) streams come from who they say. Off, VLC accepts certificates \
         it can't verify. Update checks always verify."
    ))
    .confirm_off(STOP_VERIFYING)
    .danger_hint(unverified_hint)
    .applies(ApplyMode::Restart)
    .keywords(&["tls", "ssl", "https", "certificate", "trust"]),
    SettingDef::new(
        "network.ciphers",
        SECURITY,
        tr_noop!("Cipher preference"),
        Control::Dropdown(CipherPreference::CHOICES),
        choice!(prefs.network.ciphers),
    )
    .info(tr_noop!(
        "Which encryption secure connections may use. Stricter choices can fail with older \
         servers."
    ))
    .applies(ApplyMode::Restart)
    .keywords(&["tls", "ssl", "encryption", "gnutls"]),
];

/// Checks "HTTP proxy": empty, or an http address with a host, a scheme being optional. A
/// password belongs in the keychain, not in the address.
fn proxy_address(value: &Value) -> Result<(), String> {
    let Value::Text(text) = value else { return Ok(()) };
    if text.trim().is_empty() {
        return Ok(());
    }
    let Some(url) = settings::proxy_url(text) else {
        return Err(tr!("Enter an address such as proxy.example.com:8080").to_owned());
    };
    if url.password().is_some() {
        return Err(tr!("Put the password in HTTP proxy password, not in the address").to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::settings::registry::{ApplyMode, Value, find};
    use crate::settings::{Buffering, Settings};

    fn def(key: &str) -> &'static crate::settings::registry::SettingDef {
        find(key).unwrap_or_else(|| panic!("no setting {key}"))
    }

    #[test]
    fn the_source_times_show_the_preset_and_open_with_custom() {
        let mut settings = Settings::default();
        settings.prefs.network.buffering = Buffering::High;
        let network = def("network.custom_caching.network");
        assert!(!network.is_enabled(&settings));
        (def("network.buffering").binding.set)(&mut settings, Value::Choice("custom"));
        assert!(network.is_enabled(&settings));
        assert_eq!(network.value(&settings), Value::Number(3000.), "Custom starts from High");
        (network.binding.set)(&mut settings, Value::Number(4500.));
        assert_eq!(settings.prefs.network.caching().network, 4500);
        assert_eq!(network.apply, ApplyMode::Restart);
    }

    #[test]
    fn proxy_addresses_are_checked() {
        let proxy = def("network.http_proxy");
        for fine in ["", "proxy.example.com:8080", "http://me@proxy:3128/"] {
            assert!(proxy.accept(Value::Text(fine.into())).is_ok(), "{fine}");
        }
        let with_password = proxy.accept(Value::Text("http://me:secret@proxy:3128".into()));
        assert!(with_password.unwrap_err().0.contains("HTTP proxy password"));
        assert!(proxy.accept(Value::Text("socks://proxy:1080".into())).is_err());
    }

    #[test]
    fn turning_verification_off_is_confirmed_and_warned_about() {
        let verify = def("network.verify_certificates");
        assert!(verify.confirm_off.is_some());
        assert!(verify.hint.is_some());
        assert_eq!(verify.hint_tone, crate::settings::registry::HintTone::Danger);
    }
}
