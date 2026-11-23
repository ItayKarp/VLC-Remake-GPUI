//! The [`Player`]'s casting (S8.11): the cast devices the backend found, the one media plays on,
//! and how the backend looks for them.

use gpui::Context;

use super::{Player, PlayerEvent};
use crate::engine::cast::{CastDevice, CastDeviceId, CastSetup};

/// What the player knows about casting.
#[derive(Clone, Debug, Default)]
pub(super) struct CastState {
    /// The devices found, as the backend last listed them.
    devices: Vec<CastDevice>,
    /// The device media plays on; `None` plays here.
    target: Option<CastDeviceId>,
    /// How the backend looks for devices, as last given to it.
    setup: Option<CastSetup>,
}

impl Player {
    /// Looks for cast devices, and converts media for them, as `setup` says.
    pub fn set_cast_setup(&mut self, setup: CastSetup) {
        if self.cast.setup != Some(setup) {
            self.cast.setup = Some(setup);
            self.with_backend(|backend| backend.set_cast_setup(setup));
        }
    }

    /// The cast devices found on the network.
    pub fn cast_devices(&self) -> &[CastDevice] {
        &self.cast.devices
    }

    /// The device media plays on, or `None` while it plays here.
    pub fn cast_target(&self) -> Option<&CastDevice> {
        let target = self.cast.target?;
        self.cast.devices.iter().find(|device| device.id == target)
    }

    /// Plays this and later media on `device`, or here with `None`, from where it is.
    pub fn cast_to(&mut self, device: Option<CastDeviceId>, cx: &mut Context<Self>) {
        let known = device.is_none_or(|id| self.cast.devices.iter().any(|known| known.id == id));
        if self.cast.target == device || !known {
            return;
        }
        self.cast.target = device;
        self.with_backend(|backend| backend.cast(device));
        cx.emit(PlayerEvent::CastChanged);
        cx.notify();
    }

    /// Keeps the devices the backend found. Without the one media played on, which the backend
    /// already left, media plays here.
    pub(super) fn cast_devices_reported(
        &mut self,
        devices: Vec<CastDevice>,
        cx: &mut Context<Self>,
    ) {
        self.cast.devices = devices;
        cx.emit(PlayerEvent::CastDevicesChanged);
        if self.cast.target.is_some() && self.cast_target().is_none() {
            self.cast.target = None;
            cx.emit(PlayerEvent::CastChanged);
        }
    }

    /// The device media played on went away, and it plays here again.
    pub(super) fn cast_lost(&mut self, device: CastDevice, cx: &mut Context<Self>) {
        if self.cast.target == Some(device.id) {
            self.cast.target = None;
            cx.emit(PlayerEvent::CastChanged);
        }
        cx.emit(PlayerEvent::CastLost(device));
    }
}
