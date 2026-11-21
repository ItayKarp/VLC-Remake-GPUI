//! The bytes a second launch sends to the running instance (S0.6.1).
//!
//! A frame is a little-endian `u32` length followed by the body: `RVLC`, a version
//! byte, the start screen, the placement, the fullscreen flag, an item count, then
//! each item as a kind byte and a length-prefixed payload. Paths travel as the
//! platform's native encoding, so names that aren't valid Unicode survive. Both ends
//! are the same app on the same machine; a version mismatch is an error, and the
//! sender then reports the running instance as not responding.

use std::io::{self, Read, Write};
use std::path::PathBuf;

use crate::app::media::{self, Media};
use crate::app::request::{Launch, OpenRequest, Placement, StartScreen};

const MAGIC: &[u8; 4] = b"RVLC";
const VERSION: u8 = 1;
/// Far more than the OS allows on a command line; guards against garbage lengths.
const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

const KIND_FILE: u8 = 0;
const KIND_URL: u8 = 1;

/// Writes `launch` as one frame.
pub fn write_frame(writer: &mut impl Write, launch: &Launch) -> io::Result<()> {
    let body = encode(launch);
    let len = u32::try_from(body.len()).map_err(|_| invalid("request too large"))?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// Reads one frame written by [`write_frame`].
pub fn read_frame(reader: &mut impl Read) -> io::Result<Launch> {
    let len = read_u32(reader)? as usize;
    if len > MAX_FRAME_LEN {
        return Err(invalid("request too large"));
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    decode(&body)
}

/// Reads a little-endian `u32`.
pub fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn encode(launch: &Launch) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.push(VERSION);
    out.push(match launch.start {
        StartScreen::Auto => 0,
        StartScreen::Onboarding => 1,
        StartScreen::Gallery => 2,
    });
    out.push(match launch.open.placement {
        Placement::Replace => 0,
        Placement::Enqueue => 1,
        Placement::PlayNext => 2,
    });
    out.push(u8::from(launch.open.fullscreen));
    put_len(&mut out, launch.open.items.len());
    for item in &launch.open.items {
        let (kind, payload) = match item {
            Media::File(path) => (KIND_FILE, media::os_to_bytes(path.as_os_str())),
            Media::Url(url) => (KIND_URL, url.as_bytes().to_vec()),
        };
        out.push(kind);
        put_len(&mut out, payload.len());
        out.extend(payload);
    }
    out
}

fn decode(body: &[u8]) -> io::Result<Launch> {
    let mut cursor = Cursor(body);
    if cursor.take(MAGIC.len())? != MAGIC || cursor.byte()? != VERSION {
        return Err(invalid("not a request from this version of the app"));
    }
    let start = match cursor.byte()? {
        0 => StartScreen::Auto,
        1 => StartScreen::Onboarding,
        2 => StartScreen::Gallery,
        _ => return Err(invalid("unknown start screen")),
    };
    let placement = match cursor.byte()? {
        0 => Placement::Replace,
        1 => Placement::Enqueue,
        2 => Placement::PlayNext,
        _ => return Err(invalid("unknown placement")),
    };
    let fullscreen = cursor.byte()? != 0;
    let count = cursor.length()?;
    let items = (0..count).map(|_| cursor.item()).collect::<io::Result<_>>()?;
    Ok(Launch { start, open: OpenRequest { items, placement, fullscreen } })
}

fn put_len(out: &mut Vec<u8>, len: usize) {
    // Lengths are bounded by MAX_FRAME_LEN, which fits in a u32.
    out.extend((len as u32).to_le_bytes());
}

/// Reads fields from a frame body, failing on truncation instead of panicking.
struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(invalid("truncated request"));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn length(&mut self) -> io::Result<usize> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    }

    fn item(&mut self) -> io::Result<Media> {
        let kind = self.byte()?;
        let len = self.length()?;
        let payload = self.take(len)?;
        match kind {
            KIND_FILE => media::os_from_bytes(payload)
                .map(|path| Media::File(PathBuf::from(path)))
                .ok_or_else(|| invalid("malformed path")),
            KIND_URL => String::from_utf8(payload.to_vec())
                .map(Media::Url)
                .map_err(|_| invalid("malformed URL")),
            _ => Err(invalid("unknown item kind")),
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(launch: &Launch) -> Launch {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, launch).expect("writing to a Vec can't fail");
        read_frame(&mut bytes.as_slice()).expect("a frame we just wrote")
    }

    #[test]
    fn round_trips_every_field() {
        let launch = Launch {
            start: StartScreen::Onboarding,
            open: OpenRequest {
                items: vec![
                    Media::File(PathBuf::from("/vidéos/a \"b\" ✓.mkv")),
                    Media::Url("rtsp://cam/live".into()),
                ],
                placement: Placement::Enqueue,
                fullscreen: true,
            },
        };
        assert_eq!(round_trip(&launch), launch);
        assert_eq!(round_trip(&Launch::default()), Launch::default());
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &Launch::default()).expect("writing to a Vec can't fail");
        let truncated = &bytes[..bytes.len() - 1];
        assert!(read_frame(&mut &truncated[..]).is_err());
        let mut wrong_version = bytes.clone();
        wrong_version[4 + MAGIC.len()] = VERSION + 1;
        assert!(read_frame(&mut wrong_version.as_slice()).is_err());
        let huge = (u32::MAX).to_le_bytes();
        assert!(read_frame(&mut huge.as_slice()).is_err());
    }
}
