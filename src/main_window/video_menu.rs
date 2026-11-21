//! The video's right-click menu (S2.9.2, `menus/right_click_menu/right_click_menu.png`): Play,
//! Stop, Previous Item, Next Item, Hide UI, then the Playback, Video, Audio, Subtitle, More
//! Settings and Open Media submenus ([`playback`], [`media`]) and Quit VLC.
//!
//! Shift+F10 or the context-menu key opens it at the video's top-left corner with its first item
//! highlighted, rather than at the pointer. Opening it closes any popover or other menu first.
//! Playlist shows or hides the queue and clears it (S3.2); Repeat and Shuffle join it with S3.3.
//! Record (S8.6) joins with its story (S2.9.4).

mod media;
mod playback;

use gpui::{
    Action, AnyElement, App, Context, DismissEvent, Entity, MouseDownEvent, Pixels, Point,
    SharedString, Subscription, WeakEntity, Window, point, px,
};

use super::MainWindow;
use super::main_menu;
use crate::assets::icons;
use crate::keymap;
use crate::keymap::actions as act;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover;

/// The video's right-click menu and where it opened.
pub(super) type VideoMenu = (Entity<ContextMenu>, Subscription, Point<Pixels>);

/// How far inside the video's top-left corner a menu opened from the keyboard appears.
const KEYBOARD_INSET: f32 = 16.;

/// Why items that need open media are disabled.
const NOTHING_PLAYING: &str = "No media playing";

impl MainWindow {
    /// A right-click on the video: the menu at the pointer.
    pub(super) fn on_video_right_click(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_video_menu(event.position, false, window, cx);
    }

    /// Shift+F10 or the context-menu key: the menu at the video, where the keyboard is.
    pub(super) fn show_context_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.showing_video(cx).is_none() {
            return;
        }
        let Some(video) = self.auto_hide.video_area.bounds() else { return };
        let at = video.origin + point(px(KEYBOARD_INSET), px(KEYBOARD_INSET));
        self.open_video_menu(at, true, window, cx);
    }

    fn open_video_menu(
        &mut self,
        position: Point<Pixels>,
        from_keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_popovers(window, cx);
        self.control_bar.close_overflow();
        self.menu = None;
        let items = self.video_menu_items(window, cx);
        let menu = ContextMenu::build(items, window, cx);
        if from_keyboard {
            menu.update(cx, ContextMenu::highlight_first);
        }
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.auto_hide.video_menu = None;
            cx.notify();
        });
        self.auto_hide.video_menu = Some((menu, subscription, position));
        cx.notify();
    }

    /// The video's right-click menu, where it opened.
    pub(super) fn render_video_menu(&self) -> Option<AnyElement> {
        let (menu, _, position) = self.auto_hide.video_menu.as_ref()?;
        Some(popover::at_position(*position, 1, menu.clone()))
    }

    fn video_menu_items(&self, window: &Window, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let main = cx.entity().downgrade();
        let mut items = self.transport_items(&main, cx);
        let hide_ui = if self.auto_hide.hide_ui { "Show UI" } else { "Hide UI" };
        items.extend([
            run(&main, &act::HideInterface, hide_ui, cx).icon(icons::HIDE_UI),
            self.playback_menu(&main, cx),
            self.video_submenu(&main, window, cx),
            self.audio_submenu(&main, cx),
            self.subtitle_submenu(&main, cx),
            self.more_settings_menu(&main, cx),
            self.playlist_menu(&main, cx),
            open_media_menu(&main, cx),
            main_menu::dispatching(&act::Quit, "Quit VLC", cx).icon(icons::QUIT),
        ]);
        items
    }

    /// Play (Pause while playing), Stop, Previous Item and Next Item.
    fn transport_items(&self, main: &WeakEntity<Self>, cx: &App) -> Vec<MenuItem> {
        let player = self.loaded_player(cx);
        let playing = player.as_ref().is_some_and(|player| player.read(cx).wants_to_play());
        let active = player.is_some_and(|player| player.read(cx).state().is_active());
        let (play, play_icon) =
            if playing { ("Pause", icons::PAUSE) } else { ("Play", icons::PLAY) };
        vec![
            run(main, &act::PlayPause, play, cx).icon(play_icon),
            run(main, &act::Stop, "Stop", cx)
                .icon(icons::STOP)
                .disabled(!active)
                .why_disabled(NOTHING_PLAYING),
            run(main, &act::PreviousItem, "Previous Item", cx)
                .icon(icons::PREVIOUS)
                .disabled(!self.can_go_back(cx))
                .why_disabled("No previous item"),
            run(main, &act::NextItem, "Next Item", cx)
                .icon(icons::NEXT)
                .disabled(!self.can_go_next())
                .why_disabled("No next item"),
        ]
    }
}

impl MainWindow {
    /// Playlist: show or hide the queue, and clear it.
    fn playlist_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let show = if self.queue_panel.is_open() { "Hide Queue" } else { "Show Queue" };
        let items = vec![
            run(main, &act::ToggleQueue, show, cx).icon(icons::QUEUE),
            on_main(main, "Clear Queue", |this, _, cx| this.clear_queue(cx))
                .disabled(self.queue.len() == 0)
                .why_disabled("The queue is empty"),
        ];
        MenuItem::submenu("Playlist", items).icon(icons::LIST)
    }
}

/// An item that runs the player command `action` on `main` (as its key does, but every choice
/// counts), with the command's shortcut.
fn run(
    main: &WeakEntity<MainWindow>,
    action: &'static dyn Action,
    label: impl Into<SharedString>,
    cx: &App,
) -> MenuItem {
    let main = main.clone();
    MenuItem::action(label, move |window, cx| {
        main.update(cx, |main, cx| main.perform(action, window, cx)).ok();
    })
    .maybe_shortcut(keymap::shortcut(action, cx))
}

/// An item that runs `f` on `main`.
fn on_main(
    main: &WeakEntity<MainWindow>,
    label: impl Into<SharedString>,
    f: impl Fn(&mut MainWindow, &mut Window, &mut Context<MainWindow>) + 'static,
) -> MenuItem {
    let main = main.clone();
    MenuItem::action(label, move |window, cx| {
        main.update(cx, |main, cx| f(main, window, cx)).ok();
    })
}

/// Open Media: the open commands, Add to Queue and Play Next (S3.1.2), and Open Recent.
fn open_media_menu(main: &WeakEntity<MainWindow>, cx: &App) -> MenuItem {
    let items = vec![
        main_menu::dispatching(&act::OpenFile, "Open File(s)…", cx).icon(icons::FILE),
        main_menu::dispatching(&act::OpenFolder, "Open Folder…", cx).icon(icons::FOLDER),
        main_menu::dispatching(&act::OpenFromClipboard, "Open from Clipboard", cx),
        MenuItem::Separator,
        on_main(main, "Add to Queue…", |_, window, cx| MainWindow::prompt_to_enqueue(window, cx)),
        on_main(main, "Play Next…", |_, window, cx| MainWindow::prompt_to_play_next(window, cx)),
        MenuItem::Separator,
        main_menu::recent_menu(main, cx).icon(icons::RECENT),
    ];
    MenuItem::submenu("Open Media", items).icon(icons::FILE)
}
