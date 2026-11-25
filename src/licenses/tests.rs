use std::collections::HashSet;

use super::*;

const LOCK: &str = include_str!("../../Cargo.lock");
const MANIFEST: &str = include_str!("../../Cargo.toml");
const REGENERATE: &str = "run tools/licenses/generate.sh";

/// The `<li>name version</li>` lines naming the crates under each license.
fn listed_crates() -> HashSet<(String, String)> {
    PAGE.lines()
        .filter_map(|line| line.trim().strip_prefix("<li>")?.strip_suffix("</li>"))
        .filter(|item| !item.contains('<'))
        .filter_map(|item| item.split_once(' '))
        .map(|(name, version)| (name.to_owned(), version.to_owned()))
        .collect()
}

fn locked_crates() -> HashSet<(String, String)> {
    let lock: toml::Table = toml::from_str(LOCK).unwrap();
    lock["package"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| (package["name"].as_str(), package["version"].as_str()))
        .map(|(name, version)| (name.unwrap().to_owned(), version.unwrap().to_owned()))
        .collect()
}

/// The packages the app depends on directly, on every OS, without dev-dependencies.
fn direct_dependencies() -> Vec<String> {
    let manifest: toml::Table = toml::from_str(MANIFEST).unwrap();
    let targets = manifest["target"].as_table().unwrap().values();
    let tables = std::iter::once(&manifest["dependencies"])
        .chain(targets.filter_map(|target| target.get("dependencies")));
    let mut names = Vec::new();
    for table in tables {
        for (key, value) in table.as_table().unwrap() {
            let package = value.get("package").and_then(toml::Value::as_str);
            names.push(package.unwrap_or(key).to_owned());
        }
    }
    names
}

#[test]
fn lists_only_crates_at_their_locked_versions() {
    let listed = listed_crates();
    assert!(listed.len() > 100, "the page lists {} crates", listed.len());
    let locked = locked_crates();
    let stale: Vec<_> = listed.difference(&locked).collect();
    assert!(stale.is_empty(), "not in Cargo.lock: {stale:?}; {REGENERATE}");
}

#[test]
fn lists_every_direct_dependency() {
    let listed: HashSet<String> = listed_crates().into_iter().map(|(name, _)| name).collect();
    let missing: Vec<_> =
        direct_dependencies().into_iter().filter(|name| !listed.contains(name)).collect();
    assert!(missing.is_empty(), "not listed: {missing:?}; {REGENERATE}");
}

#[test]
fn carries_the_libvlc_notice_and_its_license_texts() {
    assert!(!PAGE.contains("@LIBVLC_NOTICE@") && !PAGE.contains("@LGPL@"), "{REGENERATE}");
    assert!(PAGE.contains("<h2>libVLC</h2>"));
    assert!(PAGE.contains("GNU LESSER GENERAL PUBLIC LICENSE\n\nVersion 2.1, February 1999"));
    assert!(PAGE.contains("GNU GENERAL PUBLIC LICENSE\nVersion 2, June 1991"));
}

#[test]
fn writes_the_page_and_writes_it_again_when_it_differs() {
    let dir = std::env::temp_dir().join(format!("rvlc-licenses-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = write_page(&dir).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), PAGE);
    std::fs::write(&path, "old").unwrap();
    assert_eq!(write_page(&dir).unwrap(), path);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), PAGE);
    std::fs::remove_dir_all(&dir).unwrap();
}
