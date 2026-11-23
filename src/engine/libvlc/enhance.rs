//! The picture's enhancements on the worker thread (S7.3): libVLC's `gradfun` (banding removal),
//! `sharpen` and `grain` filters, each on while its value isn't 0. They are set, live, as the
//! other filters on decoded pictures are (`cpu_filters.rs`).

use super::core_vars::Var;
use super::cpu_filters::{Group, OnFilter};
use super::worker::Worker;
use crate::engine::media_options::Picture;

impl Worker {
    /// Passes video through the enhancements `picture` turns on from now on; `None` turns them
    /// all off.
    pub(super) fn set_enhancements(&mut self, picture: Option<&Picture>) {
        self.set_cpu_filters(Group::Enhancements, enhancements(picture));
    }
}

/// The enhancement filters `picture` turns on, with their values.
fn enhancements(picture: Option<&Picture>) -> Vec<OnFilter> {
    let picture = picture.copied().unwrap_or(Picture::NEUTRAL);
    let gradfun = (picture.deband_radius > 0).then(|| OnFilter {
        name: "gradfun",
        settings: vec![(c"gradfun-radius", Var::Integer(picture.deband_radius.into()))],
    });
    let float = |name, variable, value: f32| {
        (value > 0.).then(|| OnFilter { name, settings: vec![(variable, Var::Float(value))] })
    };
    let sharpen = float("sharpen", c"sharpen-sigma", picture.sharpen);
    let grain = float("grain", c"grain-variance", picture.grain);
    [gradfun, sharpen, grain].into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_is_on_while_its_value_isnt_zero() {
        let picture = Picture { sharpen: 0.5, deband_radius: 16, ..Picture::NEUTRAL };
        let on: Vec<_> = enhancements(Some(&picture)).into_iter().map(|f| f.name).collect();
        assert_eq!(on, ["gradfun", "sharpen"]);
        assert!(enhancements(None).is_empty());
    }
}
