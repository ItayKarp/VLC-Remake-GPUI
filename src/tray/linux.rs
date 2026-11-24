//! The tray icon on Linux: a StatusNotifierItem (what KDE, Cinnamon, XFCE, most Wayland bars and
//! GNOME's AppIndicator extension show) with its menu over the dbusmenu interface, served on a
//! D-Bus connection of its own. Registering it with the desktop's StatusNotifierWatcher puts the
//! icon up; closing the connection takes it away.
//!
//! The icon is a pixmap of the cone, so no icon theme needs to have it. Clicking it shows VLC, a
//! middle click plays or pauses, and the menu has every command.

use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::Context as _;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::blocking::fdo::DBusProxy;
use zbus::interface;
use zbus::names::BusName;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use super::{CommandSender, ICON_PNG, TrayCommand};

const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
/// The dbusmenu protocol version spoken.
const DBUSMENU_VERSION: u32 = 3;
/// The menu's root item's id.
const ROOT_ID: i32 = 0;
/// Separators' ids start here, past every command's.
const SEPARATOR_IDS: i32 = 100;

/// An icon image: width, height and ARGB32 pixels in network byte order.
type Pixmap = (i32, i32, Vec<u8>);
/// A menu item's properties.
type Properties = HashMap<String, OwnedValue>;
/// A menu item: its id, properties and children (each one a variant holding a `Layout`).
type Layout = (i32, Properties, Vec<OwnedValue>);

/// The icon's D-Bus connection; closing it takes the icon away.
pub struct Icon {
    _connection: Connection,
}

impl Icon {
    pub fn show(sender: CommandSender) -> anyhow::Result<Self> {
        let item = Item { sender: sender.clone(), pixmaps: vec![pixmap()?] };
        let connection = Builder::session()?
            .serve_at(ITEM_PATH, item)?
            .serve_at(MENU_PATH, Menu { sender })?
            .build()?;
        let name = connection.unique_name().context("no name on the session bus")?.to_string();
        connection
            .call_method(
                Some(WATCHER),
                WATCHER_PATH,
                Some(WATCHER),
                "RegisterStatusNotifierItem",
                &(name.as_str()),
            )
            .context("the desktop's tray refused the icon")?;
        Ok(Self { _connection: connection })
    }
}

/// Whether the desktop runs a StatusNotifierWatcher to show the icon. Asked once.
pub fn is_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let ask = || -> zbus::Result<bool> {
            let connection = Connection::session()?;
            let watcher = BusName::try_from(WATCHER)?;
            Ok(DBusProxy::new(&connection)?.name_has_owner(watcher)?)
        };
        ask().unwrap_or_else(|err| {
            tracing::info!("no system tray: {err}");
            false
        })
    })
}

/// The cone, as the protocol wants it.
fn pixmap() -> anyhow::Result<Pixmap> {
    let image = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)?.to_rgba8();
    let (width, height) = image.dimensions();
    let argb = image.pixels().flat_map(|pixel| {
        let [red, green, blue, alpha] = pixel.0;
        [alpha, red, green, blue]
    });
    Ok((width as i32, height as i32, argb.collect()))
}

/// The StatusNotifierItem.
struct Item {
    sender: CommandSender,
    pixmaps: Vec<Pixmap>,
}

#[allow(
    clippy::unused_self,
    clippy::unnecessary_literal_bound,
    unused_variables,
    clippy::needless_pass_by_value,
    reason = "the D-Bus interface's methods and properties take every argument the protocol \
              sends, used or not"
)]
#[interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    fn activate(&self, x: i32, y: i32) {
        self.sender.unbounded_send(TrayCommand::ShowVlc).ok();
    }

    fn secondary_activate(&self, x: i32, y: i32) {
        self.sender.unbounded_send(TrayCommand::PlayPause).ok();
    }

    fn context_menu(&self, x: i32, y: i32) {}

    fn scroll(&self, delta: i32, orientation: &str) {}

    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "vlc"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "VLC"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<Pixmap> {
        self.pixmaps.clone()
    }

    #[zbus(property)]
    fn tool_tip(&self) -> (String, Vec<Pixmap>, String, String) {
        (String::new(), Vec::new(), "VLC".to_owned(), String::new())
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        ObjectPath::from_static_str_unchecked(MENU_PATH).into()
    }
}

/// The icon's menu, which never changes.
struct Menu {
    sender: CommandSender,
}

impl Menu {
    fn clicked(&self, id: i32, event: &str) {
        if event == "clicked"
            && let Some(command) = TrayCommand::from_id(i64::from(id))
        {
            self.sender.unbounded_send(command).ok();
        }
    }
}

#[allow(
    clippy::unused_self,
    clippy::unnecessary_literal_bound,
    unused_variables,
    clippy::needless_pass_by_value,
    reason = "the D-Bus interface's methods and properties take every argument the protocol \
              sends, used or not"
)]
#[interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(&self, parent: i32, depth: i32, names: Vec<String>) -> (u32, Layout) {
        (1, layout())
    }

    fn get_group_properties(&self, ids: Vec<i32>, names: Vec<String>) -> Vec<(i32, Properties)> {
        ids.into_iter().filter_map(|id| Some((id, properties(id)?))).collect()
    }

    fn get_property(&self, id: i32, name: String) -> zbus::fdo::Result<OwnedValue> {
        let value = properties(id).and_then(|mut properties| properties.remove(&name));
        value.ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("no {name} on item {id}")))
    }

    fn event(&self, id: i32, event: String, data: OwnedValue, timestamp: u32) {
        self.clicked(id, &event);
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        for (id, event, _, _) in events {
            self.clicked(id, &event);
        }
        Vec::new()
    }

    fn about_to_show(&self, id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        DBUSMENU_VERSION
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }
}

/// The whole menu: the root, holding the commands with separators between their groups.
fn layout() -> Layout {
    let mut ids = Vec::new();
    for (index, group) in TrayCommand::MENU.iter().enumerate() {
        if index > 0 {
            ids.push(SEPARATOR_IDS + index as i32);
        }
        ids.extend(group.iter().map(|command| command.id()));
    }
    let children = ids.into_iter().filter_map(|id| {
        let child: Layout = (id, properties(id)?, Vec::new());
        OwnedValue::try_from(Value::from(child)).ok()
    });
    (ROOT_ID, properties(ROOT_ID).unwrap_or_default(), children.collect())
}

/// Item `id`'s properties; `None` for an id the menu doesn't have.
fn properties(id: i32) -> Option<Properties> {
    let text = |text: &str| OwnedValue::try_from(Value::from(text)).ok();
    let property = |name: &str, value: &str| Some((name.to_owned(), text(value)?));
    let pairs = if id == ROOT_ID {
        vec![property("children-display", "submenu")]
    } else if id > SEPARATOR_IDS {
        vec![property("type", "separator")]
    } else {
        vec![property("label", TrayCommand::from_id(i64::from(id))?.label())]
    };
    Some(pairs.into_iter().flatten().collect())
}
