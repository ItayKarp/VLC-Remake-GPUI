//! Tests for the command table, the context rules and the shortcut labels, plus
//! headless window tests that check which keys reach the player.

mod dispatch;
mod layouts;

use gpui::{Keystroke, OperatingSystem};

use super::commands::{Area, COMMANDS, find};
use super::predicate;
use super::{actions as act, label};

const OSES: [OperatingSystem; 3] =
    [OperatingSystem::Mac, OperatingSystem::Windows, OperatingSystem::Linux];

fn parse(source: &str, os: OperatingSystem) -> Keystroke {
    Keystroke::parse_with_os(source, os).expect("default bindings parse")
}

#[test]
fn every_default_binding_parses_with_a_valid_predicate() {
    for os in OSES {
        for command in COMMANDS {
            let keystrokes = command.keys.for_os(os == OperatingSystem::Mac).iter();
            let predicates = keystrokes.filter_map(|s| predicate(command.area, &parse(s, os)));
            for p in predicates {
                gpui::KeyBindingContextPredicate::parse(&p).expect("predicates parse");
            }
        }
    }
}

#[test]
fn no_two_commands_share_a_key_in_the_same_area() {
    for os in OSES {
        let mut seen: Vec<(Area, Keystroke, &str)> = Vec::new();
        for command in COMMANDS {
            for source in command.keys.for_os(os == OperatingSystem::Mac) {
                let keystroke = parse(source, os);
                let clash =
                    seen.iter().find(|(area, k, _)| *area == command.area && *k == keystroke);
                assert!(clash.is_none(), "{os:?}: {source} is bound twice: {clash:?}");
                seen.push((command.area, keystroke, command.label));
            }
        }
    }
}

#[test]
fn every_command_is_found_by_its_action() {
    for command in COMMANDS {
        let found = find(command.action).expect("each action is in the table");
        assert_eq!(found.label, command.label);
    }
}

#[test]
fn d3_defaults() {
    let primary = |action: &dyn gpui::Action, os| {
        let command = find(action).expect("command exists");
        command.keys.for_os(os == OperatingSystem::Mac).first().map(|s| parse(s, os))
    };
    let win = OperatingSystem::Windows;
    let mac = OperatingSystem::Mac;
    assert_eq!(primary(&act::NextItem, win), Some(parse("n", win)));
    assert_eq!(primary(&act::PreviousItem, win), Some(parse("p", win)));
    assert_eq!(primary(&act::Record, win), Some(parse("shift-r", win)));
    assert_eq!(primary(&act::OpenFolder, win), Some(parse("ctrl-shift-o", win)));
    assert_eq!(primary(&act::OpenCaptureDevice, win), Some(parse("ctrl-shift-c", win)));
    assert_eq!(primary(&act::OpenSettings, win), Some(parse("ctrl-p", win)));
    assert_eq!(primary(&act::OpenSettings, mac), Some(parse("cmd-,", mac)));
    assert_eq!(primary(&act::OpenFile, mac), Some(parse("cmd-o", mac)));
}

#[test]
fn no_default_takes_os_or_editing_shortcuts() {
    // Copy stays Copy, and Ctrl+F stays the library's Search. macOS Hide and
    // Minimize stay the system's.
    let reserved = ["ctrl-c", "ctrl-f", "secondary-c", "secondary-f"];
    let mac_reserved = ["cmd-h", "cmd-m", "cmd-c"];
    for command in COMMANDS.iter().filter(|c| c.area != Area::Library) {
        for source in reserved {
            assert!(
                !command.keys.for_os(false).contains(&source),
                "{} uses {source}",
                command.label
            );
        }
        for source in mac_reserved {
            assert!(
                !command.keys.for_os(true).contains(&source),
                "{} uses {source}",
                command.label
            );
        }
    }
}

#[test]
fn predicates_follow_the_key_kind() {
    let os = OperatingSystem::Windows;
    let player = |s| predicate(Area::Player, &parse(s, os)).expect("player bindings have one");
    assert_eq!(player("f"), "Player && !Modal && !Typing");
    assert_eq!(player("shift-s"), "Player && !Modal && !Typing");
    assert_eq!(player("space"), "Player && !Modal && !Control");
    assert_eq!(player("left"), "Player && !Modal && !Control");
    assert_eq!(player("ctrl-\\"), "Player && !Modal");
    assert_eq!(
        predicate(Area::App, &parse("shift-f1", os)).as_deref(),
        Some("MainWindow && !Modal")
    );
    assert_eq!(predicate(Area::Global, &parse("ctrl-q", os)), None);
}

#[test]
fn labels_use_the_platform_spelling() {
    assert_eq!(label::text(&parse("secondary-o", OperatingSystem::Windows), false), "Ctrl + O");
    assert_eq!(label::text(&parse("secondary-o", OperatingSystem::Mac), true), "⌘O");
}
