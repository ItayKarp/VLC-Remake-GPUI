#!/usr/bin/env bash
# Runs the S1.1 measurement matrix and appends one RESULT line per run to
# target/spike-results/results.txt. Build first:
#   cargo build --release --example video_spike   (add --features runtime_shaders without Xcode's Metal toolchain)
# Usage: measure.sh [throughput|edges|all]   (default all)
# Set LIBVLC to an unpacked VLC folder to use it instead of the default (needed off macOS).
set -uo pipefail

bin=target/release/examples/video_spike
media=target/spike-media
out=target/spike-results
mkdir -p "$out"
paths=(cpu)
libvlc=()
if [[ -n "${LIBVLC:-}" ]]; then libvlc=(--libvlc "$LIBVLC"); fi
if [[ "$(uname)" == Darwin ]]; then paths+=(surface); fi

run() { # run SECONDS FILE EXTRA_ARGS...
  local seconds="$1" file="$2"
  shift 2
  for path in "${paths[@]}"; do
    local log="$out/$(basename "$file")-$path${1:+-sw}.log"
    "$bin" ${libvlc[@]+"${libvlc[@]}"} --seconds "$seconds" --path "$path" "$@" "$media/$file" >"$log" 2>&1
    grep -h "^RESULT" "$log" | sed "s/\$/ args=[$*]/" | tee -a "$out/results.txt"
  done
}

throughput() { # The acceptance criterion: 60 s each.
  run 60 1080p60-h264.mp4
  run 60 2160p30-h264.mp4
  run 60 2160p60-hevc10.mp4
}

edges() { # The S1.1.3 corpus: 10 s each.
  for file in 4320p30-hevc.mp4 1080p-hdr10.mkv 1080p-hlg.mkv dvd-anamorphic.mpg \
    odd-1919x1079.mkv 1080i-h264.ts 720p240-h264.mp4 audio-only.flac single-frame.png \
    phone-vfr-rot90.mp4; do
    run 10 "$file"
  done
  # Rotation only works with software decoding (libVLC 3 cannot rotate hardware frames).
  run 10 phone-vfr-rot90.mp4 --vlc-arg --codec=avcodec --vlc-arg --avcodec-hw=none
}

case "${1:-all}" in
  throughput) throughput ;;
  edges) edges ;;
  all) throughput && edges ;;
  *) echo "usage: $0 [throughput|edges|all]" >&2 && exit 2 ;;
esac
