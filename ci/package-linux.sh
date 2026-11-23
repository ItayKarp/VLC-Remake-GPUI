#!/usr/bin/env bash
# Packages a Linux release build into dist/: a .deb for Debian-based systems and a .tar.gz of
# the same files. libVLC isn't bundled: the app loads the system's (the .deb depends on it).
#
# Usage: ci/package-linux.sh <release binary> <version>
#   <version> names the files and the package: "0.2.0", "0.2.0-rc.14", "0.2.0-pr12.3".
set -euo pipefail

binary="$1"
version="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"
name="redesigned-vlc"
arch="$(dpkg --print-architecture)"
out="$root/dist"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

rm -rf "$out"
mkdir -p "$out"

install -Dm755 "$binary" "$stage/usr/bin/RedesignedVLC"
install -Dm644 "$root/packaging/linux/$name.desktop" "$stage/usr/share/applications/$name.desktop"
for size in 32 128 256 512; do
    install -Dm644 "$root/assets/app-icon/icon-$size.png" \
        "$stage/usr/share/icons/hicolor/${size}x${size}/apps/$name.png"
done

tar -C "$stage" -czf "$out/$name-$version-linux-$arch.tar.gz" usr

# The shared libraries the binary links, as package dependencies. libVLC is loaded at run time,
# so dpkg-shlibdeps can't see it: it's listed by hand.
shlib_depends() {
    local scratch
    scratch="$(mktemp -d)"
    mkdir -p "$scratch/debian"
    printf 'Source: %s\n\nPackage: %s\nArchitecture: any\n' "$name" "$name" > "$scratch/debian/control"
    (cd "$scratch" && dpkg-shlibdeps -O "$1" 2>/dev/null) | sed -n 's/^shlibs:Depends=//p'
    rm -rf "$scratch"
}
depends="$(shlib_depends "$stage/usr/bin/RedesignedVLC")"

# Debian sorts "~" before anything, so 0.2.0~rc.14 comes before 0.2.0 as a pre-release should.
deb_version="${version//-/"~"}"
mkdir -p "$stage/DEBIAN"
cat > "$stage/DEBIAN/control" <<EOF
Package: $name
Version: $deb_version
Architecture: $arch
Maintainer: ItayKarp <ItayKarp@users.noreply.github.com>
Depends: ${depends:+$depends, }libvlc5, vlc-plugin-base
Section: video
Priority: optional
Homepage: https://github.com/ItayKarp/VLC-Remake-GPUI
Description: Redesigned VLC media player
 A VLC media player with a redesigned interface, built on GPUI and libVLC.
EOF

dpkg-deb --root-owner-group --build "$stage" "$out/${name}_${version}_$arch.deb"
ls -l "$out"
