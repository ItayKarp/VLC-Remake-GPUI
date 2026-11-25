# Long and large files (S9.8.1): hours of audio, a subtitle file past the app's reading limit,
# playlists at and past their item limit, and, with --huge, a video past 4 GiB (32-bit sizes
# and offsets). The 4 GiB file stays out of the packed corpus (release assets stop at 2 GiB):
# make it where it is needed.

# Ten hours of silence: seeking, the time display past 9:59:59 and the seek bar's resolution.
make_once long/ten-hours.opus -f lavfi -i "anullsrc=r=48000:cl=mono" -t 36000 \
  -c:a libopus -b:a 6k
entry long/ten-hours.opus media plays 'seconds=36000'

# A subtitle file over 32 MiB: the app leaves it to libVLC as it is.
mkdir -p "$out/long"
if [[ ! -s "$out/long/huge.srt" ]]; then
  awk 'BEGIN { for (i = 0; i < 400000; i++) {
    s = i % 60; m = int(i / 60) % 60; h = int(i / 3600)
    printf "%d\n%02d:%02d:%02d,000 --> %02d:%02d:%02d,900\nLine %d of a very long subtitle file, padded with words.\n\n", i + 1, h, m, s, h, m, s, i + 1
  } }' >"$out/long/huge.srt"
fi
entry long/huge.srt subtitle skipped

# Playlists of missing files at the item limit (100,000) and one past it, which stops there.
for count in 100000 100001; do
  if [[ ! -s "$out/long/entries-$count.m3u" ]]; then
    awk -v n="$count" 'BEGIN { for (i = 1; i <= n; i++) printf "missing-%06d.mp3\n", i }' \
      >"$out/long/entries-$count.m3u"
  fi
done
entry long/entries-100000.m3u playlist reads 'items=100000' 'missing=100000'
entry long/entries-100001.m3u playlist reads 'items=100000' 'missing=100000'

# Past 4 GiB: a 10 s source repeated as often as its size needs (the encoder doesn't hold
# the bitrate on a test pattern, so its size decides).
gib4=$((4 * 1024 * 1024 * 1024))
if [[ "$huge" == yes && "$(wc -c <"$out/huge/over-4gib.mkv" 2>/dev/null || echo 0)" -le $gib4 ]]; then
  rm -f "$out/huge/over-4gib.mkv"
  make_once huge/source-1080p.ts -f lavfi -i "testsrc2=size=1920x1080:rate=25:duration=10" \
    -c:v mpeg2video -b:v 80M -maxrate 80M -bufsize 20M
  # Matroska drops the transport stream's packet overhead, hence an eighth more.
  repeats=$((gib4 * 9 / 8 / $(wc -c <"$out/huge/source-1080p.ts") + 1))
  make_once huge/over-4gib.mkv -stream_loop "$repeats" -i "$out/huge/source-1080p.ts" -c copy
  rm -f "$out/huge/source-1080p.ts"
fi
if [[ -s "$out/huge/over-4gib.mkv" ]]; then
  seconds="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$out/huge/over-4gib.mkv")"
  entry huge/over-4gib.mkv media plays "seconds=$(printf '%.0f' "$seconds")"
fi
