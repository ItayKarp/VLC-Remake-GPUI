//! What a preset's `···` menu does (Rename, Duplicate, Export…, Delete…), and importing a
//! preset file (S5.11.4).

use std::path::Path;

use gpui::{
    App, AppContext as _, Context, PathPromptOptions, PromptLevel, SharedString, WeakEntity, Window,
};

use super::PresetPicker;
use crate::assets::icons;
use crate::settings::{self, presets};
use crate::ui::menu::MenuItem;

/// An item of a preset's `···` menu.
#[derive(Clone, Copy, Debug)]
enum MenuAction {
    Rename,
    Duplicate,
    Export,
    Delete,
}

/// Preset `name`'s `···` menu.
pub(super) fn menu_items(picker: &WeakEntity<PresetPicker>, name: &SharedString) -> Vec<MenuItem> {
    let on = |action: MenuAction| {
        let (picker, name) = (picker.clone(), name.clone());
        move |window: &mut Window, cx: &mut App| {
            picker.update(cx, |this, cx| this.run(action, name.clone(), window, cx)).ok();
        }
    };
    vec![
        MenuItem::action("Rename…", on(MenuAction::Rename)).icon(icons::EDIT),
        MenuItem::action("Duplicate", on(MenuAction::Duplicate)).icon(icons::ADD),
        MenuItem::action("Export…", on(MenuAction::Export)).icon(icons::FILE),
        MenuItem::action("Delete…", on(MenuAction::Delete)).icon(icons::CLOSE),
    ]
}

impl PresetPicker {
    /// Does what `action` says to preset `name`.
    fn run(
        &mut self,
        action: MenuAction,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status = None;
        match action {
            MenuAction::Rename => self.start_renaming(name, window, cx),
            MenuAction::Duplicate => {
                if let Some(copy) = presets::duplicate(cx, &name) {
                    self.report(format!("Made a copy named “{copy}”."), false, cx);
                }
            }
            MenuAction::Export => Self::export(name, cx),
            MenuAction::Delete => Self::confirm_delete(name, window, cx),
        }
    }

    /// Asks before deleting preset `name`, or deletes it at once when confirmations are off
    /// (S5.3.3). Deleting the preset in use goes back to Default Preset.
    fn confirm_delete(name: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if !settings::get(cx).prefs.startup.show_dialogs {
            presets::delete(cx, &name);
            return;
        }
        let detail = if presets::active(cx) == Some(&*name) {
            "It's in use, so the settings go back to Default Preset."
        } else {
            "The settings it changes go with it."
        };
        let title = format!("Delete the preset “{name}”?");
        let answer =
            window.prompt(PromptLevel::Warning, &title, Some(detail), &["Delete", "Cancel"], cx);
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                cx.update(|cx| presets::delete(cx, &name));
            }
        })
        .detach();
    }

    /// Asks where to save preset `name` and writes it there.
    fn export(name: SharedString, cx: &mut Context<Self>) {
        let Some(preset) = settings::get(cx).prefs.presets.find(&name).cloned() else { return };
        let folder = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default();
        let destination = cx.prompt_for_new_path(&folder, Some(&presets::file::file_name(&name)));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = destination.await else { return };
            let text = presets::file::render(&preset);
            let written = cx
                .background_spawn(async move { std::fs::write(&path, text).map(|()| path) })
                .await;
            let (status, failed) = match written {
                Ok(path) => (format!("Exported to “{}”.", file_name(&path)), false),
                Err(err) => {
                    tracing::warn!("settings: couldn't export preset “{name}”: {err}");
                    (format!("Couldn't export “{name}”: {err}."), true)
                }
            };
            this.update(cx, |this, cx| this.report(status, failed, cx)).ok();
        })
        .detach();
    }

    /// Asks for a preset file and adds the preset in it, in use.
    pub(super) fn import(cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let read = path.clone();
            let parsed = cx
                .background_spawn(async move {
                    let text = std::fs::read_to_string(&read).map_err(|err| err.to_string())?;
                    presets::file::parse(&text, &read)
                })
                .await;
            this.update(cx, |this, cx| match parsed {
                Ok(preset) => {
                    let name = presets::add(cx, preset);
                    this.report(format!("Imported “{name}” and put it in use."), false, cx);
                }
                Err(error) => {
                    this.report(
                        format!("Couldn't import “{}”: {error}", file_name(&path)),
                        true,
                        cx,
                    );
                }
            })
            .ok();
        })
        .detach();
    }
}

/// The last part of `path`, for messages.
fn file_name(path: &Path) -> String {
    path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
}
