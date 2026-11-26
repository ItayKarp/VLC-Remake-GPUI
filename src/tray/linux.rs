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

use super::status_notifier::{self, Entry, Pixmap, ROOT_ID};
use super::{CommandSender, ICON_PNG, TrayCommand};

const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
/// The dbusmenu protocol version spoken.
const DBUSMENU_VERSION: u32 = 3;
/// The menu's revision: it never changes.
const MENU_REVISION: u32 = 1;

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
        let connection = serve(Builder::session()?, sender)?;
        let name = connection.unique_name().context("no name on the session bus")?.to_string();
        register(&connection, &name).context("the desktop's tray refused the icon")?;
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

/// Serves the item and its menu, sending what is chosen to `sender`, on the connection
/// `builder` makes.
fn serve(builder: Builder<'_>, sender: CommandSender) -> anyhow::Result<Connection> {
    let item = Item { sender: sender.clone(), pixmaps: vec![status_notifier::pixmap(ICON_PNG)?] };
    Ok(builder.serve_at(ITEM_PATH, item)?.serve_at(MENU_PATH, Menu { sender })?.build()?)
}

/// Asks the desktop's StatusNotifierWatcher to show the item that `name` on `connection`
/// serves.
fn register(connection: &Connection, name: &str) -> zbus::Result<()> {
    let method = "RegisterStatusNotifierItem";
    connection.call_method(Some(WATCHER), WATCHER_PATH, Some(WATCHER), method, &name)?;
    Ok(())
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
        status_notifier::CATEGORY
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        status_notifier::ID
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        status_notifier::TITLE
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        status_notifier::STATUS
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
        (String::new(), Vec::new(), status_notifier::TITLE.to_owned(), String::new())
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
        if let Some(command) = status_notifier::chosen(id, event) {
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
        (MENU_REVISION, layout())
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
    let children = status_notifier::menu_ids().into_iter().filter_map(|id| {
        let child: Layout = (id, properties(id)?, Vec::new());
        OwnedValue::try_from(Value::from(child)).ok()
    });
    (ROOT_ID, properties(ROOT_ID).unwrap_or_default(), children.collect())
}

/// Item `id`'s properties; `None` for an id the menu doesn't have.
fn properties(id: i32) -> Option<Properties> {
    let (name, value) = Entry::from_id(id)?.property();
    let value = OwnedValue::try_from(Value::from(value)).ok()?;
    Some(HashMap::from([(name.to_owned(), value)]))
}

#[cfg(test)]
mod tests;
