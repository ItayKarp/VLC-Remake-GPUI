#!/usr/bin/env bash
# macOS: plays FILE in the spike on PATH (cpu or surface) and captures the spike's window
# after DELAY seconds, converted to sRGB so pixel values compare across machines.
# Usage: screenshot.sh FILE PATH OUT.png [DELAY] [extra spike args...]
# The terminal running this needs Screen Recording permission.
set -euo pipefail

file="$1" path="$2" out="$3" delay="${4:-3}"
shift $(($# < 4 ? $# : 4))
bin=target/release/examples/video_spike

"$bin" --seconds $((delay + 3)) --path "$path" "$@" "$file" >"${out%.png}.log" 2>&1 &
spike=$!
sleep "$delay"

# The window number of the on-screen window the spike process owns.
window=$(swift - "$spike" <<'SWIFT'
import CoreGraphics
let pid = Int32(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as! [[String: Any]]
for window in windows where (window[kCGWindowOwnerPID as String] as? Int32) == pid {
    if let number = window[kCGWindowNumber as String] as? Int { print(number); break }
}
SWIFT
)
if [[ -z "$window" ]]; then
  echo "no window found for the spike (pid $spike)" >&2
  wait "$spike" || true
  exit 1
fi
screencapture -x -o -l "$window" "$out"
sips -s format png --matchTo "/System/Library/ColorSync/Profiles/sRGB Profile.icc" "$out" \
  --out "$out" >/dev/null
wait "$spike" || true
echo "$out"
