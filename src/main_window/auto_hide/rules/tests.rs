use std::time::{Duration, Instant};

use super::*;

const DELAY: Duration = Duration::from_secs(3);

fn playing(auto_hide: AutoHide) -> Situation {
    Situation {
        fullscreen: false,
        hide_ui: false,
        video: true,
        idle: false,
        held: false,
        summoned: false,
        full_library: false,
        prefs: Prefs { auto_hide, show_in_fullscreen: true, decorations: true },
    }
}

fn fullscreen() -> Situation {
    Situation { fullscreen: true, ..playing(AutoHide::InFullscreenOnly) }
}

#[test]
fn a_window_keeps_its_bars_by_default() {
    let idle = Situation { idle: true, ..playing(AutoHide::InFullscreenOnly) };
    assert_eq!(idle.chrome(), Chrome { layout: Layout::Docked, title_bar: true, controls: true });
    assert!(!idle.idles());
}

#[test]
fn without_window_decorations_only_the_control_bar_shows() {
    let mut bare = playing(AutoHide::InFullscreenOnly);
    bare.prefs.decorations = false;
    assert_eq!(bare.chrome(), Chrome { layout: Layout::Docked, title_bar: false, controls: true });
    let library = Situation { full_library: true, ..bare };
    assert!(library.chrome().title_bar, "full library mode keeps its navigation");
}

#[test]
fn fullscreen_hides_the_title_bar_and_auto_hides_the_controls() {
    let awake = fullscreen();
    assert_eq!(
        awake.chrome(),
        Chrome { layout: Layout::Fullscreen, title_bar: false, controls: true }
    );
    let idle = Situation { idle: true, ..awake };
    assert!(!idle.chrome().controls);
    assert!(Situation { held: true, ..idle }.chrome().controls, "a hold keeps them");
}

#[test]
fn always_floats_the_bars_over_the_video_in_a_window() {
    let awake = playing(AutoHide::Always);
    assert_eq!(
        awake.chrome(),
        Chrome { layout: Layout::Floating, title_bar: true, controls: true }
    );
    let idle = Situation { idle: true, ..awake };
    assert_eq!(
        idle.chrome(),
        Chrome { layout: Layout::Floating, title_bar: false, controls: false }
    );
}

#[test]
fn never_keeps_the_controls_even_in_fullscreen() {
    let idle = Situation { idle: true, fullscreen: true, ..playing(AutoHide::Never) };
    assert!(idle.chrome().controls);
    assert!(!idle.idles());
}

#[test]
fn the_empty_state_never_hides_anything() {
    let idle = Situation { video: false, idle: true, ..fullscreen() };
    assert!(idle.chrome().controls);
    assert!(!idle.idles());
}

#[test]
fn hide_ui_shows_only_the_video() {
    let hidden = Situation { hide_ui: true, held: true, ..playing(AutoHide::Never) };
    assert_eq!(
        hidden.chrome(),
        Chrome { layout: Layout::Floating, title_bar: false, controls: false }
    );
    assert!(hidden.idles(), "the cursor still hides");
}

#[test]
fn controls_off_in_fullscreen_show_only_when_summoned() {
    let prefs = Prefs {
        auto_hide: AutoHide::InFullscreenOnly,
        show_in_fullscreen: false,
        decorations: true,
    };
    let off = Situation { prefs, held: true, ..fullscreen() };
    assert!(!off.chrome().controls);
    assert!(Situation { summoned: true, ..off }.chrome().controls);
}

#[test]
fn the_bars_cover_the_window_edges() {
    assert!(over_bars(1050., 1080., 54., None));
    assert!(!over_bars(20., 1080., 54., None));
    assert!(over_bars(20., 1080., 54., Some(40.)));
    assert!(!over_bars(500., 1080., 54., Some(40.)));
}

#[test]
fn jitter_does_not_count_as_moving() {
    let mut anchor = PointerAnchor::default();
    assert!(anchor.moved_to(100., 100.));
    assert!(!anchor.moved_to(102., 101.));
    assert!(anchor.moved_to(105., 100.));
    anchor.clear();
    assert!(anchor.moved_to(105., 100.));
}

#[test]
fn the_timer_hides_after_the_last_wake() {
    let start = Instant::now();
    let mut timer = IdleTimer::default();
    assert_eq!(timer.wake(start, DELAY), Some(DELAY));
    assert_eq!(timer.wake(start + Duration::from_secs(1), DELAY), None, "one task at a time");
    assert_eq!(timer.tick(start + DELAY, false), Tick::Sleep(Duration::from_secs(1)));
    assert_eq!(timer.tick(start + Duration::from_secs(4), false), Tick::Hide);
    assert!(timer.is_idle());
    assert_eq!(timer.wake(start + Duration::from_secs(5), DELAY), Some(DELAY));
    assert!(!timer.is_idle());
}

#[test]
fn a_hold_stops_the_timer_and_its_end_restarts_it() {
    let start = Instant::now();
    let mut timer = IdleTimer::default();
    timer.wake(start, DELAY);
    assert!(!timer.hold_ended(true));
    assert_eq!(timer.tick(start + DELAY, true), Tick::Stop);
    assert!(!timer.is_idle());
    assert!(timer.hold_ended(false));
    assert!(!timer.hold_ended(false));
}

#[test]
fn a_cancelled_timer_hides_nothing() {
    let start = Instant::now();
    let mut timer = IdleTimer::default();
    timer.wake(start, DELAY);
    timer.cancel();
    assert_eq!(timer.tick(start + DELAY, false), Tick::Stop);
    assert!(!timer.is_idle());
    assert_eq!(timer.wake(start + DELAY, DELAY), Some(DELAY));
}

#[test]
fn full_library_mode_keeps_the_title_bar_and_drops_the_controls() {
    let library = Situation { full_library: true, idle: true, ..playing(AutoHide::Always) };
    assert_eq!(
        library.chrome(),
        Chrome { layout: Layout::Docked, title_bar: true, controls: false }
    );
    assert!(!library.idles());
    let hidden = Situation { hide_ui: true, ..library };
    assert!(!hidden.idles());
    let fullscreen = Situation { fullscreen: true, ..library };
    assert_eq!(fullscreen.chrome().layout, Layout::Fullscreen);
}
