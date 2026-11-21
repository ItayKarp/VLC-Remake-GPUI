//! The video menu's Playback and More Settings submenus (S2.9.3).
//!
//! **Playback:** Speed ▸ (common speeds, then Faster, Slower and Normal Speed), Chapter ▸ when
//! the media has chapters, Jump Forward and Jump Backward by the short skip, and Jump to Time,
//! which opens the current time for typing (S2.4). A-B loop isn't in the engine yet, so it
//! isn't listed.
//!
//! **More Settings:** the in-player settings panel's tabs (S2.7). The Settings window (S5.2)
//! joins it with its story.
//!
//! Both open things on the control bar, so they bring the controls up first; in Hide UI mode
//! there is no bar, and they say so.

use std::time::Duration;

use gpui::{App, Context, Entity, WeakEntity};

use super::{NOTHING_PLAYING, on_main, run};
use crate::assets::icons;
use crate::engine::media_info::Chapter;
use crate::engine::player::Player;
use crate::engine::values::SeekTarget;
use crate::keymap::actions as act;
use crate::main_window::MainWindow;
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::speed::rules;
use crate::ui::menu::MenuItem;

/// The speeds Speed ▸ offers, as VLC's menu does.
const SPEEDS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
/// Leeway when matching the playing speed to a listed one, for float noise.
const SPEED_EPSILON: f32 = 1e-3;
/// Why items that open something on the control bar are disabled in Hide UI mode.
const NO_CONTROLS: &str = "Leave Hide UI to use this";

impl MainWindow {
    /// Playback ▸.
    pub(super) fn playback_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let Some(player) = self.loaded_player(cx) else {
            return MenuItem::submenu("Playback", Vec::new())
                .icon(icons::PLAYBACK)
                .why_disabled(NOTHING_PLAYING);
        };
        let seekable = player.read(cx).is_seekable() != Some(false);
        let cant_seek = "This media can't seek";
        let mut items = vec![self.speed_menu(main, cx)];
        items.extend(chapter_menu(main, &player, cx));
        items.extend([
            MenuItem::Separator,
            run(main, &act::SeekForward, "Jump Forward", cx)
                .disabled(!seekable)
                .why_disabled(cant_seek),
            run(main, &act::SeekBackward, "Jump Backward", cx)
                .disabled(!seekable)
                .why_disabled(cant_seek),
            on_main(main, "Jump to Time…", |main, window, cx| {
                main.summon_controls(window, cx);
                main.start_time_edit(window, cx);
            })
            .disabled(!seekable || self.auto_hide.hide_ui)
            .why_disabled(if seekable { NO_CONTROLS } else { cant_seek }),
        ]);
        MenuItem::submenu("Playback", items).icon(icons::PLAYBACK)
    }

    /// Speed ▸: the listed speeds with the playing one checked, then the speed commands.
    fn speed_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let Some(player) = self.speed_player(cx) else {
            let live =
                self.loaded_player(cx).is_some_and(|p| p.read(cx).is_seekable() == Some(false));
            let why = if live { "Live streams play at normal speed" } else { NOTHING_PLAYING };
            return MenuItem::submenu("Speed", Vec::new()).why_disabled(why);
        };
        let rate = player.read(cx).rate().get();
        let mut items: Vec<MenuItem> = SPEEDS
            .iter()
            .map(|&speed| speed_choice(main, speed, (rate - speed).abs() < SPEED_EPSILON))
            .collect();
        items.extend([
            MenuItem::Separator,
            run(main, &act::Faster, "Faster", cx),
            run(main, &act::Slower, "Slower", cx),
            run(main, &act::NormalSpeed, "Normal Speed", cx),
        ]);
        MenuItem::submenu("Speed", items)
    }

    /// Plays at `speed`, picked from Speed ▸, and says so over the video.
    fn choose_speed(&mut self, speed: f32, cx: &mut Context<Self>) {
        self.set_speed(speed, cx);
        self.show_osd(rules::message(speed), cx);
    }

    /// More Settings ▸: a tab of the settings panel each.
    pub(super) fn more_settings_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let tabs = [
            (PanelTab::Playback, "Playback Settings…"),
            (PanelTab::Video, "Video Settings…"),
            (PanelTab::Audio, "Audio Settings…"),
            (PanelTab::Subtitles, "Subtitle Settings…"),
        ];
        let usable = self.has_active_media(cx) && !self.auto_hide.hide_ui;
        let items = tabs.into_iter().filter(|_| usable).map(|(tab, label)| {
            on_main(main, label, move |main, window, cx: &mut Context<MainWindow>| {
                main.summon_controls(window, cx);
                main.open_settings_panel(tab, window, cx);
            })
        });
        let why = if self.auto_hide.hide_ui { NO_CONTROLS } else { NOTHING_PLAYING };
        MenuItem::submenu("More Settings", items.collect()).icon(icons::SETTINGS).why_disabled(why)
    }
}

/// One speed in Speed ▸, checked when it is the one `playing`.
fn speed_choice(main: &WeakEntity<MainWindow>, speed: f32, playing: bool) -> MenuItem {
    let main = main.clone();
    MenuItem::radio(speed_label(speed), playing, move |_, cx| {
        main.update(cx, |main, cx| main.choose_speed(speed, cx)).ok();
    })
}

/// Chapter ▸, when the media has chapters: each one, with the playing one checked.
fn chapter_menu(
    main: &WeakEntity<MainWindow>,
    player: &Entity<Player>,
    cx: &App,
) -> Option<MenuItem> {
    let player = player.read(cx);
    let chapters = player.chapters();
    if chapters.is_empty() {
        return None;
    }
    let current = current_chapter(chapters, player.position());
    let items = chapters.iter().enumerate().map(|(index, chapter)| {
        let start = chapter.start;
        let main = main.clone();
        MenuItem::radio(chapter_label(chapter, index), current == Some(index), move |_, cx| {
            main.update(cx, |main, cx| main.seek_player(SeekTarget::Absolute(start), cx)).ok();
        })
    });
    Some(MenuItem::submenu("Chapter", items.collect()))
}

/// "1.25×", "1×".
fn speed_label(speed: f32) -> String {
    format!("{speed}×")
}

/// The chapter's title, or "Chapter 3" when the container names none.
fn chapter_label(chapter: &Chapter, index: usize) -> String {
    chapter.title.clone().unwrap_or_else(|| format!("Chapter {}", index + 1))
}

/// The chapter playing at `position`: the last one starting at or before it.
fn current_chapter(chapters: &[Chapter], position: Duration) -> Option<usize> {
    chapters.iter().rposition(|chapter| chapter.start <= position)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(title: Option<&str>, start: u64) -> Chapter {
        Chapter { title: title.map(str::to_owned), start: Duration::from_secs(start) }
    }

    #[test]
    fn the_current_chapter_is_the_last_one_started() {
        let chapters = [chapter(None, 5), chapter(None, 60), chapter(None, 120)];
        assert_eq!(current_chapter(&chapters, Duration::ZERO), None);
        assert_eq!(current_chapter(&chapters, Duration::from_secs(5)), Some(0));
        assert_eq!(current_chapter(&chapters, Duration::from_secs(119)), Some(1));
        assert_eq!(current_chapter(&chapters, Duration::from_secs(4000)), Some(2));
    }

    #[test]
    fn labels() {
        assert_eq!(chapter_label(&chapter(Some("Intro"), 0), 0), "Intro");
        assert_eq!(chapter_label(&chapter(None, 0), 2), "Chapter 3");
        assert_eq!(speed_label(1.0), "1×");
        assert_eq!(speed_label(0.25), "0.25×");
        assert_eq!(speed_label(1.5), "1.5×");
    }
}
