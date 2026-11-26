#!/usr/bin/env bash
# Builds the AppImage (S9.4.3): dist/RedesignedVLC-<version>-<arch>.AppImage, which carries
# libVLC and its plugins (S9.4.4) from the build machine, with the libraries they need.
#
# Usage: packaging/linux/build-appimage.sh <release binary> <version>
#
# Needs the system's libVLC (Debian: libvlc5 vlc-plugin-base, plus vlc-plugin-video-output for
# its OpenGL converters), and linuxdeploy with its AppImage plugin on PATH, or LINUXDEPLOY set
# to the linuxdeploy AppImage (github.com/linuxdeploy/linuxdeploy releases). Build on the oldest
# distribution the AppImage should run on: it needs that glibc or newer.
set -euo pipefail

binary="$1"
version="$2"
root="$(cd "$(dirname "$0")/../.." && pwd)"
name="redesigned-vlc"
linuxdeploy="${LINUXDEPLOY:-linuxdeploy}"
out="$root/dist"
appdir="$(mktemp -d)/AppDir"
trap 'rm -rf "$(dirname "$appdir")"' EXIT

fail() { echo "build-appimage: $*" >&2; exit 1; }

# The folder holding the system's libvlc.so.5, from the linker's cache.
system_libvlc_dir() {
    local library
    library="$(/sbin/ldconfig -p | sed -n 's/.*libvlc\.so\.5 .*=> //p' | head -1)"
    [[ -n "$library" ]] || fail "libVLC isn't installed (Debian: libvlc5 vlc-plugin-base)"
    dirname "$(readlink -f "$library")"
}

# libVLC goes beside the other libraries, and its plugins where libVLC 3 looks first on Linux:
# lib/vlc/plugins beside libvlccore. The plugins keep their file times for plugins.dat, and the
# AppImage's squashfs keeps them too.
copy_runtime() {
    local lib_dir="$1"
    mkdir -p "$appdir/usr/lib"
    cp -pL "$lib_dir/libvlc.so.5" "$lib_dir/libvlccore.so.9" "$appdir/usr/lib/"
    bash "$root/packaging/libvlc/copy-plugins.sh" "$lib_dir/vlc/plugins" \
        "$appdir/usr/lib/vlc/plugins"
}

# libVLC is loaded at run time, so linuxdeploy can't follow it from the binary: it's handed the
# libraries, and every plugin folder for the libraries the plugins need.
run_linuxdeploy() {
    local arguments=(--appdir "$appdir" --executable "$binary"
        --desktop-file "$root/packaging/linux/$name.desktop"
        --custom-apprun "$root/packaging/linux/AppRun"
        --library "$appdir/usr/lib/libvlc.so.5" --library "$appdir/usr/lib/libvlccore.so.9")
    local folder
    while IFS= read -r -d '' folder; do
        arguments+=(--deploy-deps-only "$folder")
    done < <(find "$appdir/usr/lib/vlc/plugins" -mindepth 1 -type d -print0)
    for size in 32 128 256 512; do
        local icon="$appdir/icons/${size}x${size}/$name.png"
        install -Dm644 "$root/assets/app-icon/icon-$size.png" "$icon"
        arguments+=(--icon-file "$icon")
    done
    "$linuxdeploy" "${arguments[@]}"
}

# linuxdeploy sets the plugins' RUNPATH, which changes their sizes and times, so the plugin
# cache is written again for the files as they now are, with the system's vlc-cache-gen.
write_plugin_cache() {
    local lib_dir="$1" plugins="$appdir/usr/lib/vlc/plugins"
    rm -f "$plugins/plugins.dat"
    "$lib_dir/vlc/vlc-cache-gen" "$plugins"
    [[ -f "$plugins/plugins.dat" ]] || fail "vlc-cache-gen wrote no plugins.dat"
}

command -v "$linuxdeploy" > /dev/null || fail "linuxdeploy isn't on PATH (or set LINUXDEPLOY)"
mkdir -p "$out"
lib_dir="$(system_libvlc_dir)"
copy_runtime "$lib_dir"
run_linuxdeploy
write_plugin_cache "$lib_dir"
OUTPUT="$out/RedesignedVLC-$version-$(uname -m).AppImage" \
    "$linuxdeploy" --appdir "$appdir" --output appimage
ls -l "$out"
