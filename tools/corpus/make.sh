#!/usr/bin/env bash
# Makes the S9.8.1 test media corpus: the S1.1.3 videos, subtitles in fourteen encodings,
# broken files, odd names and long files, with manifest.toml (what each file is and what the
# app should do with it) and SHA256SUMS. Synthetic sources only, so nothing in it is
# copyrighted. The corpus lives outside this repository: pack.sh archives it for the artifact
# store and fetch.sh brings the pinned archive back (docs/qa.md).
#
#   tools/corpus/make.sh [--huge] [folder]     (folder: target/corpus by default)
#
# --huge adds a video past 4 GiB, which takes under a minute and 4.7 GB of disk. Needs ffmpeg
# (libx264, libx265, libopus), iconv and shasum. Files already made are kept, so reruns are
# quick; the manifest and checksums are written again each time.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
huge=no
if [[ "${1:-}" == --huge ]]; then
  huge=yes
  shift
fi
out="${1:-$root/target/corpus}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
manifest="$out/manifest.toml"

here="$(dirname "$0")"
# shellcheck source=tools/corpus/lib.sh
source "$here/lib.sh"

cat >"$manifest" <<HEADER
# The S9.8.1 test media corpus, written by tools/corpus/make.sh. Each [[file]] is a path in
# this folder, its kind (media, subtitle, playlist) and what the app should do with it
# (plays, fails, reads, skipped), with what a test checks: a media file's length in seconds,
# a subtitle's encoding and first line, a playlist's item and missing counts, a video's
# subtitle files.
version = 1
made = "$(date -u +%Y-%m-%d)"
ffmpeg = $(toml_string "$(ffmpeg -version | head -n 1)")
HEADER

for section in video broken subtitles names large; do
  # shellcheck source=/dev/null
  source "$here/$section.sh"
done

# Samples that can't be generated (a Dolby Vision profile 5 clip, a camera's file) are added
# by hand under real/, with their manifest entries in real/manifest.toml.
if [[ -f "$out/real/manifest.toml" ]]; then
  cat "$out/real/manifest.toml" >>"$manifest"
fi

echo "writing SHA256SUMS"
(cd "$out" && find . -type f ! -name SHA256SUMS ! -name .DS_Store ! -path ./real/manifest.toml \
  ! -path './huge/*' -print0 | LC_ALL=C sort -z | xargs -0 shasum -a 256 >SHA256SUMS)
echo "corpus ready in $out ($(grep -c '^\[\[file\]\]' "$manifest") files in the manifest)"
