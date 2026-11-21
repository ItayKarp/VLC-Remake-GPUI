#!/usr/bin/env bash
# Generates the S1.1.3 spike corpus into target/spike-media (or the folder given as $1).
# Needs ffmpeg with libx264, libx265 and, on macOS, VideoToolbox. Synthetic sources only, so
# nothing copyrighted is needed; a real-world corpus is S9.8.1.
set -euo pipefail

out="${1:-target/spike-media}"
mkdir -p "$out"
ff=(ffmpeg -hide_banner -loglevel error -y)

# A moving test card with a frame counter, so dropped and repeated frames are visible.
card() { echo "testsrc2=size=$1:rate=$2:duration=$3"; }
tone() { echo "sine=frequency=440:sample_rate=48000:duration=$1"; }

hevc_hw() { # HEVC through VideoToolbox when present (fast), libx265 otherwise.
  if ffmpeg -hide_banner -encoders 2>/dev/null | grep -q hevc_videotoolbox; then
    echo "-c:v hevc_videotoolbox -b:v $1"
  else
    echo "-c:v libx265 -preset ultrafast -b:v $1"
  fi
}

make() { # make NAME ARGS... : skip files that already exist, so reruns are cheap.
  local name="$1"
  shift
  if [[ -s "$out/$name" ]]; then return; fi
  echo "making $name"
  "${ff[@]}" "$@" "$out/$name"
}

# Throughput: the acceptance criterion and the measurement matrix.
make 1080p60-h264.mp4 -f lavfi -i "$(card 1920x1080 60 70)" -f lavfi -i "$(tone 70)" \
  -c:v libx264 -preset veryfast -b:v 12M -pix_fmt yuv420p -c:a aac -shortest
make 2160p30-h264.mp4 -f lavfi -i "$(card 3840x2160 30 70)" -f lavfi -i "$(tone 70)" \
  -c:v libx264 -preset veryfast -b:v 30M -pix_fmt yuv420p -c:a aac -shortest
# shellcheck disable=SC2046
make 2160p60-hevc10.mp4 -f lavfi -i "$(card 3840x2160 60 70)" -f lavfi -i "$(tone 70)" \
  $(hevc_hw 50M) -profile:v main10 -pix_fmt p010le -tag:v hvc1 -c:a aac -shortest
# shellcheck disable=SC2046
make 4320p30-hevc.mp4 -f lavfi -i "$(card 7680x4320 30 10)" $(hevc_hw 80M) -tag:v hvc1

# Colour: PQ (HDR10 with mastering metadata) and HLG, both BT.2020 10-bit.
make 1080p-hdr10.mkv -f lavfi -i "$(card 1920x1080 30 10)" -c:v libx265 -preset ultrafast \
  -pix_fmt yuv420p10le -color_primaries bt2020 -color_trc smpte2084 -colorspace bt2020nc \
  -x265-params "colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:hdr10=1:repeat-headers=1:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=1000,400"
make 1080p-hlg.mkv -f lavfi -i "$(card 1920x1080 30 10)" -c:v libx265 -preset ultrafast \
  -pix_fmt yuv420p10le -color_primaries bt2020 -color_trc arib-std-b67 -colorspace bt2020nc \
  -x265-params "colorprim=bt2020:transfer=arib-std-b67:colormatrix=bt2020nc"

# Geometry: rotation metadata with variable frame rate, anamorphic DVD, odd dimensions.
make phone-vfr.mp4 -f lavfi -i "$(card 1920x1080 30 10)" \
  -vf "setpts='N/30/TB+0.01*sin(N)/TB'" -fps_mode vfr -c:v libx264 -preset veryfast \
  -pix_fmt yuv420p
# Rotation is container metadata: remux with it so the pixels themselves stay landscape.
make phone-vfr-rot90.mp4 -display_rotation 90 -i "$out/phone-vfr.mp4" -c copy
make dvd-anamorphic.mpg -f lavfi -i "$(card 720x480 30000/1001 10)" -vf "setsar=32/27" \
  -c:v mpeg2video -b:v 6M -flags +ilme+ildct -top 1 -aspect 16:9
# FFV1 accepts odd sizes with 4:2:0 chroma, which H.264 and HEVC encoders refuse.
# testsrc2 rounds its size down to even numbers, so scale to the odd size afterwards.
make odd-1919x1079.mkv -f lavfi -i "$(card 1920x1080 30 10)" -vf "scale=1919:1079" \
  -c:v ffv1 -pix_fmt yuv420p

# Timing: interlaced 1080i and 240 fps.
make 1080i-h264.ts -f lavfi -i "$(card 1920x1080 30000/1001 10)" -c:v libx264 \
  -preset veryfast -flags +ildct+ilme -x264-params tff=1 -pix_fmt yuv420p
make 720p240-h264.mp4 -f lavfi -i "$(card 1280x720 240 10)" -c:v libx264 -preset veryfast \
  -pix_fmt yuv420p

# No video, and a single still frame.
make audio-only.flac -f lavfi -i "$(tone 30)"
make single-frame.png -f lavfi -i "$(card 1920x1080 1 1)" -frames:v 1

# Screenshot checks (screenshot.sh): still pictures whose correct pixels are known.
# SMPTE HD bars hold black, white, greys and 75% colours: a wrong range or matrix shows.
make bars-709.mp4 -f lavfi -i "smptehdbars=size=1920x1080:rate=30:duration=10" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -color_range tv -colorspace bt709 \
  -color_primaries bt709 -color_trc bt709
# SD bars (BT.601), so both matrices are checked.
make bars-601-sd.mp4 -f lavfi -i "smptebars=size=720x480:rate=30000/1001:duration=10" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -color_range tv -colorspace smpte170m \
  -color_primaries smpte170m -color_trc smpte170m
# A 1-pixel red frame on grey at an odd size: green or missing edge lines show.
make edges-1919x1079.mkv -f lavfi -i "color=c=gray:size=1919x1079:rate=30:duration=10" \
  -vf "drawbox=x=0:y=0:w=iw:h=ih:color=red:t=1" -c:v ffv1 -pix_fmt yuv420p
# Fast horizontal motion, truly interlaced (59.94p woven into 29.97i, so the two fields of a
# frame show different moments): combing on moving edges shows missing deinterlacing.
make 1080i-motion.ts -f lavfi -i "$(card 1920x1080 60000/1001 10)" \
  -vf "scroll=h=0.01,interlace=scan=tff" -c:v libx264 -preset veryfast -flags +ildct+ilme \
  -x264-params tff=1 -pix_fmt yuv420p

# Tracks (S1.6): English AC3 5.1 and Japanese AAC stereo (titled, the default), forced English
# and plain French subtitles.
# The subtitle source stays out of the corpus: libVLC would load it next to the video.
srt="$(mktemp -t tracks).srt"
# A cue every second, so a subtitle picked mid-playback shows within a second.
for second in 0 1 2 3 4 5 6 7 8 9; do
  printf '%d\n00:00:0%d,000 --> 00:00:0%d,900\nSign %d\n\n' $((second + 1)) "$second" "$second" "$second"
done >"$srt"
make tracks.mkv -f lavfi -i "$(card 640x360 30 10)" -f lavfi -i "$(tone 10)" \
  -f lavfi -i "$(tone 10)" -i "$srt" -i "$srt" \
  -map 0:v -map 1:a -map 2:a -map 3:s -map 4:s -c:v libx264 -preset veryfast -pix_fmt yuv420p \
  -c:a:0 ac3 -ac:a:0 6 -c:a:1 aac -ac:a:1 2 -c:s srt \
  -metadata:s:a:0 language=eng -metadata:s:a:1 language=jpn -metadata:s:a:1 title=Commentary \
  -metadata:s:s:0 language=eng -metadata:s:s:0 "title=English Forced" \
  -metadata:s:s:1 language=fre -disposition:a:0 0 -disposition:a:1 default \
  -disposition:s:0 forced -disposition:s:1 0
rm -f "$srt"

echo "corpus ready in $out"
