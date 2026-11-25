#!/usr/bin/env bash
# Writes assets/licenses/third-party.html (S9.9.2), the licenses About VLC shows: the libVLC
# notice with the LGPL and GPL texts, then every crate in Cargo.lock under its license, from
# cargo-about (`cargo install cargo-about --locked --features cli`) and about.toml. Run it
# after changing dependencies; a test fails while the page lists other crates than Cargo.lock.
#
#   tools/licenses/generate.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
here="$root/tools/licenses"
licenses="$root/packaging/libvlc/licenses"
out="$root/assets/licenses/third-party.html"

escape() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' "$1"; }

# Copies its input, putting the file named by $2 where a line holds only the marker $1.
splice() {
  local marker="$1" file="$2"
  awk -v marker="$marker" -v file="$file" '
    $0 == marker { while ((getline line < file) > 0) print line; next }
    { print }'
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
escape "$licenses/LGPL-2.1.txt" >"$work/lgpl"
escape "$licenses/GPL-2.0.txt" >"$work/gpl"
splice @LGPL@ "$work/lgpl" <"$here/libvlc.html" | splice @GPL@ "$work/gpl" >"$work/libvlc"

cd "$root"
cargo about generate --frozen --fail -c about.toml "$here/about.hbs" -o "$work/crates"
splice @LIBVLC_NOTICE@ "$work/libvlc" <"$work/crates" >"$out"
echo "wrote $out ($(wc -c <"$out" | tr -d ' ') bytes)"
