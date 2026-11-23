//! NV12 to RGBA with the matrix GPUI's surface shader uses (full-range BT.601), so a snapshot
//! has the colours the screen showed.

use image::RgbaImage;

/// One NV12 picture: a luma plane and an interleaved Cb/Cr plane at half the size each way,
/// each with its own row stride in bytes.
pub struct Planes<'a> {
    /// The luma plane.
    pub luma: &'a [u8],
    /// Bytes per luma row.
    pub luma_stride: usize,
    /// The chroma plane.
    pub chroma: &'a [u8],
    /// Bytes per chroma row.
    pub chroma_stride: usize,
}

/// `planes`, `width` by `height`, in RGBA.
pub fn to_rgba(planes: &Planes<'_>, width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let (x, y) = (x as usize, y as usize);
        let luma = planes.luma[y * planes.luma_stride + x];
        let chroma = (y / 2) * planes.chroma_stride + (x / 2) * 2;
        let (cb, cr) = (planes.chroma[chroma], planes.chroma[chroma + 1]);
        image::Rgba(rgb(luma, cb, cr))
    })
}

/// One full-range BT.601 pixel in RGBA.
fn rgb(luma: u8, cb: u8, cr: u8) -> [u8; 4] {
    let luma = f32::from(luma) / 255.;
    let (cb, cr) = (f32::from(cb) / 255. - 0.5, f32::from(cr) / 255. - 0.5);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    [
        channel(luma + 1.402 * cr),
        channel(luma - 0.3441 * cb - 0.7141 * cr),
        channel(luma + 1.772 * cb),
        u8::MAX,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Within a step: neutral chroma, 128, reads as 0.002 above 0.5, as in GPUI's shader.
    #[test]
    fn grey_stays_grey() {
        let close = |got: [u8; 4], want: u8| got[..3].iter().all(|&c| c.abs_diff(want) <= 1);
        assert!(close(rgb(128, 128, 128), 128));
        assert!(close(rgb(0, 128, 128), 0));
        assert!(close(rgb(255, 128, 128), 255));
    }

    #[test]
    fn red_comes_back_red() {
        // Full-range BT.601 red: Y 76, Cb 85, Cr 255.
        let [red, green, blue, _] = rgb(76, 85, 255);
        assert!(red > 250 && green < 5 && blue < 5, "{red} {green} {blue}");
    }

    #[test]
    fn chroma_is_shared_by_two_by_two_pixels_with_padded_rows() {
        let luma = [128_u8, 128, 0, 0, 128, 128, 0, 0];
        let chroma = [85_u8, 255, 0, 0];
        let planes = Planes { luma: &luma, luma_stride: 4, chroma: &chroma, chroma_stride: 4 };
        let image = to_rgba(&planes, 2, 2);
        assert_eq!(image.get_pixel(0, 0), image.get_pixel(1, 1));
        assert!(image.get_pixel(1, 0)[0] > image.get_pixel(1, 0)[2], "reddish");
    }
}
