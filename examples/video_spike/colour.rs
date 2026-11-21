//! Which YCbCr matrix a picture was encoded with. libVLC 3 does not report a track's matrix,
//! so, like other players, we guess it from the picture size: HD and larger is BT.709, and
//! standard definition is BT.601.

/// The largest standard-definition picture.
const LARGEST_SD: (u32, u32) = (1024, 576);

/// Whether a `width`×`height` picture counts as HD, and so BT.709 rather than BT.601.
pub fn is_hd(width: u32, height: u32) -> bool {
    width > LARGEST_SD.0 || height > LARGEST_SD.1
}

#[cfg(test)]
mod tests {
    use super::is_hd;

    #[test]
    fn hd_starts_above_standard_definition() {
        assert!(!is_hd(720, 480));
        assert!(!is_hd(1024, 576));
        assert!(is_hd(1280, 720));
        assert!(is_hd(1080, 1920));
    }
}
