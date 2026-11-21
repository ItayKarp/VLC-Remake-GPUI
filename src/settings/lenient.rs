//! Field-level leniency: a bad value falls back to that field's default (S0.1.5).

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

/// `deserialize_with` helper: parse the field as any TOML value first (which can't fail
/// on well-formed TOML), then as `T`. If that fails, for example `accent = "magenta"`
/// written by a newer build, log a warning and use `T::default()`.
pub fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let value = toml::Value::deserialize(deserializer)?;
    Ok(parse_or_default(value))
}

pub fn parse_or_default<T: DeserializeOwned + Default>(value: toml::Value) -> T {
    match T::deserialize(value.clone()) {
        Ok(parsed) => parsed,
        Err(err) => {
            log::warn!(
                "settings: ignoring invalid {} value `{value}` ({}); using the default",
                short_type_name::<T>(),
                err.to_string().trim(),
            );
            T::default()
        }
    }
}

fn short_type_name<T>() -> &'static str {
    let name = std::any::type_name::<T>();
    name.rsplit("::").next().unwrap_or(name)
}
