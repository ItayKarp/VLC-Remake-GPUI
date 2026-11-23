//! The Track and Language rows of the Audio and Subtitles tabs' Defaults (S7.6.1, S6.3.1,
//! `controls-audio_settings.png`, `controls-subtitle_settings.png`): **Audio Track** or
//! **Subtitle Track**, and **Audio Language** or **Subtitle Language**, for media opened next.
//! They are Settings → Playback's "Default audio/subtitle track" and the first "Preferred
//! language" of Settings → Audio or Subtitles. They aren't the track playing now, which the
//! track popover picks; the section says so under its heading. Unlike the panel's other values
//! (D24), these are the defaults themselves, saved to the preset in use as in Settings.
//!
//! The Track field takes a number from 1 to 99, or nothing for the media's own choice. The
//! Language dropdown puts the picked language first among the preferred ones; Automatic clears
//! them.

use gpui::{
    App, AppContext as _, Context, Div, Entity, KeyDownEvent, SharedString, Subscription, Window,
    div, prelude::*, px,
};

use super::SettingsPanel;
use crate::engine::language::{self, Language};
use crate::main_window::MainWindow;
use crate::settings::{self, Settings};
use crate::theme::Palette;
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::text_input::{self, TextInput, Validation};

/// The highest track number the field takes, as in Settings.
const MAX_TRACK: u32 = 99;
/// What the field shows for "the media's own choice", and the language list's first choice.
const AUTOMATIC: &str = "Automatic";
/// The rows' height, text size and control width.
const ROW_HEIGHT: f32 = 52.;
const ROW_TEXT: f32 = 14.;
const CONTROL_WIDTH: f32 = 140.;

/// Which tab's defaults the rows set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrackDefaults {
    Audio,
    Subtitle,
}

impl TrackDefaults {
    fn track_label(self) -> &'static str {
        match self {
            Self::Audio => "Audio Track",
            Self::Subtitle => "Subtitle Track",
        }
    }

    fn language_label(self) -> &'static str {
        match self {
            Self::Audio => "Audio Language",
            Self::Subtitle => "Subtitle Language",
        }
    }

    /// The default track number saved; 0 for the media's own choice.
    fn track(self, settings: &Settings) -> u32 {
        match self {
            Self::Audio => settings.prefs.player.default_audio_track,
            Self::Subtitle => settings.prefs.player.default_subtitle_track,
        }
    }

    fn set_track(self, settings: &mut Settings, track: u32) {
        match self {
            Self::Audio => settings.prefs.player.default_audio_track = track,
            Self::Subtitle => settings.prefs.player.default_subtitle_track = track,
        }
    }

    /// The preferred languages, most preferred first.
    fn languages(self, settings: &mut Settings) -> &mut Vec<String> {
        match self {
            Self::Audio => &mut settings.prefs.audio.preferred_languages,
            Self::Subtitle => &mut settings.prefs.subtitles.preferred_languages,
        }
    }

    /// The most preferred language, as saved.
    fn first_language(self, cx: &App) -> Option<String> {
        let prefs = &settings::get(cx).prefs;
        let languages = match self {
            Self::Audio => &prefs.audio.preferred_languages,
            Self::Subtitle => &prefs.subtitles.preferred_languages,
        };
        languages.first().cloned()
    }

    /// The Track field's text for what is saved.
    fn track_text(self, cx: &App) -> String {
        track_text(self.track(settings::get(cx)))
    }
}

/// The Track and Language rows' controls.
pub(super) struct DefaultsRows {
    kind: TrackDefaults,
    track: Entity<TextInput>,
    language: Entity<Dropdown>,
    /// The language codes behind the dropdown's choices after Automatic.
    codes: Vec<String>,
    _events: [Subscription; 2],
}

impl DefaultsRows {
    /// The controls, showing the settings.
    pub(super) fn new(
        kind: TrackDefaults,
        window: &mut Window,
        cx: &mut Context<MainWindow>,
    ) -> Self {
        let saved = kind.first_language(cx);
        let (codes, labels) = choices(saved.as_deref());
        let selected = selected(&codes, saved.as_deref());
        let language =
            cx.new(|cx| Dropdown::new(kind.language_label(), labels, selected, window, cx));
        let track = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(kind.track_text(cx))
                .with_placeholder(AUTOMATIC)
                .with_label(kind.track_label())
        });
        let picked = cx.subscribe(&language, move |this, _, event: &dropdown::Changed, cx| {
            this.on_default_language(kind, event.0, cx);
        });
        let typed = cx.subscribe(&track, move |_, field, event, cx| {
            MainWindow::on_default_track(kind, &field, event, cx);
        });
        Self { kind, track, language, codes, _events: [picked, typed] }
    }

    /// Shows the settings as they are now, changed here or elsewhere.
    pub(super) fn sync(&mut self, window: &Window, cx: &mut Context<MainWindow>) {
        let saved = self.kind.first_language(cx);
        let (codes, labels) = choices(saved.as_deref());
        let index = selected(&codes, saved.as_deref());
        if codes != self.codes {
            self.codes = codes;
            self.language.update(cx, |dropdown, cx| dropdown.set_options(labels, cx));
        }
        self.language.update(cx, |dropdown, cx| dropdown.set_selected(index, cx));
        if !self.track.read(cx).is_focused(window) {
            let text = self.kind.track_text(cx);
            if self.track.read(cx).text() != text {
                self.track.update(cx, |field, cx| field.set_text(text, cx));
            }
        }
    }

    /// The two rows.
    pub(super) fn render(&self, p: &Palette, cx: &mut Context<MainWindow>) -> [Div; 2] {
        let kind = self.kind;
        let track = div()
            .on_key_down(cx.listener(move |this, event, window, cx| {
                this.on_default_track_key(kind, event, window, cx);
            }))
            .child(self.track.clone());
        [
            labelled(kind.track_label(), track, p),
            labelled(kind.language_label(), self.language.clone(), p),
        ]
    }
}

impl SettingsPanel {
    /// The Track and Language rows of `kind`'s tab.
    fn track_defaults(&self, kind: TrackDefaults) -> &DefaultsRows {
        match kind {
            TrackDefaults::Audio => &self.audio.defaults,
            TrackDefaults::Subtitle => &self.subtitles.defaults,
        }
    }
}

impl MainWindow {
    fn on_default_language(&mut self, kind: TrackDefaults, index: usize, cx: &mut Context<Self>) {
        let rows = self.settings_panel.track_defaults(kind);
        let code = index.checked_sub(1).and_then(|index| rows.codes.get(index)).cloned();
        settings::update(cx, |s| {
            let languages = kind.languages(s);
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
        kind: TrackDefaults,
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
                settings::update(cx, |s| kind.set_track(s, track));
                field.update(cx, |field, cx| field.set_text(track_text(track), cx));
            }
            (text_input::Event::Submitted, None) => field.update(cx, |field, cx| {
                let why = "A track number from 1 to 99, or empty for the video's own";
                field.set_validation(Validation::Error(why.into()), cx);
            }),
            (text_input::Event::Blurred, None) => {
                let text = kind.track_text(cx);
                field.update(cx, |field, cx| {
                    field.set_text(text, cx);
                    field.set_validation(Validation::None, cx);
                });
            }
        }
    }

    /// Escape in the Track field puts the saved number back before it closes the panel.
    fn on_default_track_key(
        &mut self,
        kind: TrackDefaults,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "escape" || event.keystroke.modifiers.modified() {
            return;
        }
        let text = kind.track_text(cx);
        let field = self.settings_panel.track_defaults(kind).track.clone();
        if field.read(cx).text() != text || field.read(cx).is_invalid() {
            field.update(cx, |field, cx| {
                field.set_text(text, cx);
                field.set_validation(Validation::None, cx);
            });
            cx.stop_propagation();
        }
    }
}

/// A Defaults row: the label, and `control` in the right-hand column.
pub(super) fn labelled(label: &'static str, control: impl IntoElement, p: &Palette) -> Div {
    div()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .pl(px(4.))
        .text_size(px(ROW_TEXT))
        .text_color(p.text_muted)
        .child(label)
        .child(div().w(px(CONTROL_WIDTH)).flex_none().child(control))
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

    #[test]
    fn each_tab_sets_its_own_defaults() {
        let mut settings = Settings::default();
        TrackDefaults::Audio.set_track(&mut settings, 2);
        TrackDefaults::Audio.languages(&mut settings).push("fra".into());
        assert_eq!(settings.prefs.player.default_audio_track, 2);
        assert_eq!(settings.prefs.player.default_subtitle_track, 0);
        assert_eq!(settings.prefs.audio.preferred_languages, ["fra"]);
        assert_eq!(settings.prefs.subtitles.preferred_languages, Vec::<String>::new());
    }
}
