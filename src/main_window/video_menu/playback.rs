//! The video menu's Playback and More Settings submenus (S2.9.3).
//!
//! **Playback:** Speed ▸ (common speeds, then Faster, Slower and Normal Speed), Chapter ▸ when
//! the media has chapters (S8.2), Title ▸ and Program ▸ when it has several (S8.9), Jump Forward and Jump Backward by the short skip, and Jump to Time,
//! which opens the current time for typing (S2.4), and Add Bookmark (S8.3.2). A-B loop isn't in
//! the engine yet, so it isn't listed.
//!
//! **More Settings:** the in-player settings panel's tabs (S2.7). The Settings window (S5.2)
//! joins it with its story.
//!
//! Both open things on the control bar, so they bring the controls up first; in Hide UI mode
//! there is no bar, and they say so.

use std::time::Duration;

use gpui::{App, Context, WeakEntity};

use super::{NOTHING_PLAYING, disc, on_main, run};
use crate::assets::icons;
use crate::i18n;
use crate::keymap::actions as act;
use crate::main_window::MainWindow;
use crate::main_window::seeking::chapters::{self, Listed};
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::speed::rules;
use crate::ui::menu::MenuItem;

/// The speeds Speed ▸ offers, as VLC's menu does.
const SPEEDS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
/// Leeway when matching the playing speed to a listed one, for float noise.
const SPEED_EPSILON: f32 = 1e-3;
/// Why items that open something on the control bar are disabled in Hide UI mode.
const NO_CONTROLS: &str = tr_noop!("Leave Hide UI to use this");

impl MainWindow {
    /// Playback ▸.
    pub(super) fn playback_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let Some(player) = self.loaded_player(cx) else {
            return MenuItem::submenu(tr!("Playback"), Vec::new())
                .icon(icons::PLAYBACK)
                .why_disabled(i18n::text(NOTHING_PLAYING));
        };
        let seekable = player.read(cx).is_seekable() != Some(false);
        let cant_seek = tr!("This media can't seek");
        let mut items = vec![self.speed_menu(main, cx)];
        let position = player.read(cx).position();
        items.extend(chapter_menu(main, self.chapters(cx), position, cx));
        items.extend(disc::title_menu(main, player.read(cx).titles(), cx));
        items.extend(disc::program_menu(main, player.read(cx).programs(), cx));
        items.extend([
            MenuItem::Separator,
            run(main, &act::SeekForward, tr!("Jump Forward"), cx)
                .disabled(!seekable)
                .why_disabled(cant_seek),
            run(main, &act::SeekBackward, tr!("Jump Backward"), cx)
                .disabled(!seekable)
                .why_disabled(cant_seek),
            on_main(main, tr!("Jump to Time…"), |main, window, cx| {
                main.summon_controls(window, cx);
                main.start_time_edit(window, cx);
            })
            .disabled(!seekable || self.auto_hide.hide_ui)
            .why_disabled(if seekable { i18n::text(NO_CONTROLS) } else { cant_seek }),
            run(main, &act::AddBookmark, tr!("Add Bookmark"), cx)
                .disabled(!seekable)
                .why_disabled(cant_seek),
        ]);
        MenuItem::submenu(tr!("Playback"), items).icon(icons::PLAYBACK)
    }

    /// Speed ▸: the listed speeds with the playing one checked, then the speed commands.
    fn speed_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let Some(player) = self.speed_player(cx) else {
            let live =
                self.loaded_player(cx).is_some_and(|p| p.read(cx).is_seekable() == Some(false));
            let why = if live {
                tr!("Live streams play at normal speed")
            } else {
                i18n::text(NOTHING_PLAYING)
            };
            return MenuItem::submenu(tr!("Speed"), Vec::new()).why_disabled(why);
        };
        let rate = player.read(cx).rate().get();
        let mut items: Vec<MenuItem> = SPEEDS
            .iter()
            .map(|&speed| speed_choice(main, speed, (rate - speed).abs() < SPEED_EPSILON))
            .collect();
        items.extend([
            MenuItem::Separator,
            run(main, &act::Faster, tr!("Faster"), cx),
            run(main, &act::Slower, tr!("Slower"), cx),
            run(main, &act::NormalSpeed, tr!("Normal Speed"), cx),
        ]);
        MenuItem::submenu(tr!("Speed"), items)
    }

    /// Plays at `speed`, picked from Speed ▸, and says so over the video.
    fn choose_speed(&mut self, speed: f32, cx: &mut Context<Self>) {
        self.set_speed(speed, cx);
        self.show_osd(rules::message(speed), cx);
    }

    /// More Settings ▸: a tab of the settings panel each.
    pub(super) fn more_settings_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let tabs = [
            (PanelTab::Playback, tr!("Playback Settings…")),
            (PanelTab::Video, tr!("Video Settings…")),
            (PanelTab::Audio, tr!("Audio Settings…")),
            (PanelTab::Subtitles, tr!("Subtitle Settings…")),
        ];
        let usable = self.has_active_media(cx) && !self.auto_hide.hide_ui;
        let items = tabs.into_iter().filter(|_| usable).map(|(tab, label)| {
            on_main(main, label, move |main, window, cx: &mut Context<MainWindow>| {
                main.summon_controls(window, cx);
                main.open_settings_panel(tab, window, cx);
            })
        });
        let why = i18n::text(if self.auto_hide.hide_ui { NO_CONTROLS } else { NOTHING_PLAYING });
        let menu = MenuItem::submenu(tr!("More Settings"), items.collect());
        menu.icon(icons::SETTINGS).why_disabled(why)
    }
}

/// One speed in Speed ▸, checked when it is the one `playing`.
fn speed_choice(main: &WeakEntity<MainWindow>, speed: f32, playing: bool) -> MenuItem {
    let main = main.clone();
    MenuItem::radio(speed_label(speed), playing, move |_, cx| {
        main.update(cx, |main, cx| main.choose_speed(speed, cx)).ok();
    })
}

/// Chapter ▸, when the media has chapters: each one, with the playing one checked, then Next and
/// Previous Chapter (S8.2.3).
fn chapter_menu(
    main: &WeakEntity<MainWindow>,
    listed: Vec<Listed>,
    position: Duration,
    cx: &App,
) -> Option<MenuItem> {
    if listed.is_empty() {
        return None;
    }
    let current = chapters::current(&listed, position);
    let mut items: Vec<MenuItem> = listed
        .into_iter()
        .enumerate()
        .map(|(index, chapter)| {
            let main = main.clone();
            MenuItem::radio(chapter.title.clone(), current == Some(index), move |_, cx| {
                main.update(cx, |main, cx| main.seek_to_chapter(&chapter, cx)).ok();
            })
        })
        .collect();
    items.extend([
        MenuItem::Separator,
        run(main, &act::NextChapter, tr!("Next Chapter"), cx),
        run(main, &act::PreviousChapter, tr!("Previous Chapter"), cx),
    ]);
    Some(MenuItem::submenu(tr!("Chapter"), items))
}

/// "1.25×", "1×".
fn speed_label(speed: f32) -> String {
    format!("{}×", rules::number(f64::from(speed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(speed_label(1.0), "1×");
        assert_eq!(speed_label(0.25), "0.25×");
        assert_eq!(speed_label(1.5), "1.5×");
    }
}
