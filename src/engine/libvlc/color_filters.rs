//! The color filters on the worker thread (S7.4): libVLC's `extract`, `colorthres`, `invert`,
//! `posterize`, `gradient` and `sepia`, each in the chain while [`Colors`] turns it on. Each
//! takes its settings while it runs, so they change live as the enhancements do
//! (`cpu_filters.rs`).

use super::core_vars::Var;
use super::cpu_filters::{Group, OnFilter};
use super::worker::Worker;
use crate::engine::colors::{ColorThreshold, Colors, Gradient};

impl Worker {
    /// Shows video through the color filters `colors` turns on from now on.
    pub(super) fn set_colors(&mut self, colors: Colors) {
        self.set_cpu_filters(Group::Colors, color_filters(&colors));
    }
}

/// The filters `colors` turns on, with their settings.
fn color_filters(colors: &Colors) -> Vec<OnFilter> {
    let Colors { extract, threshold, negate, posterize, gradient, sepia } = *colors;
    let extract = extract.map(|color| OnFilter {
        name: "extract",
        settings: vec![(c"extract-component", integer(color))],
    });
    let invert = negate.then(|| OnFilter { name: "invert", settings: Vec::new() });
    let posterize = posterize.map(|levels| OnFilter {
        name: "posterize",
        settings: vec![(c"posterize-level", integer(levels))],
    });
    let sepia = sepia.map(|intensity| OnFilter {
        name: "sepia",
        settings: vec![(c"sepia-intensity", integer(intensity))],
    });
    let filters = [extract, threshold.map(color_threshold), invert, posterize];
    filters.into_iter().chain([gradient.map(gradient_filter), sepia]).flatten().collect()
}

fn color_threshold(threshold: ColorThreshold) -> OnFilter {
    OnFilter {
        name: "colorthres",
        settings: vec![
            (c"colorthres-color", integer(threshold.color)),
            (c"colorthres-saturationthres", integer(threshold.saturation)),
            (c"colorthres-similaritythres", integer(threshold.similarity)),
        ],
    }
}

fn gradient_filter(gradient: Gradient) -> OnFilter {
    OnFilter {
        name: "gradient",
        settings: vec![
            (c"gradient-mode", Var::String(gradient.mode.libvlc_name())),
            (c"gradient-type", Var::Integer(i64::from(gradient.colored))),
            (c"gradient-cartoon", Var::Bool(gradient.cartoon)),
        ],
    }
}

fn integer(value: u32) -> Var<'static> {
    Var::Integer(value.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::colors::GradientMode;

    #[test]
    fn only_the_filters_turned_on_join() {
        assert!(color_filters(&Colors::NONE).is_empty());
        let colors = Colors {
            negate: true,
            sepia: Some(120),
            gradient: Some(Gradient { mode: GradientMode::Edge, colored: true, cartoon: false }),
            ..Colors::NONE
        };
        let on: Vec<_> = color_filters(&colors).into_iter().map(|f| f.name).collect();
        assert_eq!(on, ["invert", "gradient", "sepia"]);
    }

    #[test]
    fn a_threshold_sets_its_color_and_both_limits() {
        let threshold = ColorThreshold { color: 0x00_FF00, saturation: 20, similarity: 15 };
        let filter = color_threshold(threshold);
        let names: Vec<_> = filter.settings.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [c"colorthres-color", c"colorthres-saturationthres", c"colorthres-similaritythres"]
        );
        assert!(matches!(filter.settings[0].1, Var::Integer(0x00_FF00)));
    }
}
