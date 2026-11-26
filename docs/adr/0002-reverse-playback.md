# ADR 0002 — Reverse playback

- **Status:** Accepted.
- **Story:** S8.13.3 (decides D18 in PRD Appendix B)
- **Date:** 2026-11-24

## Context

The Playback tab's design (`controls-playback.png`) has a **Reverse** toggle. The PRD asked for
a spike: libVLC has no native reverse playback, and doing it ourselves means decoding groups
of pictures (GOPs) backwards, which is expensive. Keep the toggle if reverse can be made
smooth; remove it otherwise.

## What the spike found

Against libVLC 3.0.23 on macOS (Apple silicon), with the S1.1 test media:

1. **libVLC ignores a negative rate.** `libvlc_media_player_set_rate(player, -1.0)` returns 0
   (success) and sets no error, but playback goes on forwards at 1× in every file tried: an
   H.264 MP4 (1080p60), an interlaced H.264 transport stream, an MPEG-2 program stream (DVD
   style) and FLAC audio. The position, read every 400 ms after the call, kept rising at
   about real time.
2. **Doing it in the app means a seek per picture.** libVLC 3 has no backward frame step; the
   app's own (`,`, S2.10.3) pauses and seeks back one frame's time, and every such seek
   decodes from the previous keyframe. Reverse at 1× would need 24 to 60 of those a second,
   each decoding up to a whole GOP (2 to 10 s of pictures in typical files), with no sound.
   That is a slideshow, not playback, and it would load the CPU or GPU fully for it.
3. **Doing it properly** means decoding each GOP forwards into a buffer of pictures and
   showing them backwards, beside libVLC rather than through it: our own demuxing and
   decoding, memory for a GOP of 4K pictures (about 12 MB each in NV12), and no audio. That
   is a second playback engine, far past this story's size.

## Decision

**Remove Reverse from the design.** The settings panel's Playback tab never shows it (it was
hidden, as every unbuilt row is, S2.7.2), and the PRD's S8.13.3 and D18 record why.

## Consequences

- Stepping back one frame (`,`) stays the only way back, as in VLC.
- Revisit when the app moves to libVLC 4 (ADR 0001): if its player can step or play
  backwards, Reverse can come back as a story of its own.
