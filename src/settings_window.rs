//! The Settings window (S5.2): a search field over a sidebar of categories, and the selected
//! category's page (`settings_view`) beside it under its title.
//!
//! - The sidebar groups the 13 categories under App, Media and System; the page shown last is
//!   remembered across launches (S5.2.2).
//! - Search filters every category at once (S5.2.3): the sidebar counts the matches on each page
//!   and dims the pages without any, the page shows only its matching rows with the words marked
//!   and Advanced Options opened when a match is in it, and when nothing matches anywhere an empty
//!   state says so.
//! - Advanced Options stays open or closed per page for the session (S5.2.4).
//! - Keys (S5.2.5): see [`keys`].
//! - A narrow window shows the sidebar as icons only; labels wrap beside their controls.
//!
//! Pages follow the settings as they change, so a value changed elsewhere (the volume popover,
//! the theme from the menu, the settings file) shows here at once. `app::open_settings` opens the
//! window, and brings it forward rather than opening a second one.

#[cfg(test)]
mod audio_screenshot;
mod chrome;
mod keys;
#[cfg(test)]
mod library_screenshot;
#[cfg(test)]
mod playback_screenshot;
#[cfg(test)]
mod screenshots;
mod search;
mod sidebar;
#[cfg(test)]
mod subtitles_screenshot;
#[cfg(test)]
mod system_screenshots;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod video_screenshot;

use std::collections::HashMap;

use gpui::{
    AnyElement, Context, Div, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription,
    Window, div, prelude::*, px,
};

use self::keys::{Dismiss, FocusSearch, NextCategory, PreviousCategory};
use self::search::Search;
use self::sidebar::Sidebar;
use crate::settings::{self, registry::Category};
use crate::settings_view::{PAGE_PADDING, PresetPicker, SettingsPage};
use crate::theme::{self, Palette};
use crate::ui::focus;
use crate::ui::text_input::{self, Look, TextInput};

pub use keys::bind_keys;

/// Below this window width the sidebar shows icons only.
const COMPACT_BELOW: f32 = 760.;
/// The search field's width in a narrow window.
const COMPACT_SEARCH_WIDTH: f32 = 200.;
/// The space above and below the preset picker at the foot of a page.
const PRESET_FOOTER_PADDING: f32 = 16.;

/// The Settings window's view.
pub struct SettingsWindow {
    category: Category,
    /// The pages shown so far; each keeps its own controls.
    pages: HashMap<Category, Entity<SettingsPage>>,
    query: Entity<TextInput>,
    search: Search,
    /// The preset picker at the foot of the pages presets cover (S5.11).
    presets: Entity<PresetPicker>,
    focus_handle: FocusHandle,
    _query_events: Subscription,
}

impl SettingsWindow {
    /// The window's view, showing `category`'s page with the keyboard on the sidebar.
    pub fn new(category: Category, window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        crate::app::save_geometry_on_change(window, cx, |s| &mut s.local.window.settings);
        let query = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_look(Look::Bare)
                .with_placeholder("Search")
                .with_label("Search settings")
        });
        let query_events = cx.subscribe_in(&query, window, |this, _, event, window, cx| {
            if *event == text_input::Event::Changed {
                this.search_changed(window, cx);
            }
        });
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let presets = cx.new(|cx| PresetPicker::new(window, cx));
        let mut this = Self {
            category,
            pages: HashMap::new(),
            query,
            search: Search::default(),
            presets,
            focus_handle,
            _query_events: query_events,
        };
        this.select(category, window, cx);
        this
    }

    /// Shows `category`'s page, answering the current search, and remembers it.
    pub fn select(&mut self, category: Category, window: &mut Window, cx: &mut Context<Self>) {
        self.category = category;
        let page = self
            .pages
            .entry(category)
            .or_insert_with(|| cx.new(|cx| SettingsPage::new(category, window, cx)))
            .clone();
        let words = self.search.words().to_vec();
        page.update(cx, |page, cx| page.set_search(words, cx));
        if settings::get(cx).local.settings_category != category {
            settings::update(cx, |settings| settings.local.settings_category = category);
        }
        cx.notify();
    }

    /// Selects `category` from the sidebar, leaving the keyboard there for Up and Down.
    fn choose(&mut self, category: Category, window: &mut Window, cx: &mut Context<Self>) {
        self.select(category, window, cx);
        self.focus_handle.focus(window, cx);
    }

    /// Runs the search as typed. When the current page has no match, the first page with one
    /// shows instead.
    fn search_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search = Search::new(self.query.read(cx).text());
        let category = if self.search.offers(self.category) {
            self.category
        } else {
            self.search.first_offered().unwrap_or(self.category)
        };
        self.select(category, window, cx);
    }

    /// Empties the search field and shows every row again.
    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, |query, cx| query.set_text("", cx));
        self.search_changed(window, cx);
    }

    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, TextInput::select_everything);
        self.query.focus_handle(cx).focus(window, cx);
    }

    fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(category) = self.search.step(self.category, forward) {
            self.select(category, window, cx);
        }
    }

    /// Escape: clears the search, or closes the window when there is none.
    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.query.read(cx).text().is_empty() {
            window.remove_window();
        } else {
            self.clear_search(window, cx);
        }
    }

    /// The sidebar beside the page, or the empty state when the search finds nothing.
    fn body(&self, compact: bool, p: &Palette, cx: &mut Context<Self>) -> Div {
        let width = if compact { sidebar::COMPACT_WIDTH } else { sidebar::WIDTH };
        let sidebar = Sidebar { current: self.category, search: &self.search, compact };
        let sidebar = div()
            .w(px(width))
            .flex_none()
            .flex()
            .flex_col()
            .py(px(chrome::MARGIN))
            .child(sidebar.render(p, cx));
        let footer = (self.category.has_presets() && !self.search.finds_nothing()).then(|| {
            div()
                .flex_none()
                .px(px(PAGE_PADDING))
                .py(px(PRESET_FOOTER_PADDING))
                .border_t_1()
                .border_color(p.divider)
                .child(self.presets.clone())
        });
        let page = div().flex_1().min_w_0().flex().flex_col().ml(px(chrome::MARGIN)).bg(p.sidebar);
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .pl(px(chrome::MARGIN))
            .child(sidebar)
            .child(page.child(div().flex_1().min_h_0().child(self.content(p, cx))).children(footer))
    }

    /// The selected page, or the empty state when the search finds nothing.
    fn content(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match self.pages.get(&self.category) {
            Some(_) if self.search.finds_nothing() => {
                chrome::no_matches(self.search.query(), p, cx)
            }
            Some(page) => page.clone().into_any_element(),
            None => div().into_any_element(),
        }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let compact = window.viewport_size().width < px(COMPACT_BELOW);
        let search_width = if compact { COMPACT_SEARCH_WIDTH } else { sidebar::WIDTH };
        let search = chrome::search_field(&self.query, &p);
        let top_row = chrome::top_row(search, search_width, self.category.title(), &p);
        let body = self.body(compact, &p, cx);
        let root = div()
            .id("settings-window")
            .key_context(keys::CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::focus_search))
            .on_action(cx.listener(|this, _: &PreviousCategory, window, cx| {
                this.step(false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextCategory, window, cx| {
                this.step(true, window, cx);
            }))
            .on_action(cx.listener(Self::dismiss))
            .size_full()
            .flex()
            .flex_col()
            .bg(p.sunken)
            .text_color(p.text);
        focus::tab_navigation(root)
            .children(chrome::traffic_light_strip())
            .child(top_row)
            .child(body)
    }
}
