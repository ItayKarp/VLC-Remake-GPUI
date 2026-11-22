//! An options row's dropdown (Settings → Interface "Open fullscreen on", S5.4.1): its options are
//! what this machine has, listed again whenever the page syncs, so a display plugged in while
//! Settings is open shows up.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Context, Entity, SharedString, Subscription, Window, prelude::*};

use super::SettingsPage;
use crate::settings::{
    self, Settings,
    registry::{MachineChoice, OptionsSource, SettingDef, Value},
};
use crate::ui::dropdown::{self, Dropdown};

/// The dropdown and the options it lists.
pub(super) struct OptionsDropdown {
    /// The control.
    pub(super) dropdown: Entity<Dropdown>,
    /// The listed options, shared with the subscription that saves a pick.
    listed: Rc<RefCell<Vec<MachineChoice>>>,
}

impl OptionsDropdown {
    /// The dropdown of `def`, whose options `source` lists, and the subscription saving picks.
    pub(super) fn new(
        def: &'static SettingDef,
        source: OptionsSource,
        window: &mut Window,
        cx: &mut Context<SettingsPage>,
    ) -> (Self, Subscription) {
        let options = source(cx, &saved_id(def, settings::get(cx)));
        let selected = index_of(&options, &saved_id(def, settings::get(cx)));
        let labels = labels(&options);
        let dropdown = cx.new(|cx| Dropdown::new(def.label, labels, selected, window, cx));
        let listed = Rc::new(RefCell::new(options));
        let picks = listed.clone();
        let subscription =
            cx.subscribe(&dropdown, move |page, _, event: &dropdown::Changed, cx| {
                let id = picks.borrow().get(event.0).map(|choice| choice.id.clone());
                if let Some(id) = id {
                    page.write(def, Value::Text(id), cx);
                }
            });
        (Self { dropdown, listed }, subscription)
    }

    /// Lists the options again and shows the saved one.
    pub(super) fn sync(
        &self,
        def: &SettingDef,
        source: OptionsSource,
        settings: &Settings,
        cx: &mut Context<SettingsPage>,
    ) {
        let id = saved_id(def, settings);
        let options = source(cx, &id);
        let selected = index_of(&options, &id);
        let changed = *self.listed.borrow() != options;
        let disabled = !def.is_enabled(settings);
        self.dropdown.update(cx, |dropdown, cx| {
            if changed {
                dropdown.set_options(labels(&options), cx);
            }
            dropdown.set_selected(selected, cx);
            dropdown.set_disabled(disabled, cx);
        });
        if changed {
            *self.listed.borrow_mut() = options;
        }
    }
}

/// The id the setting holds.
fn saved_id(def: &SettingDef, settings: &Settings) -> String {
    match def.value(settings) {
        Value::Text(id) => id,
        _ => String::new(),
    }
}

fn labels(options: &[MachineChoice]) -> Vec<SharedString> {
    options.iter().map(|choice| SharedString::from(choice.label.clone())).collect()
}

fn index_of(options: &[MachineChoice], id: &str) -> usize {
    options.iter().position(|choice| choice.id == id).unwrap_or_default()
}
