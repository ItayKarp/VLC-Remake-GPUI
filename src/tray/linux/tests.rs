//! The StatusNotifierItem and its dbusmenu as a tray host sees them: their methods and
//! properties called directly, and the item served and registered over a private D-Bus
//! connection, since a test can't count on a session bus or a tray to register with.

use std::os::unix::net::UnixStream;
use std::sync::mpsc;

use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::interface;
use zbus::zvariant::{OwnedValue, Str};

use super::*;

fn item() -> (Item, UnboundedReceiver<TrayCommand>) {
    let (sender, receiver) = unbounded();
    let pixmaps = vec![status_notifier::pixmap(ICON_PNG).unwrap()];
    (Item { sender, pixmaps }, receiver)
}

fn menu() -> (Menu, UnboundedReceiver<TrayCommand>) {
    let (sender, receiver) = unbounded();
    (Menu { sender }, receiver)
}

/// The commands sent so far, oldest first.
fn sent(receiver: &mut UnboundedReceiver<TrayCommand>) -> Vec<TrayCommand> {
    std::iter::from_fn(|| receiver.try_recv().ok()).collect()
}

fn text(value: &OwnedValue) -> String {
    value.downcast_ref::<Str<'_>>().unwrap().to_string()
}

fn child_ids(layout: &Layout) -> Vec<i32> {
    let child = |value: &OwnedValue| Layout::try_from(value.try_clone().unwrap()).unwrap().0;
    layout.2.iter().map(child).collect()
}

#[test]
fn clicking_the_icon_shows_vlc_and_a_middle_click_plays_or_pauses() {
    let (item, mut receiver) = item();
    item.activate(4, 2);
    item.secondary_activate(4, 2);
    item.context_menu(4, 2);
    item.scroll(1, "vertical");
    assert_eq!(sent(&mut receiver), [TrayCommand::ShowVlc, TrayCommand::PlayPause]);
}

#[test]
fn the_item_describes_vlc_with_the_cone_and_its_menu() {
    let (item, _receiver) = item();
    assert_eq!(item.category(), status_notifier::CATEGORY);
    assert_eq!(item.id(), status_notifier::ID);
    assert_eq!(item.title(), status_notifier::TITLE);
    assert_eq!(item.status(), status_notifier::STATUS);
    assert_eq!(item.icon_name(), "", "the pixmap stands in for a themed icon");
    assert_eq!(item.icon_pixmap(), [status_notifier::pixmap(ICON_PNG).unwrap()]);
    assert_eq!(item.tool_tip().2, status_notifier::TITLE);
    assert!(!item.item_is_menu(), "a click activates; the menu is the context menu");
    assert_eq!(item.menu().as_str(), MENU_PATH);
}

#[test]
fn the_layout_is_the_root_holding_the_menu() {
    let (menu, _receiver) = menu();
    let (revision, layout) = menu.get_layout(ROOT_ID, -1, Vec::new());
    assert_eq!(revision, MENU_REVISION);
    assert_eq!(layout.0, ROOT_ID);
    assert_eq!(text(&layout.1["children-display"]), "submenu");
    assert_eq!(child_ids(&layout), status_notifier::menu_ids());
}

#[test]
fn properties_are_given_for_the_items_the_menu_has() {
    let (menu, _receiver) = menu();
    let quit = TrayCommand::Quit.id();
    let group = menu.get_group_properties(vec![quit, 999], Vec::new());
    let ids: Vec<_> = group.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [quit]);
    assert_eq!(text(&group[0].1["label"]), TrayCommand::Quit.label());
    let label = menu.get_property(quit, "label".to_owned()).unwrap();
    assert_eq!(text(&label), TrayCommand::Quit.label());
    assert!(menu.get_property(quit, "icon-name".to_owned()).is_err());
    assert!(menu.get_property(999, "label".to_owned()).is_err());
}

#[test]
fn clicked_items_send_their_commands() {
    let (menu, mut receiver) = menu();
    let data = || OwnedValue::from(0_i32);
    menu.event(TrayCommand::Next.id(), "clicked".to_owned(), data(), 0);
    menu.event(TrayCommand::Next.id(), "hovered".to_owned(), data(), 0);
    let events = vec![
        (TrayCommand::Previous.id(), "clicked".to_owned(), data(), 0),
        (ROOT_ID, "clicked".to_owned(), data(), 0),
    ];
    assert_eq!(menu.event_group(events), Vec::<i32>::new(), "no id is unknown");
    assert_eq!(sent(&mut receiver), [TrayCommand::Next, TrayCommand::Previous]);
}

#[test]
fn the_menu_never_changes_before_showing() {
    let (menu, _receiver) = menu();
    assert!(!menu.about_to_show(ROOT_ID));
    assert_eq!(menu.about_to_show_group(vec![ROOT_ID]), (Vec::new(), Vec::new()));
    assert_eq!(menu.version(), DBUSMENU_VERSION);
    assert_eq!(menu.text_direction(), "ltr");
    assert_eq!(menu.status(), "normal");
    assert_eq!(menu.icon_theme_path(), Vec::<String>::new());
}

#[test]
fn availability_is_asked_once() {
    assert_eq!(is_available(), is_available());
}

/// A StatusNotifierWatcher that passes on the names registered with it.
struct Watcher {
    registered: mpsc::Sender<String>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: String) {
        self.registered.send(service).unwrap();
    }
}

/// The icon served on one end of a socket and a watcher on the other, as a tray host would.
fn served() -> (Connection, Connection, mpsc::Receiver<String>, UnboundedReceiver<TrayCommand>) {
    let (icon_end, host_end) = UnixStream::pair().unwrap();
    let (sender, commands) = unbounded();
    let (registered, names) = mpsc::channel();
    let guid = zbus::Guid::generate();
    let host = std::thread::spawn(move || {
        let builder = Builder::async_io_unix_stream(host_end).server(guid).unwrap().p2p();
        builder.serve_at(WATCHER_PATH, Watcher { registered }).unwrap().build().unwrap()
    });
    let icon = serve(Builder::async_io_unix_stream(icon_end).p2p(), sender).unwrap();
    (icon, host.join().unwrap(), names, commands)
}

#[test]
fn the_served_icon_registers_and_answers_the_tray() {
    let (icon, host, names, mut commands) = served();
    register(&icon, ":1.42").unwrap();
    assert_eq!(names.recv().unwrap(), ":1.42");

    let item = Some("org.kde.StatusNotifierItem");
    host.call_method(None::<&str>, ITEM_PATH, item, "Activate", &(0_i32, 0_i32)).unwrap();
    let reply = host
        .call_method(None::<&str>, MENU_PATH, Some("com.canonical.dbusmenu"), "AboutToShow", &0)
        .unwrap();
    assert!(!reply.body().deserialize::<bool>().unwrap());
    assert_eq!(sent(&mut commands), [TrayCommand::ShowVlc]);
}
