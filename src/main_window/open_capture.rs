//! The Open Capture Device dialog (S8.9.2), not designed: **Source** (Camera or Screen),
//! **Device** (the cameras and capture cards plugged in, or the screens), and **Format** (a
//! camera's size and frame rate where libVLC can pick them, a screen's frame rate).
//!
//! Play first asks whether the OS lets the app use the device: where the user refused (macOS's
//! Camera or Screen Recording permission, Windows' camera privacy setting), the dialog says how
//! to allow it and offers the settings page, rather than opening a device that shows nothing.

mod open;
mod render;

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    SharedString, Subscription, Task, Window,
};

use crate::app::media::Media;
use crate::capture::{self, Access, Camera, Format, Screen, Source};
use crate::main_window::device_dialog::Then;
use crate::ui::dropdown::{self, Dropdown};

/// What is captured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Camera,
    Screen,
}

/// Runs with the media to play, once the dialog has closed.
type OnPlay = Then<Media>;

/// The Open Capture Device dialog's view.
pub(super) struct CaptureDialog {
    kind: Kind,
    /// The cameras plugged in; `None` while looking.
    cameras: Option<Vec<Camera>>,
    screens: Vec<Screen>,
    devices: Entity<Dropdown>,
    sizes: Entity<Dropdown>,
    rates: Entity<Dropdown>,
    /// Why Play was refused: the OS doesn't let the app use the device.
    refused: Option<Source>,
    looking: Option<Task<()>>,
    on_play: OnPlay,
    focus: FocusHandle,
    _subscriptions: [Subscription; 1],
}

impl EventEmitter<DismissEvent> for CaptureDialog {}

impl Focusable for CaptureDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CaptureDialog {
    /// The dialog, looking for cameras, handing what to play to `on_play`.
    pub(super) fn new(
        on_play: impl Fn(Media, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let looking = vec![tr!("Looking…").into()];
        let devices = cx.new(|cx| Dropdown::new(tr!("Device"), looking, 0, window, cx));
        let sizes = cx.new(|cx| {
            let options =
                choices(capture::SIZES.iter().map(|s| format!("{} × {}", s.width, s.height)));
            Dropdown::new(tr!("Size"), options, 0, window, cx)
        });
        let rates = cx.new(|cx| {
            let options = choices(capture::FRAME_RATES.iter().map(|fps| tr!("{fps} fps", fps)));
            Dropdown::new(tr!("Frame rate"), options, 0, window, cx)
        });
        let subscriptions =
            [cx.subscribe(&devices, |this: &mut Self, _, _: &dropdown::Changed, cx| {
                this.refused = None;
                cx.notify();
            })];
        let mut dialog = Self {
            kind: Kind::Camera,
            cameras: None,
            screens: capture::screens(),
            devices,
            sizes,
            rates,
            refused: None,
            looking: None,
            on_play: Rc::new(on_play),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        dialog.look_for_cameras(cx);
        dialog
    }

    /// Lists the cameras, off the UI thread.
    fn look_for_cameras(&mut self, cx: &mut Context<Self>) {
        let looking = cx.background_executor().spawn(async { capture::cameras() });
        self.looking = Some(cx.spawn(async move |this, cx| {
            let cameras = looking.await;
            this.update(cx, |this, cx| {
                this.cameras = Some(cameras);
                this.refresh_devices(cx);
            })
            .ok();
        }));
    }

    /// Captures `kind` from now on.
    fn set_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        self.kind = kind;
        self.refused = None;
        self.refresh_devices(cx);
    }

    /// Lists the devices of the kind picked.
    fn refresh_devices(&mut self, cx: &mut Context<Self>) {
        let names: Vec<SharedString> = match self.kind {
            Kind::Camera => match &self.cameras {
                None => vec![tr!("Looking…").into()],
                Some(cameras) if cameras.is_empty() => vec![tr!("No camera found").into()],
                Some(cameras) => cameras.iter().map(|camera| camera.name.clone().into()).collect(),
            },
            Kind::Screen => self.screens.iter().map(|screen| screen.name.clone().into()).collect(),
        };
        let empty = self.source(cx).is_none();
        self.devices.update(cx, |list, cx| {
            list.set_options(names, cx);
            list.set_selected(0, cx);
            list.set_disabled(empty, cx);
        });
        cx.notify();
    }

    /// The device picked.
    fn source(&self, cx: &App) -> Option<Source> {
        let index = self.devices.read(cx).selected();
        match self.kind {
            Kind::Camera => self.cameras.as_ref()?.get(index).cloned().map(Source::Camera),
            Kind::Screen => self.screens.get(index).cloned().map(Source::Screen),
        }
    }

    /// The format picked: the first choice of each list leaves it to the device.
    fn format(&self, cx: &App) -> Format {
        let picked = |list: &Entity<Dropdown>| list.read(cx).selected().checked_sub(1);
        Format {
            size: picked(&self.sizes)
                .filter(|_| self.kind == Kind::Camera)
                .and_then(|at| capture::SIZES.get(at).copied()),
            fps: picked(&self.rates).and_then(|at| capture::FRAME_RATES.get(at).copied()),
        }
    }

    /// Closes the dialog and plays the device, unless the OS doesn't let the app use it.
    fn play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.source(cx) else { return };
        let access = match source {
            Source::Camera(_) => capture::camera_access(),
            Source::Screen(_) => capture::screen_access(),
        };
        if access == Access::Denied {
            self.refused = Some(source);
            cx.notify();
            return;
        }
        let media = capture::media(&source, self.format(cx));
        super::device_dialog::close_with(media, self.on_play.clone(), window, cx);
    }
}

/// A list's choices: the device's own first, then `options`.
fn choices(options: impl Iterator<Item = String>) -> Vec<SharedString> {
    std::iter::once(tr!("Device's default").into()).chain(options.map(Into::into)).collect()
}
