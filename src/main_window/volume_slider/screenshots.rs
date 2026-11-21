//! Screenshot tests for the wedge against `volume/volume-*.png`: its shape, the gradient and
//! where the fill ends on the non-linear scale.

use gpui::{
    AppContext as _, Context, IntoElement, Render, Window, black, canvas, div, prelude::*, rgb,
};

use super::scale::Scale;
use super::wedge::{self, Look, WEDGE_WIDTH};
use crate::engine::values::Volume;
use crate::screenshot_test::{self, Shot};

/// The designs' height: the wedge with half a pixel to spare.
const DESIGN_HEIGHT: f32 = 28.;
/// The dark theme's well, which the designs draw the track in.
const TRACK: u32 = 0x090909;
/// The share of pixels allowed to differ: the anti-aliased edges. Measured below 3% on macOS
/// (November 2026).
const MAX_MISMATCH: f64 = 0.04;

/// The wedge alone at one volume, on the designs' transparent (black) background.
struct WedgeShot {
    volume: Volume,
}

impl Render for WedgeShot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let look = Look {
            track: rgb(TRACK).into(),
            segments: Scale::new(Volume::from_percent(200)).segments(self.volume),
            opacity: 1.,
        };
        let painted = canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                wedge::paint(bounds, &look, window);
            },
        );
        div().size_full().bg(black()).child(painted.size_full())
    }
}

#[test]
fn the_wedge_matches_the_designs() {
    let failures: Vec<String> = [(0, "volume-0"), (50, "volume-50pct"), (100, "volume-100pct")]
        .into_iter()
        .chain([(150, "volume-150pct"), (200, "volume-200pct")])
        .filter_map(|(percent, file)| check(percent, file))
        .collect();
    assert!(failures.is_empty(), "the wedge differs from the design:\n{}", failures.join("\n"));
}

fn check(percent: u32, file: &'static str) -> Option<String> {
    let shot = Shot {
        name: file,
        design: format!("docs/ui/player/controls/volume/{file}.png"),
        design_origin: (0, 0),
        window: (WEDGE_WIDTH, DESIGN_HEIGHT),
        skip_top: 0.,
        max_mismatch: MAX_MISMATCH,
    };
    let volume = Volume::from_percent(percent);
    screenshot_test::check(&shot, |_, cx| cx.new(|_| WedgeShot { volume }))
}
