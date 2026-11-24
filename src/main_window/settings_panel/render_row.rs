//! The Video tab's **Render** row (S8.11, `controls-video_settings.png`): where media plays,
//! "Local" or a cast device found on the network, with a line under it when there is something
//! to say: discovery is off, no device was found, or what a cast device doesn't get.

use gpui::{App, AppContext as _, Context, Div, Entity, SharedString, Subscription, Window, div};
use gpui::{prelude::*, px};

use super::video_defaults::{CONTROL_WIDTH, labelled, note};
use crate::engine::cast::CastDeviceId;
use crate::engine::player::Player;
use crate::i18n;
use crate::i18n::DirectionalStyle;
use crate::main_window::MainWindow;
use crate::settings;
use crate::theme::Palette;
use crate::ui::dropdown::{self, Dropdown};

/// The row's label, and its first choice: playing here.
const LABEL: &str = tr_noop!("Render");
const LOCAL: &str = tr_noop!("Local");
/// The line under the row while "Discover cast devices" is off.
const DISCOVERY_OFF: &str =
    tr_noop!("Turn on Discover cast devices in Settings → Recording to play on a TV.");
/// The line under the row while no device was found.
const NONE_FOUND: &str = tr_noop!(
    "No cast devices found. They need to be on this network, and a firewall can hide them."
);
/// The line under the row while media plays on a cast device.
const CASTING: &str = tr_noop!(
    "Subtitles don't show on cast devices, and the device keeps its own volume. What it can't \
     play is converted first, as Settings → Recording's Cast quality says."
);

/// The Render dropdown and the devices its choices after "Local" stand for.
pub(super) struct RenderControl {
    dropdown: Entity<Dropdown>,
    /// The choices shown: "Local", then each device's name.
    labels: Vec<SharedString>,
    devices: Vec<CastDeviceId>,
    _changed: Subscription,
}

impl RenderControl {
    /// The row's dropdown, playing on the device picked.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let labels = vec![SharedString::from(i18n::text(LOCAL))];
        let label = i18n::text(LABEL);
        let dropdown = cx.new(|cx| Dropdown::new(label, labels.clone(), 0, window, cx));
        let changed = cx.subscribe(&dropdown, |this, _, event: &dropdown::Changed, cx| {
            let device = event.0.checked_sub(1).and_then(|index| {
                this.settings_panel.video_defaults.render.devices.get(index).copied()
            });
            this.cast_to(device, cx);
        });
        Self { dropdown, labels, devices: Vec::new(), _changed: changed }
    }

    /// Lists the devices `player` found and picks the one media plays on.
    pub(super) fn sync(&mut self, player: Option<&Entity<Player>>, cx: &mut Context<MainWindow>) {
        let (devices, target) = player.map_or_else(Default::default, |player| {
            let player = player.read(cx);
            (player.cast_devices().to_vec(), player.cast_target().map(|device| device.id))
        });
        let local = SharedString::from(i18n::text(LOCAL));
        let labels: Vec<SharedString> = std::iter::once(local)
            .chain(devices.iter().map(|device| SharedString::from(device.name.clone())))
            .collect();
        self.devices = devices.iter().map(|device| device.id).collect();
        let selected = target
            .and_then(|target| self.devices.iter().position(|id| *id == target))
            .map_or(0, |index| index + 1);
        let changed = labels != self.labels;
        self.labels.clone_from(&labels);
        self.dropdown.update(cx, |dropdown, cx| {
            if changed {
                dropdown.set_options(labels, cx);
            }
            dropdown.set_selected(selected, cx);
        });
    }
}

impl MainWindow {
    /// The Render row, and the line under it.
    pub(super) fn render_render_row(&self, p: &Palette, cx: &App) -> Div {
        let control = &self.settings_panel.video_defaults.render;
        let dropdown = div().w(px(CONTROL_WIDTH)).child(control.dropdown.clone());
        let line = render_note(control, cx);
        div()
            .flex_row_start()
            .flex_col()
            .child(labelled(i18n::text(LABEL), dropdown, p))
            .children(line.map(|text| note(i18n::text(text).to_owned(), p)))
    }
}

/// What the line under `control`'s row says, if anything.
fn render_note(control: &RenderControl, cx: &App) -> Option<&'static str> {
    if !settings::get(cx).prefs.recording.discover_cast_devices {
        Some(DISCOVERY_OFF)
    } else if control.dropdown.read(cx).selected() > 0 {
        Some(CASTING)
    } else {
        control.devices.is_empty().then_some(NONE_FOUND)
    }
}
