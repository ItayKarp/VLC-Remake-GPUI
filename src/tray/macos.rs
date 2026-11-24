//! The menu-bar icon on macOS: an `NSStatusItem` showing the cone as a template image (drawn in
//! the menu bar's own color), whose menu's items call a small Objective-C target that sends the
//! chosen command.

use anyhow::Context as _;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{
    AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
};
use objc2::{ClassType, sel};
use objc2_app_kit::{
    NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSData, NSObjectProtocol, NSSize, NSString};

use super::{CommandSender, ICON_PNG, TrayCommand};

/// The icon's size in the menu bar, in points: AppKit's usual for status items.
const ICON_POINTS: f64 = 18.;

define_class!(
    /// Receives the menu's clicks: each item carries its command's id as its tag.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RedesignedVlcTrayTarget"]
    #[ivars = CommandSender]
    struct Target;

    unsafe impl NSObjectProtocol for Target {}

    impl Target {
        #[unsafe(method(choose:))]
        fn choose(&self, item: &NSMenuItem) {
            if let Some(command) = TrayCommand::from_id(item.tag() as i64) {
                self.ivars().unbounded_send(command).ok();
            }
        }
    }
);

impl Target {
    fn new(sender: CommandSender, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(sender);
        // SAFETY: `NSObject`'s `init` on a freshly allocated instance with its ivars set.
        unsafe { msg_send![super(this), init] }
    }
}

/// The status item; removed from the menu bar when dropped.
pub struct Icon {
    item: Retained<NSStatusItem>,
    /// The menu items' target, which they don't retain.
    _target: Retained<Target>,
}

impl Icon {
    pub fn show(sender: CommandSender) -> anyhow::Result<Self> {
        let mtm = MainThreadMarker::new().context("the menu-bar icon needs the main thread")?;
        let target = Target::new(sender, mtm);
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            button.setImage(icon().as_deref());
            button.setToolTip(Some(&NSString::from_str("VLC")));
        }
        item.setMenu(Some(&menu(&target, mtm)));
        Ok(Self { item, _target: target })
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
    }
}

/// The cone as a template image, so the menu bar tints it.
fn icon() -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(ICON_PNG);
    let image = NSImage::initWithData(NSImage::alloc(), &data)?;
    image.setTemplate(true);
    image.setSize(NSSize::new(ICON_POINTS, ICON_POINTS));
    Some(image)
}

/// The icon's menu, its items aimed at `target`.
fn menu(target: &Target, mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    for (index, group) in TrayCommand::MENU.iter().enumerate() {
        if index > 0 {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
        }
        for command in *group {
            menu.addItem(&menu_item(*command, target, mtm));
        }
    }
    menu
}

fn menu_item(command: TrayCommand, target: &Target, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let title = NSString::from_str(command.label());
    // SAFETY: `choose:` is `Target`'s method taking the item, and the target outlives the menu.
    unsafe {
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &title,
            Some(sel!(choose:)),
            &NSString::new(),
        );
        item.setTarget(Some(target.as_super().as_ref() as &AnyObject));
        item.setTag(command.id() as isize);
        item
    }
}

/// The menu bar is always there.
pub fn is_available() -> bool {
    true
}
