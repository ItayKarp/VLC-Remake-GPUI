use gpui::{Keystroke, OperatingSystem};

use super::Shortcuts;
use crate::keymap::actions as act;
use crate::settings::{CustomBinding, KeymapSettings};

const OS: OperatingSystem = OperatingSystem::Linux;

fn key(source: &str) -> Keystroke {
    Keystroke::parse_with_os(source, OS).expect("test keys parse")
}

fn index(action: &dyn gpui::Action) -> usize {
    Shortcuts::index_of(action).expect("a command")
}

fn custom(entries: &[(&str, &[&str])]) -> KeymapSettings {
    let bindings = entries
        .iter()
        .map(|(command, keys)| {
            CustomBinding::new(*command, keys.iter().map(|key| (*key).to_owned()).collect())
        })
        .collect();
    KeymapSettings { bindings }
}

#[test]
fn defaults_have_no_conflicts_on_any_os() {
    for os in [OperatingSystem::Mac, OperatingSystem::Windows, OperatingSystem::Linux] {
        let shortcuts = Shortcuts::defaults(os);
        for (index, (command, keys)) in shortcuts.iter().enumerate() {
            for key in keys {
                let conflict = shortcuts.conflict(index, key);
                assert_eq!(conflict, None, "{} on {os:?}: {key:?}", command.label);
            }
        }
    }
}

#[test]
fn saving_keeps_only_changes_and_loading_reads_them_back() {
    let mut shortcuts = Shortcuts::defaults(OS);
    assert_eq!(shortcuts.to_settings(), KeymapSettings::default());
    let stop = index(&act::Stop);
    shortcuts.set(stop, vec![key("ctrl-k"), key("z")]);
    let saved = shortcuts.to_settings();
    assert_eq!(saved, custom(&[("player::Stop", &["ctrl-k", "z"])]));
    let (loaded, problems) = Shortcuts::load(&saved, OS);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(loaded, shortcuts);
    shortcuts.reset(stop);
    assert!(!shortcuts.is_customized());
}

#[test]
fn a_cleared_command_stays_cleared() {
    let mut shortcuts = Shortcuts::defaults(OS);
    shortcuts.set(index(&act::Stop), Vec::new());
    let (loaded, _) = Shortcuts::load(&shortcuts.to_settings(), OS);
    assert_eq!(loaded.keys(index(&act::Stop)), []);
}

#[test]
fn loading_keeps_the_valid_entries_and_reports_the_rest() {
    let hand_edited = custom(&[
        ("vlc::NoSuchCommand", &["k"]),
        ("player::Stop", &["z", "ctrl-shift", "alt-tab", "z"]),
        ("player::Stop", &["y"]),
        ("player::Snapshot", &["space"]),
    ]);
    let (loaded, problems) = Shortcuts::load(&hand_edited, OS);
    assert_eq!(loaded.keys(index(&act::Stop)), [key("z")]);
    assert!(loaded.keys(index(&act::Snapshot)).is_empty(), "Space is Play / pause's");
    assert_eq!(problems.len(), 5, "{problems:?}");
}

#[test]
fn a_command_keeps_its_own_reserved_default() {
    let mac = OperatingSystem::Mac;
    let (loaded, problems) = Shortcuts::load(&custom(&[("vlc::Quit", &["cmd-q"])]), mac);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(loaded.is_default(index(&act::Quit)));
}

#[test]
fn replacing_takes_the_key_from_the_other_command() {
    let mut shortcuts = Shortcuts::defaults(OS);
    let (stop, play) = (index(&act::Stop), index(&act::PlayPause));
    let space = key("space");
    assert_eq!(shortcuts.conflict(stop, &space).map(super::Conflict::label), Some("Play / pause"));
    shortcuts.take_from_others(stop, &space);
    shortcuts.set(stop, vec![space.clone()]);
    assert_eq!(shortcuts.keys(play), []);
    assert_eq!(shortcuts.conflict(stop, &space), None);
}

#[test]
fn the_player_and_the_library_may_share_a_key() {
    let shortcuts = Shortcuts::defaults(OS);
    let back = index(&act::GoBack);
    assert_eq!(shortcuts.conflict(back, &key("alt-left")), None, "Medium seek is the player's");
    let quit = index(&act::Quit);
    assert!(shortcuts.conflict(quit, &key("space")).is_some(), "global keys reach the player");
}
