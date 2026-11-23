//! The displays Settings → Interface "Open fullscreen on" offers (S5.4.1), by an id that lasts
//! across launches (the display's UUID), and the display a saved id names.

use std::rc::Rc;

use gpui::{App, PlatformDisplay};

use crate::settings::registry::MachineChoice;

/// The id of "Current screen": fullscreen stays on the screen the window is on.
pub const CURRENT_SCREEN: &str = "";

/// "Current screen", then each connected display, the main one first. A saved display that is
/// no longer connected is listed last, so the row still shows what is saved.
pub fn choices(cx: &App, saved: &str) -> Vec<MachineChoice> {
    let current =
        MachineChoice { id: CURRENT_SCREEN.into(), label: "Current screen".into(), glyph: None };
    let mut choices = vec![current];
    choices.extend(ordered(cx).iter().enumerate().filter_map(|(index, display)| {
        Some(MachineChoice {
            id: id_of(display.as_ref())?,
            label: label(index, display.as_ref()),
            glyph: None,
        })
    }));
    if !choices.iter().any(|choice| choice.id == saved) {
        choices.push(MachineChoice {
            id: saved.into(),
            label: "Disconnected screen".into(),
            glyph: None,
        });
    }
    choices
}

/// The connected display with `id`; `None` for "Current screen" or a display that is gone.
pub fn find(id: &str, cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    if id == CURRENT_SCREEN {
        return None;
    }
    cx.displays().into_iter().find(|display| id_of(display.as_ref()).is_some_and(|d| d == id))
}

/// The displays, the main one first.
fn ordered(cx: &App) -> Vec<Rc<dyn PlatformDisplay>> {
    let primary = cx.primary_display().map(|display| display.id());
    let mut displays = cx.displays();
    displays.sort_by_key(|display| Some(display.id()) != primary);
    displays
}

/// A display's lasting id.
fn id_of(display: &dyn PlatformDisplay) -> Option<String> {
    display.uuid().ok().map(|uuid| uuid.to_string())
}

/// "Main screen (1512 × 982)", "Screen 2 (2560 × 1440)".
fn label(index: usize, display: &dyn PlatformDisplay) -> String {
    let size = display.bounds().size;
    let (width, height) = (f32::from(size.width).round(), f32::from(size.height).round());
    let name = if index == 0 { "Main screen".to_owned() } else { format!("Screen {}", index + 1) };
    format!("{name} ({width} × {height})")
}
