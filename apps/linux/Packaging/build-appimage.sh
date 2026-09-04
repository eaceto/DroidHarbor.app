#!/usr/bin/env bash
# Build a DroidHarbor AppImage.
#
# Runs inside the droidharbor-build container (see ../Dockerfile), which pins
# the compatibility floor: whatever glibc and GTK that image has is the oldest
# system the result will start on.
#
# GTK does not travel by itself. Beyond the shared libraries, the bundle needs
# the gdk-pixbuf loader cache, compiled GSettings schemas, and an icon theme —
# without them the app launches and renders nothing. linuxdeploy-plugin-gtk
# handles that, which is the only reason this is a script and not one cargo
# command.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
crate="$(dirname "$here")"
repo="$(cd "$crate/../.." && pwd)"

APP_ID="dev.eaceto.apps.linux.droidharbor"
version="${DH_VERSION:-0.1.0}"
arch="$(uname -m)"
target_dir="${CARGO_TARGET_DIR:-$crate/target}"
out_dir="${DH_OUT:-$crate/build}"
appdir="$target_dir/AppDir"

echo "==> Building release binary"
cargo build --release --bin droidharbor --manifest-path "$crate/Cargo.toml"

echo "==> Assembling AppDir"
rm -rf "$appdir"
install -Dm755 "$target_dir/release/droidharbor" "$appdir/usr/bin/droidharbor"
# Every size the theme spec expects, so the launcher, the switcher and the
# window list each pick one rendered for their size rather than downscaling the
# 512 and blurring it.
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 "$crate/Resources/icons/icon_${size}.png" \
        "$appdir/usr/share/icons/hicolor/${size}x${size}/apps/$APP_ID.png"
done
install -Dm644 "$here/$APP_ID.desktop" \
    "$appdir/usr/share/applications/$APP_ID.desktop"

echo "==> Bundling the icon theme"
# linuxdeploy-plugin-gtk does NOT bundle an icon theme, and the app names icons
# from Adwaita throughout. Leaving it to the host means the UI renders without
# icons wherever the theme is absent, older, or simply not selected — so carry
# it, and stop depending on the host for anything.
cp -r /usr/share/icons/Adwaita "$appdir/usr/share/icons/"
# A theme without an index is not a theme as far as GTK is concerned.
cp /usr/share/icons/hicolor/index.theme "$appdir/usr/share/icons/hicolor/index.theme"
for theme in Adwaita hicolor; do
    gtk4-update-icon-cache --force --quiet "$appdir/usr/share/icons/$theme" 2>/dev/null \
        || gtk-update-icon-cache --force --quiet "$appdir/usr/share/icons/$theme" 2>/dev/null \
        || echo "    (no icon cache tool; GTK will scan the directories instead)"
done

echo "==> Bundling GTK and its runtime furniture"
# DEPLOY_GTK_VERSION tells the plugin which stack to pull in; without it the
# plugin guesses from the binary and can bundle GTK 3 alongside GTK 4.
export DEPLOY_GTK_VERSION=4
export OUTPUT="$out_dir/DroidHarbor-$version-$arch.AppImage"
mkdir -p "$out_dir"

linuxdeploy \
    --appdir "$appdir" \
    --plugin gtk \
    --desktop-file "$appdir/usr/share/applications/$APP_ID.desktop" \
    --icon-file "$appdir/usr/share/icons/hicolor/512x512/apps/$APP_ID.png" \
    --output appimage

echo "==> Built $OUTPUT"
ls -lh "$OUTPUT"
