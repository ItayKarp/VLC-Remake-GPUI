//! A page in a window: fields refuse half-typed values and put the saved one back, disabled rows
//! ignore writes, a `NextMedia` change is noted, and controls follow changes made elsewhere.

use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};

use super::SettingsPage;
use crate::settings::{
    self, StartupVolume,
    registry::{self, Category, Value},
};
use crate::ui::text_input::{self, TextInput};

fn open(
    category: Category,
    cx: &mut TestAppContext,
) -> (Entity<SettingsPage>, &mut VisualTestContext) {
    cx.update(|cx| {
        settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
    });
    let (page, cx) = cx.add_window_view(|window, cx| SettingsPage::new(category, window, cx));
    cx.run_until_parked();
    (page, cx)
}

fn field(page: &Entity<SettingsPage>, key: &str, cx: &mut VisualTestContext) -> Entity<TextInput> {
    page.read_with(cx, |page, _| page.widgets.field(key).cloned()).expect("a field row")
}

/// Puts `text` in the field and presses Enter.
fn submit(field: &Entity<TextInput>, text: &str, cx: &mut VisualTestContext) {
    field.update(cx, |field, cx| {
        field.set_text(text, cx);
        cx.emit(text_input::Event::Submitted);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_number_field_saves_good_values_and_refuses_half_typed_ones(cx: &mut TestAppContext) {
    let (page, cx) = open(Category::Audio, cx);
    let step = field(&page, "audio.volume_step_percent", cx);
    let saved = |cx: &mut VisualTestContext| {
        cx.read(|cx| settings::get(cx).prefs.audio.volume_step_percent)
    };

    submit(&step, "-", cx);
    assert!(step.read_with(cx, |field, _| field.is_invalid()));
    assert_eq!(saved(cx), 5);

    submit(&step, "500", cx);
    assert!(step.read_with(cx, |field, _| field.is_invalid()), "out of range");
    assert_eq!(saved(cx), 5);

    submit(&step, "12", cx);
    assert_eq!(saved(cx), 12);
    assert_eq!(step.read_with(cx, |field, _| field.text().to_owned()), "12%");

    // Leaving with a half-typed value drops it.
    step.update(cx, |field, cx| {
        field.set_text("1.", cx);
        cx.emit(text_input::Event::Blurred);
    });
    cx.run_until_parked();
    assert_eq!(saved(cx), 12);
    assert_eq!(step.read_with(cx, |field, _| field.text().to_owned()), "12%");
}

#[gpui::test]
fn a_disabled_row_ignores_writes_until_its_parent_allows_it(cx: &mut TestAppContext) {
    let (page, cx) = open(Category::Audio, cx);
    let fixed = registry::find("audio.startup_volume_percent").expect("a setting");
    page.update(cx, |page, cx| page.write(fixed, Value::Number(40.), cx));
    assert_eq!(cx.read(|cx| settings::get(cx).prefs.audio.startup_volume_percent), 100);

    cx.update(|_, cx| {
        settings::update(cx, |s| s.prefs.audio.startup_volume = StartupVolume::Fixed);
    });
    page.update(cx, |page, cx| page.write(fixed, Value::Number(40.), cx));
    assert_eq!(cx.read(|cx| settings::get(cx).prefs.audio.startup_volume_percent), 40);
}

#[gpui::test]
fn a_next_media_change_is_noted_and_outside_changes_show(cx: &mut TestAppContext) {
    let (page, cx) = open(Category::Subtitles, cx);
    let enabled = registry::find("subtitles.enabled").expect("a setting");
    page.update(cx, |page, cx| page.write(enabled, Value::Bool(false), cx));
    assert!(page.read_with(cx, |page, _| page.edited.contains("subtitles.enabled")));

    let languages = field(&page, "subtitles.preferred_languages", cx);
    cx.update(|_, cx| {
        settings::update(cx, |s| s.prefs.subtitles.preferred_languages = vec!["he".into()]);
    });
    cx.run_until_parked();
    assert_eq!(languages.read_with(cx, |field, _| field.text().to_owned()), "he");
}

#[gpui::test]
fn a_search_shows_matching_rows_and_opens_advanced_options_for_them(cx: &mut TestAppContext) {
    let (page, cx) = open(Category::Audio, cx);
    let open_now = |page: &Entity<SettingsPage>, cx: &mut VisualTestContext| {
        page.update(cx, |page, cx| page.is_advanced_open(true, cx))
    };
    assert!(!open_now(&page, cx), "closed until opened");

    page.update(cx, |page, cx| page.set_search(registry::search_words("passthrough"), cx));
    assert!(open_now(&page, cx), "a match inside opens it");
    let shown: Vec<_> = page.read_with(cx, |page, _| page.shown_rows().map(|d| d.key).collect());
    assert_eq!(shown, ["audio.passthrough"]);

    // Closing it by hand during a search isn't remembered once the search ends.
    page.update(cx, |page, cx| page.set_advanced_open(false, cx));
    assert!(!open_now(&page, cx));
    page.update(cx, |page, cx| page.set_search(Vec::new(), cx));
    page.update(cx, |page, cx| page.set_advanced_open(true, cx));
    let reopened =
        cx.update(|window, cx| cx.new(|cx| SettingsPage::new(Category::Audio, window, cx)));
    assert!(open_now(&reopened, cx), "remembered for the session");
}

#[gpui::test]
fn privacy_patterns_and_the_submission_url_are_checked_before_saving(cx: &mut TestAppContext) {
    let (page, cx) = open(Category::Privacy, cx);
    let patterns = field(&page, "privacy.never_remember", cx);
    let saved = |cx: &mut VisualTestContext| {
        cx.read(|cx| settings::get(cx).prefs.privacy.never_remember.clone())
    };

    submit(&patterns, "*.part, ep[1-", cx);
    assert!(patterns.read_with(cx, |field, _| field.is_invalid()), "a broken set");
    assert!(saved(cx).is_empty(), "no part of the list applies");

    submit(&patterns, "*.part,  ~/Private/* ,", cx);
    assert_eq!(saved(cx), ["*.part", "~/Private/*"]);
    assert_eq!(patterns.read_with(cx, |field, _| field.text().to_owned()), "*.part, ~/Private/*");

    // The last.fm rows wait for their toggle.
    let url = field(&page, "privacy.lastfm.submission_url", cx);
    let submit_on = registry::find("privacy.lastfm.submit").expect("a setting");
    page.update(cx, |page, cx| page.write(submit_on, Value::Bool(true), cx));
    cx.run_until_parked();
    submit(&url, "ftp://example.com", cx);
    assert!(url.read_with(cx, |field, _| field.is_invalid()));
    submit(&url, "https://libre.fm/2.0/", cx);
    let saved_url = cx.read(|cx| settings::get(cx).prefs.privacy.lastfm.submission_url.clone());
    assert_eq!(saved_url, "https://libre.fm/2.0/");
}
