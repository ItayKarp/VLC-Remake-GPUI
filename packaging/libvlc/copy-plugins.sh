#!/usr/bin/env bash
# Copies a libVLC plugins folder into a package, leaving out excluded-plugins.txt (S9.4.4).
#
# Usage: packaging/libvlc/copy-plugins.sh <plugins folder> <destination>
#
# Files keep their modification times (cp -p): libVLC trusts plugins.dat only while every
# plugin's time and size match it, and otherwise scans all ~340 plugins at every start, logging
# an error for each. Leaving plugins out doesn't spoil the cache: libVLC ignores its entries for
# files that aren't there.
set -euo pipefail

source="$1"
destination="$2"
here="$(cd "$(dirname "$0")" && pwd)"

[[ -d "$source" ]] || { echo "No plugins folder at $source" >&2; exit 1; }
rm -rf "$destination"
mkdir -p "$(dirname "$destination")"
cp -pR "$source" "$destination"

removed=0
while read -r name; do
    [[ -z "$name" || "$name" == \#* ]] && continue
    while IFS= read -r -d '' file; do
        rm -f "$file"
        removed=$((removed + 1))
    done < <(find "$destination" -type f -name "lib${name}_plugin.*" -print0)
done < "$here/excluded-plugins.txt"

# BD-J menus run in Java, which the packages don't ship.
find "$destination" -type f -name '*.jar' -delete

echo "Copied $(find "$destination" -type f -name 'lib*_plugin.*' | wc -l | tr -d ' ') plugins," \
    "left out $removed"
