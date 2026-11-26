//! Casting (S8.11) on the worker thread: libVLC's renderer discoverers find cast devices, and
//! `libvlc_media_player_set_renderer` plays to one.
//!
//! Every discovery service libVLC has is started (Bonjour on macOS, microdns elsewhere), so
//! devices on any network interface the OS browses are found, a VPN's included. Their events
//! arrive on libVLC's threads: the callback holds an added device, as libVLC asks, and passes it
//! to the worker, which keeps the list and tells the player.
//!
//! A renderer set on the player outlives the media, so later media and reopens cast too.
//! libVLC asks "Performance warning" before converting for a device; nobody would answer, so the
//! question is turned off.

use std::ffi::{CStr, CString, c_int, c_void};
use std::sync::{Arc, mpsc};

use super::api::{Event, LibVlc, RendererDiscoverer, RendererItem};
use super::core_vars::Var;
use super::worker::{Message, Worker};
use crate::engine::backend::BackendEvent;
use crate::engine::cast::{self, CastDevice, CastDeviceId, CastSetup};

/// `libvlc_RendererDiscovererItemAdded` and `libvlc_RendererDiscovererItemDeleted`.
const ITEM_ADDED: c_int = 0x502;
const ITEM_DELETED: c_int = 0x503;
/// `LIBVLC_RENDERER_CAN_VIDEO`.
const CAN_VIDEO: c_int = 0x0002;
/// The chromecast output's conversion quality and its question before converting.
const QUALITY: &CStr = c"sout-chromecast-conversion-quality";
const PERF_WARNING: &CStr = c"sout-chromecast-show-perf-warning";

/// A renderer item the worker holds a reference to, or one being deleted (compared, never read).
#[derive(Debug)]
pub struct Renderer(*mut RendererItem);

// SAFETY: renderer items are reference-counted libVLC objects, safe to pass between threads.
unsafe impl Send for Renderer {}

/// What a discoverer reported.
#[derive(Debug)]
pub enum RendererChange {
    /// A device appeared, held for the worker, found by the discoverers started `round`th.
    Added { renderer: Renderer, round: u64 },
    /// A device went away.
    Removed(Renderer),
}

/// What the discoverers' callback needs: libVLC, to hold an added device, the worker, and which
/// start of discovery it serves.
struct Listener {
    vlc: Arc<LibVlc>,
    worker: mpsc::Sender<Message>,
    round: u64,
}

/// A device found, and the worker's reference to it.
struct Found {
    device: CastDevice,
    item: *mut RendererItem,
}

/// The worker's casting state.
#[derive(Default)]
pub(super) struct Casting {
    /// The running discoverers; empty while discovery is off.
    discoverers: Vec<*mut RendererDiscoverer>,
    /// The callback's data, boxed so its address holds; kept until the discoverers are gone.
    listener: Option<Box<Listener>>,
    found: Vec<Found>,
    next_id: u64,
    /// How many times discovery started: devices from discoverers since stopped are let go.
    round: u64,
    /// The device media plays on; `None` plays here.
    target: Option<CastDeviceId>,
    setup: Option<CastSetup>,
}

impl Worker {
    /// Looks for devices or stops, and converts for them, as `setup` says.
    pub(super) fn set_cast_setup(&mut self, setup: CastSetup) {
        let before = self.casting.setup.replace(setup);
        if before.map(|before| before.quality) != Some(setup.quality) {
            let quality = Var::Integer(cast::conversion_quality(setup.quality));
            // SAFETY: the player is live.
            unsafe {
                self.vlc.set_player_var(self.handles.player, QUALITY, quality);
                self.vlc.set_player_var(self.handles.player, PERF_WARNING, Var::Integer(0));
            }
        }
        match (before.is_some_and(|before| before.discover), setup.discover) {
            (false, true) => self.start_discovery(),
            (true, false) => {
                self.stop_discovery();
                self.send_cast_devices();
            }
            _ => {}
        }
    }

    /// Plays on `device`, or here with `None`; an unknown device is ignored.
    pub(super) fn cast(&mut self, device: Option<CastDeviceId>) {
        let Some(item) = self.renderer_item(device) else { return };
        // SAFETY: the player is live, and `item` is NULL or held by the worker.
        let failed =
            unsafe { (self.vlc.api.libvlc_media_player_set_renderer)(self.handles.player, item) };
        if failed == 0 {
            self.casting.target = device;
        } else {
            tracing::warn!("casting: libVLC refused the device: {}", self.vlc.last_error());
        }
    }

    /// The renderer for `device`: NULL to play here, `None` for a device no longer known.
    fn renderer_item(&self, device: Option<CastDeviceId>) -> Option<*mut RendererItem> {
        let Some(id) = device else { return Some(std::ptr::null_mut()) };
        let found = self.casting.found.iter().find(|found| found.device.id == id);
        found.map(|found| found.item)
    }

    /// Keeps a device a discoverer found, or forgets one that went away.
    pub(super) fn renderer_changed(&mut self, change: RendererChange) {
        match change {
            RendererChange::Added { renderer, round } if round == self.casting.round => {
                self.renderer_added(renderer.0);
            }
            RendererChange::Added { renderer, .. } => self.release_item(renderer.0),
            RendererChange::Removed(renderer) => self.renderer_removed(renderer.0),
        }
    }

    /// Whether media plays on a cast device, so nothing shows here.
    pub(super) fn is_casting(&self) -> bool {
        self.casting.target.is_some()
    }

    /// Stops looking and lets the devices go; for shutting down, after the player is released.
    pub(super) fn release_casting(&mut self) {
        self.stop_discovery();
    }

    fn renderer_added(&mut self, item: *mut RendererItem) {
        let api = &self.vlc.api;
        // SAFETY: the worker holds `item`; its name is a string it owns.
        let (name, flags) = unsafe {
            let name = super::api::borrowed_string((api.libvlc_renderer_item_name)(item));
            (name, (api.libvlc_renderer_item_flags)(item))
        };
        self.casting.next_id += 1;
        let id = CastDeviceId(self.casting.next_id);
        let name = name.unwrap_or_else(|| tr!("Cast device").to_owned());
        tracing::info!("casting: found a device");
        let device = CastDevice { id, name, video: flags & CAN_VIDEO != 0 };
        self.casting.found.push(Found { device, item });
        self.send_cast_devices();
    }

    fn renderer_removed(&mut self, item: *mut RendererItem) {
        let Some(index) = self.casting.found.iter().position(|found| found.item == item) else {
            return;
        };
        let gone = self.casting.found.remove(index);
        if self.casting.target == Some(gone.device.id) {
            self.cast(None);
            self.shared.send(BackendEvent::CastLost(gone.device.clone()));
        }
        self.release_item(gone.item);
        self.send_cast_devices();
    }

    fn send_cast_devices(&self) {
        let devices = self.casting.found.iter().map(|found| found.device.clone()).collect();
        self.shared.send(BackendEvent::CastDevices(devices));
    }

    /// Starts every discovery service libVLC has.
    fn start_discovery(&mut self) {
        self.casting.round += 1;
        let listener = Box::new(Listener {
            vlc: Arc::clone(&self.vlc),
            worker: self.shared.worker.clone(),
            round: self.casting.round,
        });
        let data = std::ptr::from_ref::<Listener>(&listener).cast_mut().cast();
        self.casting.listener = Some(listener);
        for name in self.discovery_services() {
            if let Some(discoverer) = self.start_discoverer(&name, data) {
                self.casting.discoverers.push(discoverer);
            }
        }
        if self.casting.discoverers.is_empty() {
            tracing::warn!("casting: libVLC has no way to find cast devices");
        }
    }

    /// The names of libVLC's renderer discovery services.
    fn discovery_services(&self) -> Vec<CString> {
        let api = &self.vlc.api;
        let mut services = std::ptr::null_mut();
        // SAFETY: the instance is live; libVLC fills `services` with `count` entries.
        let count = unsafe {
            (api.libvlc_renderer_discoverer_list_get)(self.handles.instance, &raw mut services)
        };
        if services.is_null() {
            return Vec::new();
        }
        // SAFETY: `services` holds `count` live entries, each NULL or with a NULL or
        // NUL-terminated name, until released below.
        unsafe {
            let names = std::slice::from_raw_parts(services, count)
                .iter()
                .filter_map(|service| service.as_ref())
                .filter(|service| !service.name.is_null())
                .map(|service| CStr::from_ptr(service.name).to_owned())
                .collect();
            (api.libvlc_renderer_discoverer_list_release)(services, count);
            names
        }
    }

    /// Starts the discoverer `name`, reporting to `data`; `None` (logged) if it can't start.
    fn start_discoverer(&self, name: &CStr, data: *mut c_void) -> Option<*mut RendererDiscoverer> {
        let api = &self.vlc.api;
        let discoverer = self.new_discoverer(name)?;
        // SAFETY: the discoverer is live, and `data` outlives it (released first in
        // `stop_discovery`).
        let started = unsafe {
            self.listen(discoverer, data);
            (api.libvlc_renderer_discoverer_start)(discoverer) == 0
        };
        if !started {
            tracing::warn!("casting: {name:?} didn't start: {}", self.vlc.last_error());
            // SAFETY: the discoverer is live and released once.
            unsafe { (api.libvlc_renderer_discoverer_release)(discoverer) };
        }
        started.then_some(discoverer)
    }

    /// A new discoverer `name`; `None` (logged) if libVLC has none by that name.
    fn new_discoverer(&self, name: &CStr) -> Option<*mut RendererDiscoverer> {
        // SAFETY: the instance is live and `name` is NUL-terminated.
        let discoverer = unsafe {
            (self.vlc.api.libvlc_renderer_discoverer_new)(self.handles.instance, name.as_ptr())
        };
        if discoverer.is_null() {
            tracing::warn!("casting: no discoverer {name:?}");
        }
        (!discoverer.is_null()).then_some(discoverer)
    }

    /// Reports `discoverer`'s devices coming and going to the callback, with `data`.
    ///
    /// # Safety
    ///
    /// `discoverer` is live, and `data` is a [`Listener`] that outlives it.
    unsafe fn listen(&self, discoverer: *mut RendererDiscoverer, data: *mut c_void) {
        let api = &self.vlc.api;
        // SAFETY: the caller's promise; the event manager lives as long as the discoverer.
        unsafe {
            let events = (api.libvlc_renderer_discoverer_event_manager)(discoverer);
            for kind in [ITEM_ADDED, ITEM_DELETED] {
                (api.libvlc_event_attach)(events, kind, on_renderer_event, data);
            }
        }
    }

    /// Stops the discoverers and lets every device go; media casting plays here again.
    fn stop_discovery(&mut self) {
        if self.casting.target.is_some() {
            self.cast(None);
        }
        let api = &self.vlc.api;
        for discoverer in self.casting.discoverers.drain(..) {
            // SAFETY: each discoverer is live and released once; no callback runs after this.
            unsafe {
                (api.libvlc_renderer_discoverer_stop)(discoverer);
                (api.libvlc_renderer_discoverer_release)(discoverer);
            }
        }
        // Held devices still on their way to the worker are let go as they arrive.
        self.casting.round += 1;
        self.casting.listener = None;
        for found in std::mem::take(&mut self.casting.found) {
            self.release_item(found.item);
        }
    }

    fn release_item(&self, item: *mut RendererItem) {
        // SAFETY: the worker held `item` once, and lets it go once here.
        unsafe { (self.vlc.api.libvlc_renderer_item_release)(item) };
    }
}

/// The discoverers' `libvlc_callback_t`, with a `*const Listener` as user data.
unsafe extern "C" fn on_renderer_event(event: *const Event, data: *mut c_void) {
    if event.is_null() || data.is_null() {
        return;
    }
    // SAFETY: libVLC passes a live event, and `data` is the listener the worker keeps alive
    // until its discoverers are released.
    let (event, listener) = unsafe { (&*event, &*data.cast::<Listener>()) };
    // SAFETY: both renderer events carry an item at the union's start.
    let item = unsafe { event.data.renderer };
    if item.is_null() {
        return;
    }
    let change = match event.kind {
        // SAFETY: libVLC says to hold an added item in this callback; it is valid during it.
        ITEM_ADDED => RendererChange::Added {
            renderer: Renderer(unsafe { (listener.vlc.api.libvlc_renderer_item_hold)(item) }),
            round: listener.round,
        },
        ITEM_DELETED => RendererChange::Removed(Renderer(item)),
        _ => return,
    };
    // An error means the worker exited during shutdown; nothing is left to do.
    let _ = listener.worker.send(Message::Renderer(change));
}
