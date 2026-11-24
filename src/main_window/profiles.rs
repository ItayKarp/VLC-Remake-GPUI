//! The main menu's Profile ▸ (S8.8, `menus/profiles/profiles.png`): the profiles with the one
//! in use checked, then + New, which asks for a name. Rename, Delete and switching by itself
//! ([`automatic`]) aren't designed; they follow, for the profile in use.
//!
//! Switching applies at once, without touching playback: the bars take the profile's layout
//! and its preset's settings apply as the preset picker's do.

mod automatic;

use gpui::{App, Context, Entity, WeakEntity, Window, prelude::*};

use super::MainWindow;
use crate::assets::icons;
use crate::settings::{self, profiles};
use crate::ui::menu::MenuItem;
use crate::ui::modal::{Dialog, DialogButton};
use crate::ui::text_input::TextInput;
use crate::ui::toast::{Level, Toast};

pub(super) use self::automatic::AutoProfile;

/// The longest profile name kept, in characters, as for presets.
const MAX_NAME: usize = settings::presets::names::MAX_CHARS;

/// Profile (Movies) ▸: the profiles, + New, then what can be done to the one in use.
pub(super) fn profile_menu(main: &WeakEntity<MainWindow>, cx: &App) -> MenuItem {
    let active = profiles::active(cx);
    let list = profiles::list(cx);
    let mut items: Vec<MenuItem> = list
        .iter()
        .map(|profile| {
            let name = profile.name.clone();
            let choose = on_main(main, move |main, _, cx| main.choose_profile(&name, cx));
            MenuItem::radio(shown(&profile.name), profile.name == active, choose)
        })
        .collect();
    items.push(
        MenuItem::action(tr!("New"), on_main(main, MainWindow::ask_new_profile)).icon(icons::ADD),
    );
    items.push(MenuItem::Separator);
    items.extend(own_items(main, &active, list.len(), cx));
    MenuItem::submenu(tr!("Profile ({active})", active = shown(&active)), items)
}

/// How profile `name` is drawn: a designed profile's name in the interface language, any other
/// as typed. Names are stored and matched in English.
fn shown(name: &str) -> String {
    let designed = profiles::book::DESIGNED.into_iter().find(|designed| *designed == name);
    designed.map_or_else(|| name.to_owned(), |designed| crate::i18n::text(designed).to_owned())
}

/// Rename…, Delete… and Use Automatically ▸ for profile `active`, one of `count`.
fn own_items(main: &WeakEntity<MainWindow>, active: &str, count: usize, cx: &App) -> Vec<MenuItem> {
    let rename = MenuItem::action(
        tr!("Rename “{active}”…", active = shown(active)),
        on_main(main, MainWindow::ask_rename),
    );
    let delete = MenuItem::action(
        tr!("Delete “{active}”…", active = shown(active)),
        on_main(main, MainWindow::ask_delete),
    )
    .disabled(count < 2)
    .why_disabled(tr!("The last profile can't be deleted"));
    vec![rename.icon(icons::EDIT), delete.icon(icons::REMOVE), automatic::menu(active, cx)]
}

/// What a menu item runs to run `f` on the main window `main`.
fn on_main(
    main: &WeakEntity<MainWindow>,
    f: impl Fn(&mut MainWindow, &mut Window, &mut Context<MainWindow>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let main = main.clone();
    move |window, cx| {
        main.update(cx, |main, cx| f(main, window, cx)).ok();
    }
}

/// What a name dialog asks: its title, the name it starts with and its confirming button.
struct NamePrompt {
    title: &'static str,
    name: String,
    confirm: &'static str,
}

impl MainWindow {
    /// Switches to profile `name`, chosen by the user: it stays until they or the media
    /// switch again.
    fn choose_profile(&mut self, name: &str, cx: &mut Context<Self>) {
        self.auto_profile.forget();
        self.switch_profile(name, cx);
    }

    /// Switches to profile `name` and says so over the video.
    fn switch_profile(&mut self, name: &str, cx: &mut Context<Self>) {
        if profiles::active(cx) == name {
            return;
        }
        profiles::select(cx, name);
        self.control_bar.sync_layout(cx);
        self.show_osd(tr!("Profile: {name}", name = shown(name)), cx);
        cx.notify();
    }

    /// + New: asks for the new profile's name, then makes it from the one in use.
    fn ask_new_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let main = cx.entity().downgrade();
        let from = profiles::active(cx);
        let prompt = NamePrompt {
            title: tr!("New Profile"),
            name: crate::i18n::text(profiles::NEW).to_owned(),
            confirm: tr!("Create"),
        };
        self.ask_profile_name(&prompt, window, cx, move |typed, cx| {
            let made = profiles::create(cx, &typed);
            let message =
                tr!("Made the profile “{made}” from “{from}”.", made, from = shown(&from));
            main.update(cx, |main, cx| main.profile_switched(message, cx)).ok();
        });
    }

    /// Asks for the profile in use's new name, and renames it.
    fn ask_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let main = cx.entity().downgrade();
        let from = profiles::active(cx);
        let prompt =
            NamePrompt { title: tr!("Rename Profile"), name: from.clone(), confirm: tr!("Rename") };
        self.ask_profile_name(&prompt, window, cx, move |typed, cx| {
            let message = match profiles::rename(cx, &from, &typed) {
                Some(name) => Toast::new(Level::Success, tr!("Renamed to “{name}”.", name)),
                None => {
                    Toast::new(Level::Error, tr!("A profile needs a name; it keeps the old one."))
                }
            };
            main.update(cx, |main, cx| main.show_toast(message, cx)).ok();
        });
    }

    /// Opens the dialog `prompt` describes, whose confirming button hands what was typed to
    /// `on_confirm`.
    fn ask_profile_name(
        &mut self,
        prompt: &NamePrompt,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_confirm: impl Fn(String, &mut App) + 'static,
    ) {
        let input: Entity<TextInput> = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(&prompt.name)
                .with_all_selected()
                .with_label(tr!("Profile name"))
                .with_max_len(MAX_NAME)
        });
        let button = {
            let input = input.clone();
            DialogButton::new(prompt.confirm).primary().on_click(move |_, cx| {
                on_confirm(input.read(cx).text().to_owned(), cx);
            })
        };
        let body = move |_: &mut Window, _: &mut App| input.clone().into_any_element();
        let buttons = vec![DialogButton::new(tr!("Cancel")), button];
        let dialog = cx.new(|cx| Dialog::new(prompt.title, body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// Asks before deleting the profile in use, or deletes it at once when confirmations are
    /// off (S5.3.3).
    fn ask_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = profiles::active(cx);
        if !settings::get(cx).prefs.startup.show_dialogs {
            self.delete_profile(&name, cx);
            return;
        }
        let main = cx.entity().downgrade();
        let title = tr!("Delete “{name}”?", name = shown(&name));
        let delete = DialogButton::new(tr!("Delete")).primary().on_click(move |_, cx| {
            main.update(cx, |main, cx| main.delete_profile(&name, cx)).ok();
        });
        let body = |_: &mut Window, _: &mut App| {
            tr!("Its player layout is removed and VLC switches to another profile. The preset it \
                 uses stays.")
            .into_any_element()
        };
        let buttons = vec![DialogButton::new(tr!("Cancel")), delete];
        let dialog = cx.new(|cx| Dialog::new(title, body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// Deletes profile `name`, which switches to another when it's in use.
    fn delete_profile(&mut self, name: &str, cx: &mut Context<Self>) {
        if profiles::delete(cx, name) {
            let now = profiles::active(cx);
            self.profile_switched(
                tr!("Deleted “{name}”. Now using “{now}”.", name = shown(name), now = shown(&now)),
                cx,
            );
        }
    }

    /// Shows the profile now in use, saying `message`.
    fn profile_switched(&mut self, message: String, cx: &mut Context<Self>) {
        self.auto_profile.forget();
        self.control_bar.sync_layout(cx);
        self.show_toast(Toast::new(Level::Success, message), cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
