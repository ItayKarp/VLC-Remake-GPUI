//! The main window shell (S0.3.4): the player-style title bar and an empty state
//! that opens or accepts dropped files. Onboarding hands off to it (S0.2); the
//! player (Stage 2) and the library (Stage 4) fill the content area later.

use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, DismissEvent, Entity, ExternalPaths, FocusHandle,
    PathPromptOptions, SharedString, Subscription, Window, div, prelude::*, px, svg,
};

use crate::app::{self, MAIN_WINDOW_CONTEXT, OpenFile};
use crate::assets::icons;
use crate::settings::{self, Notice};
use crate::theme::{self, Palette};
use crate::ui::button::{Button, IconButton};
use crate::ui::focus;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, BoundsTracker, Side};
use crate::ui::toast::{Level, banner};
use crate::window_chrome::{self, Caption, TRAFFIC_LIGHTS_WIDTH};

/// The player title bar's height in the designs (`player/screens/*`).
pub const TITLE_BAR_HEIGHT: f32 = 40.;

const APP_TITLE: &str = "VLC Media Player";

/// Title bar buttons on the left: 28px square, 4px apart.
const BAR_BUTTON: f32 = 28.;
const BAR_PAD: f32 = 8.;

pub struct MainWindow {
    /// Files the app was launched with, opened or dropped. Played once the engine
    /// exists (Stage 1).
    files: Vec<PathBuf>,
    /// Settings notices (corrupt file recovered, changes not saved, …).
    notices: Vec<SharedString>,
    focus_handle: FocusHandle,
    open_button: FocusHandle,
    /// The `…` menu, while it's open.
    menu: Option<(Entity<ContextMenu>, Subscription)>,
    menu_button: BoundsTracker,
}

impl MainWindow {
    pub fn new(files: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        // Swap the Maximize/Restore icon when the window is maximized or restored.
        cx.observe_window_bounds(window, |_, _, cx| cx.notify())
            .detach();
        app::save_geometry_on_change(window, cx, |s| &mut s.local.window.main);

        let open_button = cx.focus_handle().tab_stop(true).tab_index(0);
        open_button.focus(window, cx);

        let notices = settings::take_notices(cx)
            .into_iter()
            .map(|notice| notice_text(&notice).into())
            .collect();
        let this = Self {
            files: Vec::new(),
            notices,
            focus_handle: cx.focus_handle(),
            open_button,
            menu: None,
            menu_button: BoundsTracker::new(),
        };
        let mut this = this;
        this.add_files(files, window);
        this
    }

    fn add_files(&mut self, files: impl IntoIterator<Item = PathBuf>, window: &mut Window) {
        self.files.extend(files);
        window.set_window_title(&self.title());
    }

    fn title(&self) -> String {
        match self.files.last() {
            Some(file) => file_name(file),
            None => APP_TITLE.to_owned(),
        }
    }

    fn open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                this.update_in(cx, |this, window, cx| {
                    this.add_files(paths, window);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    fn on_drop(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        self.add_files(paths.paths().iter().cloned(), window);
        cx.notify();
    }

    fn toggle_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        let this = cx.entity().downgrade();
        let display = window.display(cx).map(|d| d.id());
        let items = vec![
            MenuItem::action("Open file…", move |window, cx| {
                this.update(cx, |this, cx| this.open_file(&OpenFile, window, cx))
                    .ok();
            })
            .shortcut(open_hint()),
            MenuItem::action("Run setup again", move |_, cx| {
                cx.defer(move |cx| app::run_setup_again(display, cx))
            }),
        ];
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        self.menu = Some((menu, subscription));
        cx.notify();
    }
}

fn open_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘O"
    } else {
        "Ctrl+O"
    }
}

fn file_name(file: &std::path::Path) -> String {
    file.file_name()
        .unwrap_or(file.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn notice_text(notice: &Notice) -> String {
    match notice {
        Notice::Corrupt { file, moved_to, .. } => match moved_to {
            Some(moved) => format!(
                "{} couldn't be read and was reset. The old file was kept as {}.",
                file.display(),
                moved.display()
            ),
            None => format!(
                "{} couldn't be read; settings won't be saved.",
                file.display()
            ),
        },
        Notice::RestoredFromBackup { file } => {
            format!(
                "{} was empty and was restored from its backup.",
                file.display()
            )
        }
        Notice::NotPersisted { reason } => format!("Changes won't be saved. {reason}"),
        Notice::SaveFailed { error } => format!("Settings couldn't be saved: {error}"),
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);

        focus::tab_navigation(div())
            .id("main-window")
            .key_context(MAIN_WINDOW_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_file))
            .relative()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .child(
                div()
                    .id("content")
                    .absolute()
                    .top(px(TITLE_BAR_HEIGHT))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .overflow_y_scroll()
                    .on_drop(cx.listener(Self::on_drop))
                    .child(self.render_empty_state(&p, cx)),
            )
            .child(self.render_title_bar(&p, window, cx))
            .children(self.render_menu())
    }
}

impl MainWindow {
    fn render_title_bar(&self, p: &Palette, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let left_inset = if cfg!(target_os = "macos") {
            TRAFFIC_LIGHTS_WIDTH
        } else {
            BAR_PAD
        };
        let folder = settings::get(cx)
            .local
            .media_folders
            .first()
            .map(|folder| SharedString::from(file_name(folder)));

        let left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .pl(px(left_inset))
            .child(
                div()
                    .size(px(BAR_BUTTON))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(icons::LOGO_SMALL)
                            .size(px(18.))
                            .text_color(p.accent_fg),
                    ),
            )
            .child(
                div().relative().child(self.menu_button.element()).child(
                    IconButton::new("menu", icons::MORE, "Menu")
                        .size(BAR_BUTTON, 18.)
                        .selected(self.menu.is_some())
                        .in_title_bar()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.toggle_menu(window, cx)
                        })),
                ),
            )
            .when_some(folder, |left, folder| {
                left.child(div().mx(px(6.)).w(px(1.)).h(px(16.)).bg(p.divider))
                    .child(
                        // The folder the library opens on (Stage 4).
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .px(px(4.))
                            .child(svg().path(icons::FOLDER).size(px(18.)).text_color(p.icon))
                            .child(div().text_size(px(13.)).text_color(p.text).child(folder)),
                    )
            });

        let caption = window_chrome::caption_buttons(
            &[
                Caption::Minimize,
                Caption::Maximize,
                Caption::Fullscreen,
                Caption::Close,
            ],
            window.is_maximized(),
            p,
        );

        window_chrome::drag_bar("title-bar", TITLE_BAR_HEIGHT)
            .bg(p.title_bar)
            .justify_between()
            .child(left)
            .child(div().flex().flex_row().h_full().children(caption))
            .child(
                // Centered on the window, not between the side groups.
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .max_w(px(480.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_size(px(13.))
                            .text_color(p.text_strong)
                            .child(SharedString::from(self.title())),
                    ),
            )
            .into_any_element()
    }

    /// The `…` menu, below its button.
    fn render_menu(&self) -> Option<AnyElement> {
        let (menu, _) = self.menu.as_ref()?;
        let button = self.menu_button.bounds()?;
        Some(popover::attached(button, Side::Below, 4., 1, menu.clone()))
    }

    fn render_empty_state(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let hover = p.hover;
        let accent_fg = p.accent_fg;

        let drop_zone = div()
            .id("drop-zone")
            .w(px(520.))
            .max_w_full()
            .py(px(48.))
            .px(px(32.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .rounded(px(12.))
            .border_2()
            .border_dashed()
            .border_color(p.key_border)
            .drag_over::<ExternalPaths>(move |style, _, _, _| {
                style.border_color(accent_fg).bg(hover)
            })
            .child(svg().path(icons::FILE).size(px(48.)).text_color(p.icon))
            .child(
                div()
                    .text_size(px(20.))
                    .text_color(p.text_strong)
                    .child("Drop files here to play"),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(p.text_muted)
                    .child("or"),
            )
            .child(
                Button::new("open-file", "Open file…")
                    .primary()
                    .track_focus(&self.open_button)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.open_file(&OpenFile, window, cx)
                    })),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child(open_hint()),
            );

        let opened = self.files.iter().map(|file| {
            div()
                .text_size(px(14.))
                .text_color(p.text_muted)
                .child(SharedString::from(format!("Opened {}", file_name(file))))
        });

        div()
            .min_h_full()
            .p(px(24.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .children(self.notices.iter().enumerate().map(|(i, notice)| {
                let this = cx.entity().downgrade();
                banner(("notice", i), Level::Warning, notice.clone()).on_dismiss(move |_, cx| {
                    this.update(cx, |this, cx| {
                        if i < this.notices.len() {
                            this.notices.remove(i);
                        }
                        cx.notify();
                    })
                    .ok();
                })
            }))
            .child(drop_zone)
            .children(opened)
            .into_any_element()
    }
}
