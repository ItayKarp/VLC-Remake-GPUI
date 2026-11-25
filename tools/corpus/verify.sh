#!/usr/bin/env bash
# Checks a corpus folder against its SHA256SUMS (S9.8.1): a file changed, cut short or missing
# fails. The 4 GiB file (huge/) is made where it is used and has no checksum; without
# names-unix/ (left out on Windows) its files aren't expected.
#
#   tools/corpus/verify.sh [folder]     (folder: target/corpus by default)
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "${1:-$root/target/corpus}"
if [[ -d names-unix ]]; then
  sums="$(cat SHA256SUMS)"
else
  sums="$(grep -v '  \./names-unix/' SHA256SUMS)"
fi
shasum -a 256 --quiet -c - <<<"$sums"
echo "corpus verified: $(grep -c . <<<"$sums") files"
