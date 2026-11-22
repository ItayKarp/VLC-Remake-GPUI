//! The Subtitles tab's Defaults section (S6.3.1, `controls-subtitle_settings.png`): **Subtitle
//! Track** and **Subtitle Language** for media opened next, which are Settings → Playback's
//! "Default subtitle track" and Settings → Subtitles' first "Preferred language". They aren't
//! the subtitle showing now, which the track popover picks; the section says so under its
//! heading. Unlike the panel's other values (D24), these are the defaults themselves, saved to
//! the preset in use as in Settings.
//!
//! Subtitle Track takes a number from 1 to 99, or nothing for the media's own choice. Subtitle
//! Language puts the picked language first among the preferred ones; Automatic clears them.

use gpui::{AppContext as _, Context, Entity, KeyDownEvent, SharedString, Subscription, Window};

use crate::engine::language::{self, Language};
use crate::main_window::MainWindow;
use crate::settings;
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::text_input::{self, TextInput, Validation};

/// The highest track number the field takes, as in Settings.
const MAX_TRACK: u32 = 99;
/// What the field shows for "the media's own choice", and the language list's first choice.
const AUTOMATIC: &str = "Automatic";

/// The Defaults section's two controls.
pub(super) struct DefaultsRows {
    pub(super) track: Entity<TextInput>,
    pub(super) language: Entity<Dropdown>,
    /// The language codes behind the dropdown's choices after Automatic.
    codes: Vec<String>,
    _events: [Subscription; 2],
}

impl DefaultsRows {
    /// The controls, showing the settings.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let saved = first_language(cx);
        let (codes, labels) = choices(saved.as_deref());
        let selected = selected(&codes, saved.as_deref());
        let language =
            cx.new(|cx| Dropdown::new("Subtitle Language", labels, selected, window, cx));
        let track = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(track_text(settings::get(cx).prefs.player.default_subtitle_track))
                .with_placeholder(AUTOMATIC)
                .with_label("Subtitle Track")
        });
        let picked = cx.subscribe_in(&language, window, MainWindow::on_default_language);
        let typed = cx.subscribe(&track, |_, field, event, cx| {
            MainWindow::on_default_track(&field, event, cx);
        });
        Self { track, language, codes, _events: [picked, typed] }
    }

    /// Shows the settings as they are now, changed here or elsewhere.
    pub(super) fn sync(&mut self, window: &Window, cx: &mut Context<MainWindow>) {
        let saved = first_language(cx);
        let (codes, labels) = choices(saved.as_deref());
        let index = selected(&codes, saved.as_deref());
        if codes != self.codes {
            self.codes = codes;
            self.language.update(cx, |dropdown, cx| dropdown.set_options(labels, cx));
        }
        self.language.update(cx, |dropdown, cx| dropdown.set_selected(index, cx));
        if !self.track.read(cx).is_focused(window) {
            let text = track_text(settings::get(cx).prefs.player.default_subtitle_track);
            if self.track.read(cx).text() != text {
                self.track.update(cx, |field, cx| field.set_text(text, cx));
            }
        }
    }
}

impl MainWindow {
    fn on_default_language(
        &mut self,
        _: &Entity<Dropdown>,
        event: &dropdown::Changed,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = &self.settings_panel.subtitles.defaults;
        let code = event.0.checked_sub(1).and_then(|index| rows.codes.get(index)).cloned();
        settings::update(cx, |s| {
            let languages = &mut s.prefs.subtitles.preferred_languages;
            match code {
                Some(code) => {
                    languages.retain(|other| !same_language(other, &code));
                    languages.insert(0, code);
                }
                None => languages.clear(),
            }
        });
    }

    fn on_default_track(
        field: &Entity<TextInput>,
        event: &text_input::Event,
        cx: &mut Context<Self>,
    ) {
        let parsed = parse_track(field.read(cx).text());
        match (event, parsed) {
            (text_input::Event::Changed, _) => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            (text_input::Event::Submitted | text_input::Event::Blurred, Some(track)) => {
                settings::update(cx, |s| s.prefs.player.default_subtitle_track = track);
                field.update(cx, |field, cx| field.set_text(track_text(track), cx));
            }
            (text_input::Event::Submitted, None) => field.update(cx, |field, cx| {
                let why = "A track number from 1 to 99, or empty for the video's own";
                field.set_validation(Validation::Error(why.into()), cx);
            }),
            (text_input::Event::Blurred, None) => {
                let saved = settings::get(cx).prefs.player.default_subtitle_track;
                field.update(cx, |field, cx| {
                    field.set_text(track_text(saved), cx);
                    field.set_validation(Validation::None, cx);
                });
            }
        }
    }

    /// Escape in the track field puts the saved number back before it closes the panel.
    pub(super) fn on_default_track_key(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "escape" || event.keystroke.modifiers.modified() {
            return;
        }
        let text = track_text(settings::get(cx).prefs.player.default_subtitle_track);
        let field = self.settings_panel.subtitles.defaults.track.clone();
        if field.read(cx).text() != text || field.read(cx).is_invalid() {
            field.update(cx, |field, cx| {
                field.set_text(text, cx);
                field.set_validation(Validation::None, cx);
            });
            cx.stop_propagation();
        }
    }
}

/// The most preferred subtitle language, as saved.
fn first_language(cx: &gpui::App) -> Option<String> {
    settings::get(cx).prefs.subtitles.preferred_languages.first().cloned()
}

/// The dropdown's codes and labels: Automatic, every known language, and `saved` at the end
/// when it isn't one of them.
fn choices(saved: Option<&str>) -> (Vec<String>, Vec<SharedString>) {
    let mut codes: Vec<String> = language::known().iter().map(|l| l.code().to_owned()).collect();
    let mut labels: Vec<SharedString> = vec![AUTOMATIC.into()];
    labels.extend(language::known().iter().map(|l| SharedString::from(l.name())));
    if let Some(saved) = saved.filter(|saved| selected(&codes, Some(saved)) == 0) {
        codes.push(saved.to_owned());
        labels.push(Language::parse(saved).map_or_else(|| saved.to_owned(), |l| l.name()).into());
    }
    (codes, labels)
}

/// The dropdown's choice for `saved`: its language's place after Automatic, or Automatic.
fn selected(codes: &[String], saved: Option<&str>) -> usize {
    let Some(saved) = saved else { return 0 };
    codes.iter().position(|code| same_language(code, saved)).map_or(0, |index| index + 1)
}

/// Whether two language settings name the same language (`he` and `heb`).
fn same_language(a: &str, b: &str) -> bool {
    match (Language::parse(a), Language::parse(b)) {
        (Some(a), Some(b)) => a == b,
        _ => a.eq_ignore_ascii_case(b),
    }
}

/// The field's text for a saved track: its number, or nothing for the media's own choice.
fn track_text(track: u32) -> String {
    if track == 0 { String::new() } else { track.to_string() }
}

/// A typed track number; empty is the media's own choice (0).
fn parse_track(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case(AUTOMATIC) {
        return Some(0);
    }
    text.parse().ok().filter(|track| (1..=MAX_TRACK).contains(track))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_read_as_numbers_or_nothing() {
        assert_eq!(parse_track(""), Some(0));
        assert_eq!(parse_track(" 2 "), Some(2));
        assert_eq!(parse_track("0"), None);
        assert_eq!(parse_track("100"), None);
        assert_eq!(track_text(0), "");
    }

    #[test]
    fn a_saved_language_is_chosen_by_any_of_its_codes() {
        let (codes, labels) = choices(Some("heb"));
        assert_eq!(codes.len() + 1, labels.len(), "known languages need no extra choice");
        assert!(selected(&codes, Some("heb")) > 0);
        assert_eq!(selected(&codes, None), 0);
        let (codes, labels) = choices(Some("zzz"));
        assert_eq!(codes.len() + 1, labels.len());
        assert_eq!(selected(&codes, Some("zzz")), codes.len(), "an unknown one is listed last");
    }
}
