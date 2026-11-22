//! Player tests for subtitle synchronization (S6.3): a sync belongs to one subtitle track, the
//! backend gets the showing track's delay, a new media starts without one, and remembered syncs
//! come back.

use gpui::TestAppContext;

use super::Rig;
use super::tracks::{movie, open_with};
use crate::app::media::Media;
use crate::engine::backend::Command;
use crate::engine::mock::MockMedia;
use crate::engine::subtitle_sync::{SubtitleKey, SubtitleSync};
use crate::engine::tracks::{TrackId, TrackKind};
use crate::engine::values::Offset;

fn delayed(millis: i64) -> SubtitleSync {
    SubtitleSync::default().with_delay(Offset::from_millis(millis))
}

fn show(rig: &Rig, cx: &mut TestAppContext, track: i32) {
    rig.run(cx, |player, cx| {
        player.select_track(TrackKind::Subtitle, Some(TrackId(track)), cx);
    });
}

fn sync(rig: &Rig, cx: &mut TestAppContext) -> SubtitleSync {
    rig.player.read_with(cx, |player, _| player.subtitle_sync())
}

fn delays(rig: &Rig) -> Vec<Offset> {
    let commands = rig.mock.take_commands();
    commands
        .into_iter()
        .filter_map(|command| match command {
            Command::SetSubtitleDelay(delay) => Some(delay),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn a_delay_belongs_to_the_track_it_was_set_for(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    open_with(&rig, cx, movie());
    rig.run(cx, |player, cx| player.select_track(TrackKind::Subtitle, None, cx));
    let refused = rig.player.update(cx, |player, cx| player.set_subtitle_sync(delayed(500), cx));
    assert_eq!(refused, None, "no subtitle shows");

    show(&rig, cx, 3);
    rig.mock.take_commands();
    let key = rig.player.update(cx, |player, cx| player.set_subtitle_sync(delayed(-4_000), cx));
    assert_eq!(key, Some(SubtitleKey::Stream(3)));
    assert_eq!(delays(&rig), [Offset::from_millis(-4_000)]);

    show(&rig, cx, 5);
    assert_eq!(sync(&rig, cx), SubtitleSync::default());
    assert_eq!(delays(&rig), [Offset::default()]);

    show(&rig, cx, 3);
    assert_eq!(sync(&rig, cx), delayed(-4_000));
    assert_eq!(delays(&rig), [Offset::from_millis(-4_000)]);
}

#[gpui::test]
fn new_media_starts_without_the_last_ones_delay(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    open_with(&rig, cx, movie());
    show(&rig, cx, 3);
    rig.player.update(cx, |player, cx| player.set_subtitle_sync(delayed(250), cx));
    rig.mock.take_commands();

    rig.open(cx);
    assert_eq!(delays(&rig), [Offset::default()], "the backend keeps a delay between media");
}

#[gpui::test]
fn remembered_syncs_come_back_without_undoing_a_newer_one(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    open_with(&rig, cx, movie());
    show(&rig, cx, 5);
    rig.player.update(cx, |player, cx| player.set_subtitle_sync(delayed(100), cx));
    let media = Media::Url("mock://clip".into());
    let saved =
        vec![(SubtitleKey::Stream(3), delayed(-700)), (SubtitleKey::Stream(5), delayed(900))];
    rig.run(cx, |player, cx| player.restore_subtitle_syncs(&media, saved, cx));
    assert_eq!(sync(&rig, cx), delayed(100));
    show(&rig, cx, 3);
    assert_eq!(sync(&rig, cx), delayed(-700));

    let other = Media::Url("mock://other".into());
    let late = vec![(SubtitleKey::Stream(5), delayed(1))];
    rig.run(cx, |player, cx| player.restore_subtitle_syncs(&other, late, cx));
    show(&rig, cx, 5);
    assert_eq!(sync(&rig, cx), delayed(100), "another media's syncs are ignored");
}
