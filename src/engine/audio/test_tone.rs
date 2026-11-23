//! The Stereo Mode's test tone (S7.6.2): a tone in the left channel, a pause, then the same tone
//! in the right, as a WAV file a backend can play. It is played at a fixed, moderate level
//! whatever the volume is (boost may be at 200%), so the test never blasts.

use std::f32::consts::TAU;
use std::time::Duration;

/// How long the whole test lasts: a tone each side and the pause between them.
pub const LENGTH: Duration = Duration::from_millis(TONE_MS * 2 + GAP_MS);
/// When the right channel's tone starts.
pub const RIGHT_STARTS: Duration = Duration::from_millis(TONE_MS + GAP_MS);

/// Each side's tone, and the silence between them, in milliseconds.
const TONE_MS: u64 = 800;
const GAP_MS: u64 = 250;
/// The tone's pitch (A4), and its peak: a quarter of full scale (−12 dBFS).
const FREQUENCY: f32 = 440.;
const PEAK: f32 = 0.25;
/// Fades in and out over this long, so the tone starts and stops without a click.
const FADE_MS: u64 = 20;
/// CD-quality samples: 44.1 kHz, 16-bit, two channels.
const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const BITS: u16 = 16;
/// Bytes in the WAV header before the samples.
const HEADER_BYTES: u32 = 36;

/// A channel of the test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// The test as a WAV file: 16-bit stereo PCM, left tone, silence, right tone.
pub fn wav() -> Vec<u8> {
    let frames: Vec<[i16; 2]> =
        tone(Side::Left).chain(silence(GAP_MS)).chain(tone(Side::Right)).collect();
    let data_bytes = u32::try_from(frames.len() * 4).unwrap_or(u32::MAX);
    let mut file = header(data_bytes);
    for [left, right] in frames {
        file.extend_from_slice(&left.to_le_bytes());
        file.extend_from_slice(&right.to_le_bytes());
    }
    file
}

/// The RIFF header for `data_bytes` of samples.
fn header(data_bytes: u32) -> Vec<u8> {
    let block = CHANNELS * BITS / 8;
    let mut header = Vec::with_capacity(HEADER_BYTES as usize + 8);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(HEADER_BYTES + data_bytes).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16_u32.to_le_bytes());
    header.extend_from_slice(&1_u16.to_le_bytes());
    header.extend_from_slice(&CHANNELS.to_le_bytes());
    header.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    header.extend_from_slice(&(SAMPLE_RATE * u32::from(block)).to_le_bytes());
    header.extend_from_slice(&block.to_le_bytes());
    header.extend_from_slice(&BITS.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_bytes.to_le_bytes());
    header
}

/// The number of frames in `millis`.
fn frames(millis: u64) -> u64 {
    u64::from(SAMPLE_RATE) * millis / 1_000
}

/// The tone on `side`, faded in and out, the other side silent.
fn tone(side: Side) -> impl Iterator<Item = [i16; 2]> {
    let (count, fade) = (frames(TONE_MS), frames(FADE_MS));
    (0..count).map(move |frame| {
        let envelope = (frame.min(count - 1 - frame) as f32 / fade as f32).min(1.);
        let phase = TAU * FREQUENCY * frame as f32 / SAMPLE_RATE as f32;
        let sample = (phase.sin() * PEAK * envelope * f32::from(i16::MAX)) as i16;
        match side {
            Side::Left => [sample, 0],
            Side::Right => [0, sample],
        }
    })
}

/// `millis` of silence.
fn silence(millis: u64) -> impl Iterator<Item = [i16; 2]> {
    (0..frames(millis)).map(|_| [0, 0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(file: &[u8]) -> Vec<[i16; 2]> {
        let (frames, _) = file[44..].as_chunks::<4>();
        frames
            .iter()
            .map(|c| [i16::from_le_bytes([c[0], c[1]]), i16::from_le_bytes([c[2], c[3]])])
            .collect()
    }

    #[test]
    fn the_left_tone_comes_first_then_the_right() {
        let file = wav();
        assert_eq!(&file[..4], b"RIFF");
        let frames = samples(&file);
        assert_eq!(frames.len() as u64, super::frames(LENGTH.as_millis() as u64));
        let split = super::frames(RIGHT_STARTS.as_millis() as u64) as usize;
        assert!(frames[..split].iter().all(|[_, right]| *right == 0), "left first, alone");
        assert!(frames[split..].iter().all(|[left, _]| *left == 0), "then right, alone");
        assert!(frames[..split].iter().any(|[left, _]| *left != 0));
        assert!(frames[split..].iter().any(|[_, right]| *right != 0));
    }

    #[test]
    fn the_tone_stays_at_its_fixed_level_and_starts_silent() {
        let frames = samples(&wav());
        let loudest = frames.iter().flatten().map(|sample| sample.unsigned_abs()).max();
        let peak = (PEAK * f32::from(i16::MAX)) as u16;
        assert!(loudest.is_some_and(|loudest| loudest <= peak && loudest > peak - peak / 10));
        assert_eq!(frames[0], [0, 0], "faded in");
    }
}
