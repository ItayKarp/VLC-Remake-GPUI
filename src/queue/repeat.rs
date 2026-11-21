//! Repeat (S3.3.1): what happens at an item's natural end and at the queue's ends.

/// The repeat mode. The queue button cycles Off → All → One.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    /// The queue plays through once and stops at its end.
    #[default]
    Off,
    /// The queue starts over after its last item; Next and Previous wrap around.
    All,
    /// The current item plays again when it ends. Next and Previous still move.
    One,
}

impl Repeat {
    /// Every mode, in the order the button cycles and menus list them.
    pub const ALL: [Self; 3] = [Self::Off, Self::All, Self::One];

    /// The mode a click on the button moves to.
    pub fn cycled(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    /// The mode's name in menus.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::All => "All",
            Self::One => "One",
        }
    }

    /// Whether Next and Previous wrap around the queue's ends.
    pub(super) fn wraps(self) -> bool {
        self == Self::All
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_every_mode() {
        let mut mode = Repeat::default();
        for expected in [Repeat::All, Repeat::One, Repeat::Off] {
            mode = mode.cycled();
            assert_eq!(mode, expected);
        }
    }
}
