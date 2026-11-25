#!/usr/bin/env bash
# Downloads the corpus archive pinned in tools/corpus/corpus.lock (S9.8.1), checks its
# SHA-256, unpacks it and verifies every file. On Windows (Git Bash) names-unix/ is left out:
# Windows can't hold those names.
#
#   tools/corpus/fetch.sh [folder]     (folder: target/corpus by default)
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
corpus="${1:-$root/target/corpus}"
lock="$root/tools/corpus/corpus.lock"
url="$(sed -n 's/^url=//p' "$lock")"
sha256="$(sed -n 's/^sha256=//p' "$lock")"
if [[ -z "$url" || -z "$sha256" ]]; then
  echo "no corpus is pinned in $lock yet: make one with tools/corpus/make.sh" >&2
  exit 1
fi

archive="$root/target/corpus-$sha256.tar.gz"
if [[ ! -s "$archive" ]]; then
  echo "downloading $url"
  curl --fail --location --proto '=https' --output "$archive.part" "$url"
  mv "$archive.part" "$archive"
fi
echo "$sha256  $archive" | shasum -a 256 --quiet -c - || {
  rm -f "$archive"
  echo "the download doesn't match corpus.lock's checksum" >&2
  exit 1
}

mkdir -p "$corpus"
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) tar -C "$corpus" --exclude ./names-unix -xzf "$archive" ;;
  *) tar -C "$corpus" -xzf "$archive" ;;
esac
"$root/tools/corpus/verify.sh" "$corpus"
