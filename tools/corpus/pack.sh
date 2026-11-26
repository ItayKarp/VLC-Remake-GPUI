#!/usr/bin/env bash
# Packs a corpus folder (S9.8.1) into dist/corpus/rvlc-corpus-<date>.tar.gz for the artifact
# store, leaving out the 4 GiB file (huge/), which release assets can't hold. Upload the
# archive, then put its link and checksum in tools/corpus/corpus.lock (docs/qa.md).
#
#   tools/corpus/pack.sh [folder]     (folder: target/corpus by default)
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
corpus="${1:-$root/target/corpus}"
"$root/tools/corpus/verify.sh" "$corpus"

made="$(sed -n 's/^made = "\(.*\)"$/\1/p' "$corpus/manifest.toml")"
archive="$root/dist/corpus/rvlc-corpus-$made.tar.gz"
mkdir -p "$(dirname "$archive")"
# pax keeps names that aren't ASCII as they are.
tar -C "$corpus" --format pax --exclude ./huge --exclude .DS_Store -czf "$archive" .
sum="$(shasum -a 256 "$archive" | cut -d ' ' -f 1)"
echo "$sum  $(basename "$archive")" >"$archive.sha256"
echo "packed $archive ($(du -h "$archive" | cut -f 1))"
echo "after uploading it, set tools/corpus/corpus.lock to:"
echo "  url=<the archive's link>"
echo "  sha256=$sum"
