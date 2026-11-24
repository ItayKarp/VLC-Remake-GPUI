//! A [`Slider`]'s marks: ticks on the track between its ends and, unless they are bare ticks, a
//! label under each mark, the first and last flush with the track's ends (`controls-playback.png`).

use gpui::{Div, div, prelude::*, px, relative};

use super::{Marks, Slider};
use crate::theme::Palette;
use crate::theme::text_px;

/// A tick's width, and how far it stays from the track's top and bottom edges.
const TICK_WIDTH: f32 = 1.;
const TICK_INSET: f32 = 4.;
/// The labels' text size and the row's height.
const LABEL_TEXT: f32 = 12.;
const LABEL_HEIGHT: f32 = 24.;
/// The box a middle label is centered in.
const LABEL_WIDTH: f32 = 48.;

impl Slider {
    /// The ticks inside the track, at the marks between its ends.
    pub(super) fn ticks(&self, p: &Palette) -> Vec<Div> {
        let Some(marks) = self.shown_marks() else { return Vec::new() };
        let fractions = values(self.model.min, self.model.max, marks.every)
            .into_iter()
            .map(|value| self.model.fraction_at(value) as f32);
        fractions
            .filter(|fraction| *fraction > 0. && *fraction < 1.)
            .map(|fraction| {
                div()
                    .absolute()
                    .top(px(TICK_INSET))
                    .bottom(px(TICK_INSET))
                    .w(px(TICK_WIDTH))
                    .left(relative(fraction))
                    .bg(p.divider)
            })
            .collect()
    }

    /// The labels under the track, inset as the track is in the slider.
    pub(super) fn mark_labels(&self, inset: f32, p: &Palette) -> Option<Div> {
        let marks = self.shown_marks()?;
        let text = marks.label?;
        let labels = values(self.model.min, self.model.max, marks.every).into_iter().map(|value| {
            let fraction = self.model.fraction_at(value) as f32;
            let label = div().absolute().top_0().child(text(value));
            if fraction <= 0. {
                label.left_0()
            } else if fraction >= 1. {
                label.right_0()
            } else {
                label
                    .left(relative(fraction))
                    .w(px(LABEL_WIDTH))
                    .ml(px(-LABEL_WIDTH / 2.))
                    .flex()
                    .justify_center()
            }
        });
        let row = div()
            .relative()
            .h(px(LABEL_HEIGHT))
            .mx(px(inset))
            .text_size(text_px(LABEL_TEXT))
            .text_color(p.text_muted)
            .children(labels);
        Some(row)
    }

    fn shown_marks(&self) -> Option<Marks> {
        self.marks.filter(|marks| self.horizontal() && marks.every > 0.)
    }
}

/// The marks from `min` to `max`, every `every`, both ends included when they fall on one.
fn values(min: f64, max: f64, every: f64) -> Vec<f64> {
    // Room for rounding, so `max` counts when it's a multiple of `every` from `min`.
    let count = ((max - min) / every + 1e-9).floor() as u32;
    (0..=count).map(|i| min + f64::from(i) * every).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_include_both_ends() {
        let marks = values(0.25, 2., 0.25);
        assert_eq!(marks.len(), 8);
        assert_eq!(marks.first(), Some(&0.25));
        assert!((marks[7] - 2.).abs() < 1e-9);
    }

    #[test]
    fn an_uneven_range_stops_before_the_end() {
        assert_eq!(values(0., 10., 3.), [0., 3., 6., 9.]);
    }
}
