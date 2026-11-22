//! The header's search field (S4.7.5): Search opens a row under the count with a text field
//! and an Everywhere toggle. Not designed; it follows the panel's header metrics.
//!
//! Typing searches after [`DEBOUNCE`] without a keystroke, or at once on Enter, and shows the
//! results as a search route. Refining the query replaces that route rather than stacking one
//! per keystroke, and emptying the field goes back to where the search started. The listing
//! only ever shows the latest query's results: an older read is dropped when a newer one
//! starts. The scope is the section the search started in (Videos or Music) until Everywhere is
//! on; elsewhere a search always looks everywhere. Escape closes the field and leaves the
//! results; Search closes it too. Full library mode shows a search box in its header instead of
//! the Search button, which opens the field in its place.

use std::time::Duration;

use gpui::{
    ClickEvent, Context, Div, Entity, Focusable, KeyDownEvent, Subscription, Task, Window, div,
    prelude::*, px,
};

use super::header::GAP;
use super::{INSET, MainWindow};
use crate::assets::icons;
use crate::library::search::{self, Query};
use crate::navigation::history::Step;
use crate::navigation::route::{Route, Search, Section};
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::icon::icon;
use crate::ui::metrics;
use crate::ui::text_input::{self, TextInput};

/// How long typing must pause before the search runs.
const DEBOUNCE: Duration = Duration::from_millis(150);
/// The row's gap above it and its height.
const ROW_TOP: f32 = 8.;
const ROW_HEIGHT: f32 = 32.;
/// Full library mode's search box: its width, its side padding and its icon (`player_03`).
const BOX_WIDTH: f32 = 325.;
const BOX_PAD: f32 = 10.;
const BOX_ICON: f32 = 18.;

/// The open search field.
pub(super) struct SearchField {
    input: Entity<TextInput>,
    /// The section searched while Everywhere is off: Videos or Music, else Home (everything).
    section: Section,
    everywhere: bool,
    /// The search waiting for typing to pause.
    pending: Option<Task<()>>,
    _events: Subscription,
}

impl SearchField {
    /// Where the search looks: [`Section::Home`] for everywhere.
    fn scope(&self) -> Section {
        if self.everywhere { Section::Home } else { self.section }
    }
}

impl MainWindow {
    /// Opens the search field, or closes it when open.
    pub(super) fn toggle_library_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library_panel.search.is_some() {
            self.close_library_search(window, cx);
        } else {
            self.open_library_search(window, cx);
        }
    }

    pub(super) fn open_library_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (section, query) = match self.library_panel.history.current() {
            Route::Search(search) => (search.scope, search.query.clone()),
            route => (route.section(), String::new()),
        };
        let section = match section {
            Section::Videos | Section::Music => section,
            _ => Section::Home,
        };
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(query)
                .with_all_selected()
                .with_placeholder("Search titles, files, artists and albums")
                .with_label("Search the library")
                .with_max_len(search::MAX_QUERY_CHARS)
        });
        let events = cx.subscribe_in(&input, window, Self::on_search_input);
        input.read(cx).focus_handle(cx).focus(window, cx);
        let field =
            SearchField { input, section, everywhere: false, pending: None, _events: events };
        self.library_panel.search = Some(field);
        cx.notify();
    }

    /// Closes the search field, leaving whatever it shows; the listing takes the keyboard.
    fn close_library_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.library_panel.search = None;
        self.library_panel.list_focus.focus(window, cx);
        cx.notify();
    }

    fn on_search_input(
        &mut self,
        _: &Entity<TextInput>,
        event: &text_input::Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            text_input::Event::Changed => self.search_soon(cx),
            text_input::Event::Submitted => {
                self.run_library_search(cx);
                self.library_panel.list_focus.focus(window, cx);
            }
            text_input::Event::Blurred => {}
        }
    }

    /// Runs the search once typing pauses for [`DEBOUNCE`].
    fn search_soon(&mut self, cx: &mut Context<Self>) {
        let Some(field) = &mut self.library_panel.search else { return };
        field.pending = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            this.update(cx, Self::run_library_search).ok();
        }));
    }

    /// Shows what the field's query finds; an empty query goes back to where the search began.
    pub(super) fn run_library_search(&mut self, cx: &mut Context<Self>) {
        let Some(field) = &mut self.library_panel.search else { return };
        field.pending = None;
        let query = search::clean(field.input.read(cx).text());
        let scope = field.scope();
        let searching = matches!(self.library_panel.history.current(), Route::Search(_));
        if Query::parse(&query).is_none() {
            if searching {
                self.step_library(Step::Back, cx);
            }
            return;
        }
        let route = Route::Search(Search { query, scope });
        let history = &mut self.library_panel.history;
        let moved = if searching { history.replace(route) } else { history.go(route) };
        if moved {
            self.arrived(cx);
        }
    }

    /// Searches the current query everywhere (the empty results' button), keeping the field's
    /// toggle in step.
    pub(super) fn search_everywhere(&mut self, cx: &mut Context<Self>) {
        let Route::Search(search) = self.library_panel.history.current() else { return };
        let route = Route::Search(Search { query: search.query.clone(), scope: Section::Home });
        if let Some(field) = &mut self.library_panel.search {
            field.everywhere = true;
        }
        if self.library_panel.history.replace(route) {
            self.arrived(cx);
        }
    }

    /// Searches everywhere, or only the section the search started in.
    fn toggle_everywhere(&mut self, cx: &mut Context<Self>) {
        if let Some(field) = &mut self.library_panel.search {
            field.everywhere = !field.everywhere;
            self.run_library_search(cx);
            cx.notify();
        }
    }

    /// The field's row under the count, while it is open.
    pub(super) fn render_search_field(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let field = self.library_panel.search.as_ref()?;
        let row = search_row(field, cx).h(px(ROW_HEIGHT)).mt(px(ROW_TOP)).px(px(INSET));
        Some(row.text_color(p.text))
    }

    /// Full library mode's search box in the header (`player_03`): the field while open, else
    /// a box that opens it.
    pub(super) fn render_search_box(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let box_ = div().w(px(BOX_WIDTH)).flex_none().h(px(ROW_HEIGHT)).text_color(p.text);
        if let Some(field) = &self.library_panel.search {
            return box_.child(search_row(field, cx).size_full());
        }
        let hover = p.hover;
        let closed = div()
            .id("library-search-box")
            .size_full()
            .px(px(BOX_PAD))
            .flex()
            .items_center()
            .gap(px(GAP))
            .rounded(px(metrics::RADIUS))
            .bg(p.control)
            .cursor_text()
            .hover(move |style| style.bg(hover))
            .text_color(p.text_muted)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.open_library_search(window, cx);
            }))
            .child(icon(icons::SEARCH, BOX_ICON, p.icon))
            .child("Search");
        box_.child(closed)
    }
}

/// The open field and, searching Videos or Music, Everywhere; Escape closes it.
fn search_row(field: &SearchField, cx: &mut Context<MainWindow>) -> Div {
    let row = div()
        .flex()
        .items_center()
        .gap(px(GAP))
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                this.close_library_search(window, cx);
                cx.stop_propagation();
            }
        }))
        .child(div().flex_1().min_w_0().child(field.input.clone()));
    match field.section {
        Section::Home => row,
        section => row.child(everywhere_button(field.everywhere, section, cx)),
    }
}

/// Everywhere, accent filled while on.
fn everywhere_button(on: bool, section: Section, cx: &mut Context<MainWindow>) -> Button {
    let style = if on { ButtonStyle::Primary } else { ButtonStyle::Subtle };
    let tip = if on {
        format!("Searching the whole library. Click to search only {}.", section.label())
    } else {
        format!("Searching {}. Click to search the whole library.", section.label())
    };
    Button::new("library-search-everywhere", "Everywhere")
        .style(style)
        .tooltip(tip, None::<&str>)
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_everywhere(cx)))
}
