#!/usr/bin/env bash
# Builds the macOS app (S9.4.2): VLC.app with libVLC inside, signed, in a DMG, notarized when
# asked, and the app zipped for the updater (S9.5). Run it on a Mac from anywhere in the
# repository; the results land in dist/.
#
# Usage: packaging/macos/build-app.sh [options]
#   --arch universal|arm64|x86_64  What to build for (default: universal, arm64 and x86_64 in
#                                  one binary).
#   --vlc <path>                   The libVLC to ship (S9.4.4): a VLC.app from videolan.org (a
#                                  universal one for a universal build), or a folder holding
#                                  lib/ and plugins/ like vendor/libvlc/macos-arm64. Default:
#                                  vendor/libvlc/macos-<arch>, for one architecture only.
#   --sign <identity>              Sign with this "Developer ID Application: …" identity, with
#                                  the hardened runtime. Default: an ad-hoc signature, which
#                                  runs on this Mac only (Gatekeeper quarantines it elsewhere).
#   --notarize <profile>           Notarize the DMG with this notarytool keychain profile
#                                  (`xcrun notarytool store-credentials`) and staple the ticket.
#                                  Needs --sign.
#
# RVLC_FEATURES adds cargo features, such as "runtime_shaders" on a Mac without Xcode's Metal
# toolchain (shipped builds should compile the shaders in instead). Cargo's profile is
# `release`, or RVLC_PROFILE.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
here="$root/packaging/macos"
name="RedesignedVLC"
app_name="VLC"
bundle_id="org.redesignedvlc.app"
# GPUI's Metal renderer and the arm64 half both need macOS 11.
min_macos="11.0"
profile="${RVLC_PROFILE:-release}"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
out="$root/dist"

arch="universal"
vlc=""
identity=""
notary_profile=""

fail() { echo "build-app: $*" >&2; exit 1; }

parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --arch) arch="$2"; shift 2 ;;
            --vlc) vlc="$2"; shift 2 ;;
            --sign) identity="$2"; shift 2 ;;
            --notarize) notary_profile="$2"; shift 2 ;;
            *) fail "unknown option $1 (see the usage at the top of this script)" ;;
        esac
    done
    case "$arch" in
        universal) architectures=(arm64 x86_64) ;;
        arm64 | x86_64) architectures=("$arch") ;;
        *) fail "--arch is universal, arm64 or x86_64, not $arch" ;;
    esac
    [[ -z "$notary_profile" || -n "$identity" ]] || fail "--notarize needs --sign"
}

rust_target() { echo "${1/arm64/aarch64}-apple-darwin"; }

version() { sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1; }

# The runtime's lib/ and plugins/ folders, from --vlc or the vendored runtime.
locate_runtime() {
    if [[ -z "$vlc" ]]; then
        [[ "$arch" != universal ]] || fail "a universal build needs --vlc <universal VLC.app>"
        vlc="$root/vendor/libvlc/macos-${arch/x86_64/x64}"
    fi
    if [[ "$vlc" == *.app || -d "$vlc/Contents/MacOS/lib" ]]; then
        runtime_libs="$vlc/Contents/MacOS/lib"
        runtime_plugins="$vlc/Contents/MacOS/plugins"
    else
        runtime_libs="$vlc/lib"
        runtime_plugins="$vlc/plugins"
    fi
    [[ -f "$runtime_libs/libvlc.dylib" ]] || fail "no libvlc.dylib in $runtime_libs"
    local have
    have="$(lipo -archs "$runtime_libs/libvlccore.dylib")"
    for a in "${architectures[@]}"; do
        [[ " $have " == *" $a "* ]] || fail "$runtime_libs has no $a libVLC (it has: $have)"
    done
}

build_binaries() {
    local features=()
    [[ -z "${RVLC_FEATURES:-}" ]] || features=(--features "$RVLC_FEATURES")
    binaries=()
    for a in "${architectures[@]}"; do
        local triple
        triple="$(rust_target "$a")"
        rustup target add "$triple" > /dev/null
        MACOSX_DEPLOYMENT_TARGET="$min_macos" \
            cargo build --manifest-path "$root/Cargo.toml" --profile "$profile" \
            --target "$triple" "${features[@]}"
        binaries+=("$target_dir/$triple/$profile/$name")
    done
}

# The bundle's own keys, then Info.plist.ext's: file types, URL schemes and usage descriptions.
write_info_plist() {
    local plist="$1"
    cat > "$plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>$app_name</string>
  <key>CFBundleExecutable</key><string>$name</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleIdentifier</key><string>$bundle_id</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>$app_name</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$(version)</string>
  <key>CFBundleVersion</key><string>$(version)</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.video</string>
  <key>LSMinimumSystemVersion</key><string>$min_macos</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST
    /usr/libexec/PlistBuddy -c "Merge $here/Info.plist.ext" "$plist" > /dev/null
    plutil -lint -s "$plist"
}

# VLC.app: the binary, Info.plist and icon, and libVLC in Contents/Frameworks/libvlc, where
# src/engine/libvlc/runtime.rs looks for it.
assemble_app() {
    app="$out/$app_name.app"
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks/libvlc/lib"
    lipo -create "${binaries[@]}" -output "$app/Contents/MacOS/$name"
    write_info_plist "$app/Contents/Info.plist"
    printf 'APPL????' > "$app/Contents/PkgInfo"
    cp "$root/assets/app-icon/icon.icns" "$app/Contents/Resources/AppIcon.icns"
    cp -pR "$runtime_libs"/libvlc*.dylib "$app/Contents/Frameworks/libvlc/lib/"
    bash "$root/packaging/libvlc/copy-plugins.sh" "$runtime_plugins" \
        "$app/Contents/Frameworks/libvlc/plugins"
    thin_runtime
}

# A single-architecture app doesn't carry a universal VLC's other half.
thin_runtime() {
    [[ "$arch" != universal ]] || return 0
    local file
    while IFS= read -r -d '' file; do
        [[ "$(lipo -archs "$file")" == "$arch" ]] && continue
        local stamp
        stamp="$(mktemp)"
        touch -r "$file" "$stamp"
        lipo "$file" -thin "$arch" -output "$file"
        touch -r "$stamp" "$file"
        rm -f "$stamp"
    done < <(find "$app/Contents/Frameworks" -type f -name '*.dylib' -print0)
}

# Signs inside out: every dylib, then the app. A Developer ID gets the hardened runtime and
# a secure timestamp, which notarization requires; libVLC's dylibs then load because they carry
# the same team's signature. Without one, an ad-hoc signature keeps arm64 Macs willing to run it.
sign_app() {
    local flags=(--force --sign "${identity:--}")
    [[ -z "$identity" ]] || flags+=(--options runtime --timestamp)
    find "$app/Contents/Frameworks" -type f -name '*.dylib' -print0 |
        xargs -0 -n 64 codesign "${flags[@]}" 2> >(grep -v 'replacing existing signature' >&2)
    write_plugin_cache
    # Everything in Contents/Frameworks must be signed, even data; as in VLC.app, the cache's
    # signature goes in an extended attribute, which leaves its contents alone.
    codesign "${flags[@]}" "$app/Contents/Frameworks/libvlc/plugins/plugins.dat"
    local entitlements=()
    [[ -z "$identity" ]] || entitlements=(--entitlements "$here/entitlements.plist")
    codesign "${flags[@]}" "${entitlements[@]}" "$app"
    codesign --verify --strict --deep "$app"
}

# Signing changed every plugin's size and time, so the plugins.dat copied with them no longer
# matches and libVLC would scan all of them at every start. A fresh one is written before the
# app's signature seals it. The helper runs as the host's architecture when the app has it, and
# under Rosetta otherwise.
write_plugin_cache() {
    local helper_arch="${architectures[0]}" helper libs="$app/Contents/Frameworks/libvlc/lib"
    [[ " ${architectures[*]} " != *" $(uname -m) "* ]] || helper_arch="$(uname -m)"
    helper="$(mktemp -d)/plugin-cache"
    cc -arch "$helper_arch" -O2 -o "$helper" "$here/plugin-cache.c"
    rm -f "$app/Contents/Frameworks/libvlc/plugins/plugins.dat"
    VLC_PLUGIN_PATH="$app/Contents/Frameworks/libvlc/plugins" arch "-$helper_arch" "$helper" \
        "$libs/libvlccore.dylib" "$libs/libvlc.dylib"
    rm -rf "$(dirname "$helper")"
    [[ -f "$app/Contents/Frameworks/libvlc/plugins/plugins.dat" ]] || fail "no plugins.dat written"
}

make_dmg() {
    dmg="$out/$name-$(version)-macos-$arch.dmg"
    local stage
    stage="$(mktemp -d)"
    cp -pR "$app" "$stage/"
    ln -s /Applications "$stage/Applications"
    rm -f "$dmg"
    hdiutil create -quiet -volname "$app_name" -srcfolder "$stage" -fs HFS+ -format UDZO "$dmg"
    rm -rf "$stage"
    [[ -z "$identity" ]] || codesign --sign "$identity" --timestamp "$dmg"
}

# Notarizing the DMG notarizes the app in it too; the ticket is stapled to the DMG, and
# Gatekeeper fetches the app's own online once it's copied out.
notarize() {
    [[ -n "$notary_profile" ]] || return 0
    xcrun notarytool submit "$dmg" --keychain-profile "$notary_profile" --wait
    xcrun stapler staple "$dmg"
    spctl --assess --type open --context context:primary-signature --verbose "$dmg"
}

# The app zipped for the updater (S9.5.1), which unzips it with ditto beside the installed app.
# A notarized build staples the app's own ticket first, so the update opens offline too.
make_update_zip() {
    [[ -z "$notary_profile" ]] || xcrun stapler staple "$app"
    ditto -c -k --keepParent "$app" "$out/$name-$(version)-macos-$arch.app.zip"
}

keep_debug_symbols() {
    for a in "${architectures[@]}"; do
        local dsym
        dsym="$target_dir/$(rust_target "$a")/$profile/$name.dSYM"
        [[ ! -d "$dsym" ]] || ditto -c -k --keepParent "$dsym" "$out/$name-$(version)-$a.dSYM.zip"
    done
}

main() {
    parse_arguments "$@"
    locate_runtime
    mkdir -p "$out"
    build_binaries
    assemble_app
    sign_app
    make_dmg
    notarize
    make_update_zip
    keep_debug_symbols
    ls -l "$out"
}

main "$@"
