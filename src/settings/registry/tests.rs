//! The registry's rules (S5.1): the table is well formed, dependencies chain, values are checked
//! and clamped, search folds accents and case, and restart settings are noticed.

use std::collections::HashSet;

use super::def::{Condition, Control};
use super::rules::{Invalid, trim};
use super::*;
use crate::settings::{AutoHide, Settings};

fn def(key: &str) -> &'static SettingDef {
    find(key).unwrap_or_else(|| panic!("no setting {key}"))
}

#[test]
fn keys_are_unique_and_every_row_has_its_words() {
    let mut keys = HashSet::new();
    for def in all() {
        assert!(keys.insert(def.key), "{} twice", def.key);
        assert!(!def.label.is_empty() && !def.section.is_empty(), "{}", def.key);
        assert!(def.info.ends_with('.'), "{} needs (i) text, a sentence", def.key);
    }
}

#[test]
fn dependencies_name_a_parent_and_a_value_it_can_have() {
    for def in all() {
        let Some(dependency) = def.dependency else { continue };
        let parent = def_or_fail(dependency.key, def.key);
        match dependency.condition {
            Condition::IsOn => assert!(matches!(parent.control, Control::Toggle), "{}", def.key),
            Condition::Is(id) | Condition::IsNot(id) => {
                assert!(parent.choices().iter().any(|c| c.id == id), "{}: no {id}", def.key);
            }
        }
    }
}

fn def_or_fail(key: &str, child: &str) -> &'static SettingDef {
    find(key).unwrap_or_else(|| panic!("{child} depends on missing {key}"))
}

#[test]
fn dependency_chains_end() {
    for start in all() {
        let mut row = start;
        let mut steps = 0;
        while let Some(dependency) = row.dependency {
            row = def(dependency.key);
            steps += 1;
            assert!(steps < 8, "{} depends on itself", start.key);
        }
    }
}

#[test]
fn defaults_are_valid_values() {
    for def in all() {
        let default = def.default_value();
        assert_eq!(def.accept(default.clone()), Ok(default.clone()), "{}", def.key);
        if let Value::Choice(id) = default {
            assert!(def.choices().iter().any(|c| c.id == id), "{}: default {id}", def.key);
        }
    }
}

#[test]
fn every_choice_round_trips() {
    for def in all() {
        for choice in def.choices() {
            let mut settings = Settings::default();
            (def.binding.set)(&mut settings, Value::Choice(choice.id));
            assert_eq!(def.value(&settings), Value::Choice(choice.id), "{}", def.key);
        }
    }
}

#[test]
fn every_number_round_trips_at_its_ends() {
    for def in all() {
        let Some(range) = def.range else { continue };
        for number in [range.min, range.max] {
            let mut settings = Settings::default();
            (def.binding.set)(&mut settings, Value::Number(number));
            let Value::Number(read) = def.value(&settings) else { panic!("{}", def.key) };
            assert!((read - number).abs() < 1e-6, "{}: {number} read back as {read}", def.key);
        }
    }
}

#[test]
fn a_disabled_parent_keeps_its_child_disabled() {
    let resume = def("player.resume");
    let mut settings = Settings::default();
    assert!(resume.is_enabled(&settings));
    settings.prefs.privacy.save_recently_played = false;
    // The direct parent is still on, but its own parent is off.
    assert!(settings.prefs.privacy.remember_playback_position);
    assert!(!def("privacy.remember_playback_position").is_enabled(&settings));
    assert!(!resume.is_enabled(&settings));
}

#[test]
fn a_choice_dependency_follows_the_parent() {
    let hide_after = def("interface.hide_controls_after_ms");
    let mut settings = Settings::default();
    assert!(hide_after.is_enabled(&settings));
    settings.prefs.interface.auto_hide_controls = AutoHide::Never;
    assert!(!hide_after.is_enabled(&settings));
    let fixed = def("audio.startup_volume_percent");
    assert!(!fixed.is_enabled(&Settings::default()), "only for a fixed startup volume");
}

#[test]
fn numbers_are_checked_and_snapped() {
    let step = def("audio.volume_step_percent");
    assert_eq!(step.accept(Value::Number(7.4)), Ok(Value::Number(7.)));
    assert_eq!(step.accept(Value::Number(0.)), Err(Invalid("Between 1% and 100%".into())));
    assert!(step.accept(Value::Number(f64::NAN)).is_err());
    let speed = def("player.default_speed");
    assert_eq!(speed.accept(Value::Number(1.27)), Ok(Value::Number(1.25)));
}

#[test]
fn half_typed_numbers_are_refused_and_units_are_optional() {
    let skip = def("controls.skip.short");
    for typed in ["", "-", "1.", "s", "ten"] {
        assert!(skip.parse_text(typed).is_err(), "{typed:?}");
    }
    assert_eq!(skip.parse_text("12 s"), Ok(Value::Number(12.)));
    assert_eq!(skip.parse_text(" 12s "), Ok(Value::Number(12.)));
    assert_eq!(skip.parse_text("1,5"), Ok(Value::Number(1.5)));
    assert_eq!(skip.format_value(&Value::Number(10.)), "10 s");
}

#[test]
fn lists_are_read_and_checked() {
    let languages = def("audio.preferred_languages");
    let typed = languages.parse_text(" en, , fra ").and_then(|value| languages.accept(value));
    assert_eq!(typed, Ok(Value::List(vec!["en".into(), "fra".into()])));
    assert!(languages.accept(Value::List(vec!["und".into()])).is_err());
    assert_eq!(languages.format_value(&Value::List(vec!["en".into(), "fr".into()])), "en, fr");
}

#[test]
fn values_from_other_versions_are_clamped() {
    let mut settings = Settings::default();
    settings.prefs.audio.max_volume_percent = 999;
    settings.prefs.interface.hide_controls_after_ms = 0;
    assert!(sanitize(&mut settings));
    assert_eq!(settings.prefs.audio.max_volume_percent, 300);
    assert_eq!(settings.prefs.interface.hide_controls_after_ms, 500);
    assert!(!sanitize(&mut settings), "already in range");
}

#[test]
fn search_matches_labels_info_and_keywords_without_accents_or_case() {
    let keys = |query| search(query).iter().map(|def| def.key).collect::<Vec<_>>();
    assert_eq!(keys("DARK MODE"), ["theme"]);
    assert!(keys("Résumé").contains(&"player.resume"));
    assert!(keys("headphones").contains(&"audio.pause_on_device_disconnect"));
    assert!(keys("receiver passthrough").contains(&"audio.passthrough"), "advanced rows too");
    assert_eq!(keys("   "), Vec::<&str>::new());
    assert_eq!(keys("no such setting"), Vec::<&str>::new());
}

#[test]
fn restart_settings_are_noticed_until_put_back() {
    let launch = Settings::default();
    let mut now = launch.clone();
    now.prefs.audio.volume_step_percent = 10;
    assert!(!needs_restart(&launch, &now), "a live setting");
    now.prefs.audio.passthrough = true;
    assert!(needs_restart(&launch, &now));
    now.prefs.audio.passthrough = false;
    assert!(!needs_restart(&launch, &now));
}

#[test]
fn colors_are_six_hex_digits() {
    assert_eq!(hex_color("#FFa500"), Some(0xFFA500));
    assert_eq!(hex_color("FFA500"), None);
    assert_eq!(hex_color("#FFF"), None);
    assert_eq!(trim(1.50), "1.5");
}

#[test]
fn every_category_but_hand_built_ones_lists_in_the_sidebar() {
    let titles: HashSet<_> = Category::ALL.iter().map(|c| c.title()).collect();
    assert_eq!(titles.len(), Category::ALL.len());
    assert_eq!(Category::Extensions.layout(), Layout::HandBuilt);
    assert!(all().all(|def| def.category.layout() == Layout::Rows));
}
