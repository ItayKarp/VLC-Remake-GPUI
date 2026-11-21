//! The main window: a minimal placeholder that onboarding hands off to (S0.2).
//! S0.3.4 replaces it with the real shell (player title bar, empty state, drop
//! target), and Stage 4 with the library.

use std::path::PathBuf;

use gpui::{
    ClickEvent, Context, FontWeight, Hsla, SharedString, Window, div, prelude::*, px, rgb, svg,
};

use crate::app;
use crate::assets::icons;
use crate::settings::{self, Notice};
use crate::theme::Palette;
use crate::window_chrome::{self, TITLE_BAR_HEIGHT};

pub struct MainWindow {
    /// Files the app was launched with. Played once the engine exists (Stage 1).
    files: Vec<PathBuf>,
    /// Settings notices (corrupt file recovered, changes not saved, …).
    notices: Vec<SharedString>,
}

impl MainWindow {
    pub fn new(files: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Theme and accent follow the settings, and "System" follows the OS.
        cx.observe_global::<settings::SettingsStore>(|_, cx| cx.notify())
            .detach();
        cx.observe_window_appearance(window, |_, _, cx| cx.notify())
            .detach();
        app::save_geometry_on_change(window, cx, |s| &mut s.local.window.main);

        let notices = settings::take_notices(cx)
            .into_iter()
            .map(|notice| notice_text(&notice).into())
            .collect();
        Self { files, notices }
    }

    fn palette(&self, window: &Window, cx: &Context<Self>) -> Palette {
        let prefs = &settings::get(cx).prefs;
        Palette::new(prefs.theme.is_dark(window.appearance()), prefs.accent)
    }
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
        let p = self.palette(window, cx);
        let warning: Hsla = rgb(0xE5A50A).into();

        let opened = self.files.iter().map(|file| {
            let name = file
                .file_name()
                .unwrap_or(file.as_os_str())
                .to_string_lossy()
                .into_owned();
            div()
                .text_size(px(14.))
                .text_color(p.text_muted)
                .child(SharedString::from(format!("Opened {name}")))
        });

        div()
            .id("main-window")
            .relative()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .child(
                div()
                    .absolute()
                    .top(px(TITLE_BAR_HEIGHT))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(16.))
                    .children(self.notices.iter().map(|notice| {
                        div()
                            .max_w(px(640.))
                            .px(px(14.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(warning)
                            .text_size(px(14.))
                            .child(notice.clone())
                    }))
                    .child(svg().path(icons::LOGO).size(px(96.)).text_color(p.accent))
                    .child(
                        div()
                            .text_size(px(28.))
                            .font_weight(FontWeight::LIGHT)
                            .text_color(p.text_strong)
                            .child("VLC Media Player"),
                    )
                    .children(opened)
                    .child(
                        div()
                            .id("run-setup-again")
                            .mt(px(16.))
                            .px(px(16.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .bg(p.button)
                            .hover(|style| style.bg(p.button_hover))
                            .cursor_pointer()
                            .text_size(px(15.))
                            .text_color(p.text_muted)
                            .child("Run setup again")
                            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                                let display = window.display(cx).map(|d| d.id());
                                cx.defer(move |cx| app::run_setup_again(display, cx));
                            })),
                    ),
            )
            .child(window_chrome::title_bar(&p))
    }
}
