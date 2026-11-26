//! The preset picker (S5.11, `setting_preset-default.png`, `setting_preset-open.png`): a
//! dropdown at the foot of the Playback, Video, Audio and Subtitles pages, and of the player's
//! settings panel, naming the preset in use. It opens upward onto **+ New Preset**, **Import
//! Preset…** (not in the design), each custom preset with a `···` menu (Rename, Duplicate,
//! Export…, Delete…), then **Default Preset**, the one in use ticked.
//!
//! While a custom preset is in use a line over the dropdown says changes go to it. A new preset
//! and Rename edit the name in place: Enter or leaving the field keeps it, Escape doesn't. What
//! an import or export did shows under the dropdown.
//!
//! Keys: Enter, Space or Up opens it; Up and Down move through the rows, Enter picks, Right (or
//! the menu key) opens a preset's `···` menu, and Escape closes.

mod actions;
mod render;

use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, FocusHandle, Focusable, Pixels, Point,
    SharedString, Subscription, Window,
};

use crate::settings::{self, SettingsStore, presets};
use crate::theme;
use crate::ui::menu::ContextMenu;
use crate::ui::popover::{self, BoundsTracker};
use crate::ui::text_input::{self, TextInput};

/// One row of the open list.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Row {
    /// "+ New Preset".
    New,
    /// "Import Preset…".
    Import,
    /// A custom preset, by name.
    Custom(SharedString),
    /// "Default Preset".
    Default,
}

/// A preset's name being edited in place.
struct Renaming {
    /// The name it had.
    name: SharedString,
    input: Entity<TextInput>,
    _events: Subscription,
}

/// A line under the dropdown saying what an action did.
#[derive(Clone, Debug)]
struct Status {
    text: SharedString,
    failed: bool,
}

/// The preset dropdown; it reads and changes the presets through `settings::presets`.
pub struct PresetPicker {
    open: bool,
    highlighted: Option<usize>,
    focus_handle: FocusHandle,
    trigger: BoundsTracker,
    list: BoundsTracker,
    renaming: Option<Renaming>,
    /// A preset's `···` menu, where it opened.
    menu: Option<(Point<Pixels>, Entity<ContextMenu>, Subscription)>,
    status: Option<Status>,
}

impl Focusable for PresetPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl PresetPicker {
    /// A closed picker that follows the presets as they change anywhere.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        cx.observe_global::<SettingsStore>(|_, cx| cx.notify()).detach();
        popover::close_on_deactivate(window, cx, |this: &mut Self, _, cx| this.close(cx));
        Self {
            open: false,
            highlighted: None,
            focus_handle: cx.focus_handle().tab_stop(true),
            trigger: BoundsTracker::new(),
            list: BoundsTracker::new(),
            renaming: None,
            menu: None,
            status: None,
        }
    }

    /// The rows of the open list, in order.
    fn rows(cx: &App) -> Vec<Row> {
        let custom = settings::get(cx).prefs.presets.list.iter();
        let custom = custom.map(|preset| Row::Custom(preset.name.clone().into()));
        [Row::New, Row::Import].into_iter().chain(custom).chain([Row::Default]).collect()
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(cx);
        } else {
            self.focus_handle.focus(window, cx);
            self.open = true;
            let active = presets::active(cx).map(SharedString::from);
            let rows = Self::rows(cx);
            let current = active.map_or(Row::Default, Row::Custom);
            self.highlighted = rows.iter().position(|row| *row == current);
            cx.notify();
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            self.highlighted = None;
            self.menu = None;
            cx.notify();
        }
    }

    /// Does what `row` stands for: picks a preset, makes one or imports one.
    fn activate(&mut self, row: &Row, window: &mut Window, cx: &mut Context<Self>) {
        self.status = None;
        match row {
            Row::New => {
                let name = presets::create(cx);
                self.start_renaming(name.into(), window, cx);
            }
            Row::Import => {
                self.close(cx);
                Self::import(cx);
            }
            Row::Custom(name) => {
                presets::select(cx, Some(name));
                self.close(cx);
            }
            Row::Default => {
                presets::select(cx, None);
                self.close(cx);
            }
        }
    }

    /// Opens preset `name`'s `···` menu at `position`.
    fn open_menu(
        &mut self,
        name: &SharedString,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = actions::menu_items(&cx.entity().downgrade(), name);
        let menu = ContextMenu::build(items, window, cx);
        let subscription =
            cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, window, cx| {
                this.menu = None;
                if this.renaming.is_none() {
                    this.focus_handle.focus(window, cx);
                }
                cx.notify();
            });
        self.menu = Some((position, menu, subscription));
        cx.notify();
    }

    /// Edits preset `name`'s name in place, the whole name selected.
    fn start_renaming(&mut self, name: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(&name)
                .with_all_selected()
                .with_label(tr!("Preset name"))
                .with_max_len(presets::names::MAX_CHARS)
        });
        let events = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if matches!(event, text_input::Event::Submitted | text_input::Event::Blurred) {
                this.finish_renaming(true, window, cx);
            }
        });
        input.read(cx).focus_handle(cx).focus(window, cx);
        self.renaming = Some(Renaming { name, input, _events: events });
        cx.notify();
    }

    /// Ends editing a name, keeping what was typed when `keep`. An empty name keeps the old one
    /// and says why.
    fn finish_renaming(&mut self, keep: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.take() else { return };
        if keep {
            let typed = renaming.input.read(cx).text().to_owned();
            if presets::rename(cx, &renaming.name, &typed).is_none() {
                self.report(tr!("A preset needs a name, so it keeps its old one."), true, cx);
            }
        }
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Shows `text` under the dropdown, in the danger color when the action `failed`.
    fn report(&mut self, text: impl Into<SharedString>, failed: bool, cx: &mut Context<Self>) {
        self.status = Some(Status { text: text.into(), failed });
        cx.notify();
    }
}
