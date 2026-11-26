# Reference clips and broken files (S9.8.1): what a bad download, a full disk or a wrong
# extension leaves behind. The app must refuse or play each without hanging or crashing.

clip=(-f lavfi -i "testsrc2=size=640x360:rate=30:duration=6" -f lavfi
  -i "sine=frequency=440:sample_rate=48000:duration=6" -c:v libx264 -preset veryfast
  -pix_fmt yuv420p -c:a aac -shortest)

# Two clips everything else is cut from: MP4's index (moov) at the end, as a camera writes it,
# and at the start, as a streaming site does.
make_once reference/clip-h264.mp4 "${clip[@]}"
make_once reference/clip-faststart.mp4 "${clip[@]}" -movflags +faststart
make_once reference/clip.mkv "${clip[@]}"
entry reference/clip-h264.mp4 media plays 'seconds=6'
entry reference/clip-faststart.mp4 media plays 'seconds=6'
entry reference/clip.mkv media plays 'seconds=6'

mkdir -p "$out/broken"
half() { echo $(($(wc -c <"$1") / 2)); }

: >"$out/broken/empty.mp4"
entry broken/empty.mp4 media fails
random_bytes "$out/broken/random-bytes.mkv" 65536
entry broken/random-bytes.mkv media fails
printf 'This is a text file with a video extension.\n' >"$out/broken/text-named.mp4"
entry broken/text-named.mp4 media fails
head -c 4096 "$out/reference/clip.mkv" >"$out/broken/header-only.mkv"
entry broken/header-only.mkv media fails

# Half a file: without its index an MP4 can't be read; with it in front, the first half plays.
head -c "$(half "$out/reference/clip-h264.mp4")" "$out/reference/clip-h264.mp4" \
  >"$out/broken/truncated-index-at-end.mp4"
entry broken/truncated-index-at-end.mp4 media fails
head -c "$(half "$out/reference/clip-faststart.mp4")" "$out/reference/clip-faststart.mp4" \
  >"$out/broken/truncated-index-first.mp4"
entry broken/truncated-index-first.mp4 media plays

# 32 KiB of noise in the middle of the video data: the picture breaks up, playback goes on.
cp "$out/reference/clip-faststart.mp4" "$out/broken/corrupted-middle.mp4"
dd if=/dev/urandom of="$out/broken/corrupted-middle.mp4" bs=1024 count=32 conv=notrunc \
  seek=$(($(half "$out/reference/clip-faststart.mp4") / 1024)) 2>/dev/null
entry broken/corrupted-middle.mp4 media plays 'seconds=6'

# A Matroska file named .mp4: libVLC goes by the content.
cp "$out/reference/clip.mkv" "$out/broken/wrong-extension.mp4"
entry broken/wrong-extension.mp4 media plays 'seconds=6'

# Subtitles that aren't: binary data and an empty file are left alone; timestamps that make
# no sense still leave text to read.
random_bytes "$out/broken/binary.srt" 4096
entry broken/binary.srt subtitle skipped
: >"$out/broken/empty.srt"
entry broken/empty.srt subtitle skipped
printf '1\n99:99:99,999 --> 00:00:00,000\nBackwards in time\n\nnot a number\n--> -->\n\n' \
  >"$out/broken/bad-timestamps.srt"
entry broken/bad-timestamps.srt subtitle reads 'text="Backwards in time"'

# Playlists: every entry missing, one listing itself, XML that never closes (nothing is
# taken from it), and Windows line ends after a byte order mark.
printf '#EXTM3U\ngone-1.mp4\ngone-2.mkv\n/nowhere/gone-3.mp3\n' >"$out/broken/missing-entries.m3u"
entry broken/missing-entries.m3u playlist reads 'items=3' 'missing=3'
printf '#EXTM3U\nself-reference.m3u\n../reference/clip-h264.mp4\n' \
  >"$out/broken/self-reference.m3u"
entry broken/self-reference.m3u playlist reads 'items=1' 'missing=0'
printf '<?xml version="1.0"?>\n<playlist><trackList><track><location>file:///a.mp4' \
  >"$out/broken/unclosed.xspf"
entry broken/unclosed.xspf playlist reads 'items=0' 'missing=0'
printf '\xef\xbb\xbf#EXTM3U\r\n#EXTINF:6,Clip\r\n../reference/clip-h264.mp4\r\n' \
  >"$out/broken/bom-crlf.m3u8"
entry broken/bom-crlf.m3u8 playlist reads 'items=1' 'missing=0'
