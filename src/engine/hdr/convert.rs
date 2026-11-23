//! Tone-maps a whole picture: libVLC's planar 10-bit 4:2:0 (`I0AL`) into the sink's NV12 or
//! BGRA, a band of rows per CPU thread, as a 4K picture is too much for one thread at 60 fps.
//!
//! The NV12 written is full-range BT.601, which is how GPUI's surface shader reads buffers
//! without a color tag (ours have none).

use std::thread;

use super::tone_map::Mapper;

/// Pictures smaller than this many pixels are mapped on the calling thread alone: threads
/// would cost more than they save.
const THREADED_PIXELS: usize = 640 * 360;
/// The most threads a picture is split over.
const MAX_THREADS: usize = 8;
/// BT.601's luminance weights, and its chroma scales.
const LUMA_601: [f32; 3] = [0.299, 0.587, 0.114];
const CB_SCALE: f32 = 1. / 1.772;
const CR_SCALE: f32 = 1. / 1.402;
/// The most an 8-bit sample holds.
const FULL: f32 = 255.;

/// A planar 10-bit 4:2:0 picture: its planes and their pitches, in samples.
pub struct Source<'a> {
    pub luma: &'a [u16],
    pub cb: &'a [u16],
    pub cr: &'a [u16],
    pub luma_pitch: usize,
    pub chroma_pitch: usize,
    pub width: usize,
    pub height: usize,
}

/// Where the mapped picture goes, with pitches in bytes.
pub enum Target<'a> {
    /// NV12: a luma plane, then interleaved Cb and Cr at half size.
    Nv12 { luma: &'a mut [u8], luma_pitch: usize, chroma: &'a mut [u8], chroma_pitch: usize },
    /// BGRA, four bytes a pixel.
    Bgra { pixels: &'a mut [u8], pitch: usize },
}

/// Two rows of the target: what one row of chroma covers.
enum RowPair<'a> {
    Nv12 { luma: &'a mut [u8], luma_pitch: usize, chroma: &'a mut [u8] },
    Bgra { pixels: &'a mut [u8], pitch: usize },
}

/// Maps `source` into `target` with `mapper`.
pub fn convert(mapper: &Mapper, source: &Source, target: Target) {
    let pairs = row_pairs(target, source.height.div_ceil(2));
    let threads = if source.width * source.height < THREADED_PIXELS {
        1
    } else {
        thread::available_parallelism().map_or(1, usize::from).min(MAX_THREADS)
    };
    let mut pairs: Vec<_> = pairs.enumerate().collect();
    let band = pairs.len().div_ceil(threads).max(1);
    thread::scope(|scope| {
        for band in pairs.chunks_mut(band) {
            scope.spawn(|| map_band(mapper, source, band));
        }
    });
}

/// Maps a band of row pairs, each with its chroma row.
fn map_band(mapper: &Mapper, source: &Source, band: &mut [(usize, RowPair)]) {
    for (row, pair) in band {
        map_pair(mapper, source, *row, pair);
    }
}

/// The target cut into `count` pairs of rows.
fn row_pairs(target: Target<'_>, count: usize) -> Box<dyn Iterator<Item = RowPair<'_>> + '_> {
    match target {
        Target::Nv12 { luma, luma_pitch, chroma, chroma_pitch } => Box::new(
            luma.chunks_mut(2 * luma_pitch)
                .zip(chroma.chunks_mut(chroma_pitch))
                .take(count)
                .map(move |(luma, chroma)| RowPair::Nv12 { luma, luma_pitch, chroma }),
        ),
        Target::Bgra { pixels, pitch } => Box::new(
            pixels
                .chunks_mut(2 * pitch)
                .take(count)
                .map(move |pixels| RowPair::Bgra { pixels, pitch }),
        ),
    }
}

/// Maps the two rows chroma row `row` covers.
fn map_pair(mapper: &Mapper, source: &Source, row: usize, pair: &mut RowPair) {
    let rows = if 2 * row + 1 < source.height { 2 } else { 1 };
    let chroma_row = row * source.chroma_pitch;
    for column in 0..source.width.div_ceil(2) {
        let chroma = Mapper::chroma(source.cb[chroma_row + column], source.cr[chroma_row + column]);
        let mut sum = [0.; 3];
        let mut count = 0.;
        for dy in 0..rows {
            for x in (2 * column)..(2 * column + 2).min(source.width) {
                let y = source.luma[(2 * row + dy) * source.luma_pitch + x];
                let rgb = mapper.pixel(y, chroma);
                write_pixel(pair, dy, x, rgb);
                sum = [sum[0] + rgb[0], sum[1] + rgb[1], sum[2] + rgb[2]];
                count += 1.;
            }
        }
        if let RowPair::Nv12 { chroma: plane, .. } = pair {
            let [cb, cr] = chroma_601(sum.map(|channel| channel / count));
            plane[2 * column] = cb;
            plane[2 * column + 1] = cr;
        }
    }
}

/// Writes one pixel of the pair: its luma for NV12, all of it for BGRA.
#[inline]
fn write_pixel(pair: &mut RowPair, dy: usize, x: usize, rgb: [f32; 3]) {
    match pair {
        RowPair::Nv12 { luma, luma_pitch, .. } => {
            luma[dy * *luma_pitch + x] = byte(dot(LUMA_601, rgb));
        }
        RowPair::Bgra { pixels, pitch } => {
            let at = dy * *pitch + 4 * x;
            pixels[at..at + 4].copy_from_slice(&[
                byte(rgb[2]),
                byte(rgb[1]),
                byte(rgb[0]),
                u8::MAX,
            ]);
        }
    }
}

/// Full-range BT.601 Cb and Cr of a gamma-encoded color.
fn chroma_601(rgb: [f32; 3]) -> [u8; 2] {
    let luma = dot(LUMA_601, rgb);
    [byte((rgb[2] - luma) * CB_SCALE + 0.5), byte((rgb[0] - luma) * CR_SCALE + 0.5)]
}

#[inline]
fn dot(row: [f32; 3], rgb: [f32; 3]) -> f32 {
    row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2]
}

/// A value from 0 to 1 as an 8-bit sample.
#[inline]
fn byte(value: f32) -> u8 {
    (value.clamp(0., 1.) * FULL + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::hdr::{ToneMap, Transfer};

    /// A grey 10-bit picture of `width`×`height` with luma `y`.
    fn grey(width: usize, height: usize, y: u16) -> (Vec<u16>, Vec<u16>) {
        (vec![y; width * height], vec![512; width.div_ceil(2) * height.div_ceil(2)])
    }

    fn mapper() -> Mapper {
        Mapper::new(ToneMap { transfer: Transfer::Pq, peak: 1000. })
    }

    #[test]
    fn grey_maps_to_neutral_nv12_on_odd_sizes() {
        let (width, height) = (5, 3);
        let (luma, chroma) = grey(width, height, 64);
        let source = Source {
            luma: &luma,
            cb: &chroma,
            cr: &chroma,
            luma_pitch: width,
            chroma_pitch: 3,
            width,
            height,
        };
        let (mut out_luma, mut out_chroma) = (vec![7; 6 * 3], vec![7; 6 * 2]);
        let target = Target::Nv12 {
            luma: &mut out_luma,
            luma_pitch: 6,
            chroma: &mut out_chroma,
            chroma_pitch: 6,
        };
        convert(&mapper(), &source, target);
        assert!(out_luma.chunks(6).all(|row| row[..5] == [0; 5] && row[5] == 7), "{out_luma:?}");
        assert!(out_chroma.chunks(6).all(|row| row == [128; 6]), "{out_chroma:?}");
    }

    #[test]
    fn bgra_is_opaque_and_large_pictures_split_over_threads() {
        let (width, height) = (1280, 720);
        let (luma, chroma) = grey(width, height, 940);
        let source = Source {
            luma: &luma,
            cb: &chroma,
            cr: &chroma,
            luma_pitch: width,
            chroma_pitch: width / 2,
            width,
            height,
        };
        let mut pixels = vec![0; width * height * 4];
        convert(&mapper(), &source, Target::Bgra { pixels: &mut pixels, pitch: width * 4 });
        assert!(pixels.chunks(4).all(|pixel| pixel == [255; 4]));
    }
}
