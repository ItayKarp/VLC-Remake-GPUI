//! Turning typed settings back into a document without losing what we don't own
//! (S0.1.6): keys this build doesn't know (written by a newer build) and comments or
//! formatting the user added by hand.

use serde::Serialize;
use toml_edit::{DocumentMut, Item, TableLike, Value};

/// Serializes `typed` and merges it into `doc` in place.
pub fn merge_typed<T: Serialize>(doc: &mut DocumentMut, typed: &T) -> Result<(), String> {
    let text = toml::to_string(typed).map_err(|err| err.to_string())?;
    let fresh: DocumentMut = text
        .parse()
        .map_err(|err: toml_edit::TomlError| err.to_string())?;
    merge(doc.as_table_mut(), fresh.as_table());
    Ok(())
}

/// Writes every key of `new` into `old`. Keys only in `old` are kept. A value that
/// didn't change keeps its original formatting; a changed one keeps its comments.
pub fn merge(old: &mut dyn TableLike, new: &dyn TableLike) {
    for (key, new_item) in new.iter() {
        let Some(old_item) = old.get_mut(key) else {
            old.insert(key, new_item.clone());
            continue;
        };
        if let (Some(old_table), Some(new_table)) =
            (old_item.as_table_like_mut(), new_item.as_table_like())
        {
            merge(old_table, new_table);
            continue;
        }
        match (old_item, new_item) {
            (Item::Value(old_value), Item::Value(new_value)) => {
                if !same_value(old_value, new_value) {
                    let decor = old_value.decor().clone();
                    *old_value = new_value.clone();
                    *old_value.decor_mut() = decor;
                }
            }
            (old_item, new_item) => *old_item = new_item.clone(),
        }
    }
}

/// Compares semantically: `'a'` and `"a"` are the same string.
fn same_value(a: &Value, b: &Value) -> bool {
    let normalize = |value: &Value| {
        let mut doc = DocumentMut::new();
        doc["v"] = Item::Value(value.clone());
        toml::from_str::<toml::Table>(&doc.to_string()).ok()
    };
    match (normalize(a), normalize(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// The document's `schema_version`, or `0` if it has none (files from the first
/// onboarding build).
pub fn schema_version(doc: &DocumentMut) -> i64 {
    doc.get("schema_version")
        .and_then(Item::as_integer)
        .unwrap_or(0)
}

/// Sets `schema_version`, as the first key when it's new.
pub fn set_schema_version(doc: &mut DocumentMut, version: i64) {
    if let Some(value) = doc.get_mut("schema_version").and_then(Item::as_value_mut) {
        if value.as_integer() != Some(version) {
            let decor = value.decor().clone();
            *value = version.into();
            *value.decor_mut() = decor;
        }
        return;
    }
    let mut fresh = DocumentMut::new();
    fresh["schema_version"] = toml_edit::value(version);
    for (key, _) in doc.iter() {
        if let Some((key, item)) = doc.get_key_value(key) {
            fresh.insert_formatted(key, item.clone());
        }
    }
    *fresh.decor_mut() = doc.decor().clone();
    fresh.set_trailing(doc.trailing().clone());
    *doc = fresh;
}
