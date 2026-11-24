#!/usr/bin/env bash
# Deletes the app's object files that no binary uses any more, from the build cache (target/, or
# the folder given as $1). Pass --dry-run first to see what would go.
#
# macOS only. There, debug builds keep their debug info in the object files beside each binary
# instead of copying it into the binary, so rustc keeps every rebuild's `.o` files, and it never
# deletes the ones a later rebuild replaced: the app's folders under target/debug/build grow by
# gigabytes a day. A binary lists the object files it reads (`nm -ap`, its OSO entries); every
# other one of the app's `.o` files is dead. Those are deleted, so backtraces keep their file and
# line numbers and nothing rebuilds. Run it when no cargo build is running.
set -euo pipefail

dry_run=false
if [[ "${1:-}" == "--dry-run" ]]; then
  dry_run=true
  shift
fi
target="${1:-target}"

# The package's folders, one per build of each binary, test binary and example, for every
# target triple and profile.
dirs=$(find "$target" -type d -path '*/build/RedesignedVLC/*/out' 2>/dev/null)
if [[ -z "$dirs" ]]; then
  echo "No build folders for the app under $target"
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Every object file any binary there reads.
while IFS= read -r dir; do
  find "$dir" -maxdepth 1 -type f -perm -u+x ! -name '*.*' -print0 |
    xargs -0 -n 1 nm -ap 2>/dev/null | awk '$5 == "OSO" { print $6 }' || true
done <<<"$dirs" | sort -u >"$work/used"

while IFS= read -r dir; do
  find "$(cd "$dir" && pwd)" -maxdepth 1 -type f -name '*.rcgu.o'
done <<<"$dirs" | sort >"$work/all"

comm -23 "$work/all" "$work/used" >"$work/dead"
count=$(wc -l <"$work/dead" | tr -d ' ')
# Hard links share space, so count each file's blocks only where this is its last link.
bytes=$(tr '\n' '\0' <"$work/dead" | xargs -0 stat -f '%l %b' 2>/dev/null |
  awk '$1 == 1 { sum += $2 * 512 } END { print sum + 0 }')
gib=$(awk -v b="$bytes" 'BEGIN { printf "%.1f", b / 1073741824 }')

if $dry_run; then
  echo "Would delete $count unused object files, freeing at least $gib GiB"
else
  tr '\n' '\0' <"$work/dead" | xargs -0 rm -f
  echo "Deleted $count unused object files, freeing at least $gib GiB"
fi
