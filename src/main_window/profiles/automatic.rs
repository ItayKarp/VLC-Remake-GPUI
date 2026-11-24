//! Switching profile by itself (S8.8.3): a profile can be set to be used for music, or for
//! video, and media of that kind switches to it as it starts. The library's sections follow,
//! as its Music section plays music.
//!
//! The profile the user had comes back when media no profile is set for plays, so a song
//! switching to Music doesn't leave the next film in it. Each media is decided once: a profile
//! the user picks while it plays stays until the next.

use gpui::{App, Context, Window};

use crate::app::media::Media;
use crate::main_window::MainWindow;
use crate::settings::{Automatic, profiles};
use crate::ui::menu::MenuItem;

/// What switched profile by itself, and what to go back to.
#[derive(Default)]
pub(in crate::main_window) struct AutoProfile {
    /// The media last decided for, and its kind.
    decided: Option<(Media, Automatic)>,
    /// The profile in use before media switched it.
    before: Option<String>,
}

impl AutoProfile {
    /// Forgets the profile to go back to: the user chose one.
    pub(in crate::main_window) fn forget(&mut self) {
        self.before = None;
    }
}

/// Use Automatically ▸ for profile `name`: For Music and For Videos, each a check item.
pub(super) fn menu(name: &str, cx: &App) -> MenuItem {
    let list = profiles::list(cx);
    let current = list.iter().find(|profile| profile.name == name);
    let set = current.map_or(Automatic::Never, |profile| profile.automatic);
    let item = |label: &'static str, kind: Automatic, hint: String| {
        let name = name.to_owned();
        MenuItem::check(label, set == kind, move |_, cx| toggle(&name, kind, cx)).hint(hint)
    };
    let items = vec![
        item(
            tr!("For Music"),
            Automatic::Music,
            tr!("Songs and other audio switch to “{name}”", name = super::shown(name)),
        ),
        item(
            tr!("For Videos"),
            Automatic::Video,
            tr!("Videos switch to “{name}”", name = super::shown(name)),
        ),
    ];
    MenuItem::submenu(tr!("Use Automatically"), items)
}

/// Makes `kind` of media switch to profile `name`, or stops it when it already does.
fn toggle(name: &str, kind: Automatic, cx: &mut App) {
    let list = profiles::list(cx);
    let on = list.iter().any(|profile| profile.name == name && profile.automatic == kind);
    profiles::set_automatic(cx, name, if on { Automatic::Never } else { kind });
}

impl MainWindow {
    /// Follows the media once a frame: music mode (S8.1), then the profile it switches to
    /// (S8.8.3).
    pub(in crate::main_window) fn sync_media(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_music_mode(window, cx);
        self.sync_profile(cx);
    }

    /// Follows the media once a frame: media of a kind a profile is set for switches to it as
    /// it starts, and other media goes back to the profile the user had.
    fn sync_profile(&mut self, cx: &mut Context<Self>) {
        if self.is_customizing() {
            return;
        }
        let Some(playing) = self.playing_kind(cx) else { return };
        if self.auto_profile.decided.as_ref() == Some(&playing) {
            return;
        }
        let kind = playing.1;
        self.auto_profile.decided = Some(playing);
        let active = profiles::active(cx);
        let wanted = match profiles::for_media(cx, kind) {
            Some(name) => {
                self.auto_profile.before.get_or_insert(active);
                name
            }
            None => match self.auto_profile.before.take() {
                Some(before) if profiles::list(cx).iter().any(|p| p.name == before) => before,
                _ => return,
            },
        };
        self.switch_profile(&wanted, cx);
    }

    /// What plays and whether it is music or video; `None` while nothing plays.
    fn playing_kind(&self, cx: &App) -> Option<(Media, Automatic)> {
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        if !player.state().is_active() {
            return None;
        }
        let kind = if self.is_music_mode() { Automatic::Music } else { Automatic::Video };
        Some((player.media()?.clone(), kind))
    }
}
