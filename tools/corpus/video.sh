# The S1.1.3 corpus (examples/video_spike/make_test_media.sh) under video/: resolutions,
# codecs, HDR, rotation, interlacing and frame rates, each with its length in seconds.

"$root/examples/video_spike/make_test_media.sh" "$out/video"

for clip in 1080p60-h264.mp4:70 2160p30-h264.mp4:70 2160p60-hevc10.mp4:70 \
  4320p30-hevc.mp4:10 1080p-hdr10.mkv:10 1080p-hlg.mkv:10 phone-vfr.mp4:10 \
  phone-vfr-rot90.mp4:10 dvd-anamorphic.mpg:10 odd-1919x1079.mkv:10 1080i-h264.ts:10 \
  720p240-h264.mp4:10 audio-only.flac:30 bars-709.mp4:10 bars-601-sd.mp4:10 \
  edges-1919x1079.mkv:10 1080i-motion.ts:10 tracks.mkv:10 two-programs.ts:10; do
  entry "video/${clip%%:*}" media plays "seconds=${clip##*:}"
done
entry video/single-frame.png media plays
