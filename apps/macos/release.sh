#!/bin/bash
# Build, sign, notarize and package DroidHarbor for sharing.
#
#   ./release.sh --check                  # report what is set up and what is not
#   ./release.sh 1.2.0                    # build + sign + notarize + staple + DMG
#   DH_SKIP_NOTARIZE=1 ./release.sh 1.2.0 # stop after signing (local testing)
#
# The version is the marketing version users see; the build number is derived
# from it so it always increases. Publishing build/updates.json next to the
# DMG is what makes the in-app update check notice the release.
#
# Requirements:
#   * a "Developer ID Application" certificate in the login keychain
#     (Xcode → Settings → Accounts → Manage Certificates → + )
#   * notarytool credentials stored once as a keychain profile:
#       xcrun notarytool store-credentials droidharbor \
#         --apple-id <your-apple-id> --team-id 2U378HJ7FG \
#         --password <app-specific-password>
#     (override the profile name with DH_NOTARY_PROFILE)
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="DroidHarbor"
SCHEME="DroidHarbor"
NOTARY_PROFILE="${DH_NOTARY_PROFILE:-droidharbor}"
DOWNLOAD_BASE="${DH_DOWNLOAD_BASE:-https://github.com/eaceto/DroidHarbor.app/releases/latest/download}"
BUILD_DIR="$PWD/build"
ARCHIVE="$BUILD_DIR/$APP_NAME.xcarchive"
EXPORT_DIR="$BUILD_DIR/export"
APP="$EXPORT_DIR/$APP_NAME.app"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }
fail() { printf '\033[31merror:\033[0m %s\n' "$1" >&2; exit 1; }

# --check reports readiness and exits, so the prerequisites can be sorted out
# before waiting on a full universal build.
if [ "${1:-}" = "--check" ]; then
    ready=0
    printf '\n\033[1mRelease prerequisites\033[0m\n\n'

    if security find-identity -v -p codesigning | grep -q "Developer ID Application"; then
        printf '  \033[32m✓\033[0m Developer ID Application certificate\n'
    else
        ready=1
        printf '  \033[31m✗\033[0m Developer ID Application certificate: MISSING\n'
        printf '      Xcode → Settings → Accounts → your Apple ID →\n'
        printf '      Manage Certificates → + → Developer ID Application\n'
        printf '      (needs the paid Individual/Organization team; the\n'
        printf '       Apple Development certificate cannot sign apps for\n'
        printf '       other people'"'"'s Macs)\n'
    fi

    if xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
        printf '  \033[32m✓\033[0m notarytool profile "%s"\n' "$NOTARY_PROFILE"
    else
        ready=1
        printf '  \033[31m✗\033[0m notarytool profile "%s": MISSING\n' "$NOTARY_PROFILE"
        printf '      xcrun notarytool store-credentials %s \\\n' "$NOTARY_PROFILE"
        printf '        --apple-id <your-apple-id> --team-id 2U378HJ7FG \\\n'
        printf '        --password <app-specific-password>\n'
        printf '      App-specific passwords: https://account.apple.com →\n'
        printf '      Sign-In and Security → App-Specific Passwords\n'
    fi

    for tool in tuist cargo protoc; do
        if command -v "$tool" >/dev/null 2>&1; then
            printf '  \033[32m✓\033[0m %s\n' "$tool"
        else
            ready=1
            printf '  \033[31m✗\033[0m %s: not on PATH\n' "$tool"
        fi
    done

    printf '\n'
    [ "$ready" -eq 0 ] && printf 'Ready: run ./release.sh\n\n' \
        || printf 'Fix the items above, or run DH_SKIP_NOTARIZE=1 ./release.sh to\ntest the pipeline without notarization.\n\n'
    exit "$ready"
fi

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    fail "a version is required, e.g. ./release.sh 1.2.0 (or --check)"
fi
if ! printf '%s' "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    fail "version must look like 1.2.0, got \"$VERSION\""
fi
# Monotonic build number derived from the version, so Sparkle-style
# comparisons and macOS itself always see it move forward.
# Tuist exposes environment to manifests through the TUIST_ prefix. The
# channel is what makes this the real app rather than the development one:
# it is set here and nowhere else, so a plain `tuist generate` can never
# produce a build that claims the shipping identity.
export TUIST_DH_CHANNEL=release
export TUIST_DH_VERSION="$VERSION"

# However this run ends, leave the workspace on the development channel. An
# Xcode build after a release should not quietly produce an app carrying the
# shipping identity.
restore_development_project() {
    unset TUIST_DH_CHANNEL TUIST_DH_VERSION TUIST_DH_BUILD
    tuist generate --no-open >/dev/null 2>&1 || true
}
trap restore_development_project EXIT
export TUIST_DH_BUILD=$(printf '%s' "$VERSION" | awk -F. '{ printf "%d", $1 * 10000 + $2 * 100 + $3 }')
printf '\nBuilding \033[1m%s\033[0m (build %s)\n' "$TUIST_DH_VERSION" "$TUIST_DH_BUILD"

step "Checking signing identity"
if ! security find-identity -v -p codesigning | grep -q "Developer ID Application"; then
    fail "no \"Developer ID Application\" certificate found in the keychain.
       Create one in Xcode → Settings → Accounts → Manage Certificates → +,
       then re-run. (An \"Apple Development\" certificate cannot be used for
       apps shared outside your own machines.)"
fi
IDENTITY=$(security find-identity -v -p codesigning \
    | grep "Developer ID Application" | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "using: $IDENTITY"

step "Generating the Xcode project"
tuist generate --no-open

step "Archiving (universal: Apple Silicon + Intel)"
rm -rf "$ARCHIVE" "$EXPORT_DIR"
xcodebuild archive \
    -workspace "$APP_NAME.xcworkspace" \
    -scheme "$SCHEME" \
    -configuration Release \
    -archivePath "$ARCHIVE" \
    -destination "generic/platform=macOS" \
    ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO \
    | grep -E "^(\*\*|error:)" || true
[ -d "$ARCHIVE" ] || fail "archive failed"

step "Exporting a Developer ID signed app"
xcodebuild -exportArchive \
    -archivePath "$ARCHIVE" \
    -exportPath "$EXPORT_DIR" \
    -exportOptionsPlist ExportOptions.plist \
    | grep -E "^(\*\*|error:)" || true
if [ ! -d "$APP" ]; then
    fail "export failed, see the output above.
       If it reported \"errSecInternalComponent\", codesign could not reach
       the signing key: macOS asks for permission the first time and the
       export fails if that dialog is dismissed. Either re-run and click
       \"Always Allow\", or grant it once and for all with:
         security set-key-partition-list -S apple-tool:,apple:,codesign: \\
           -s -k <your-login-password> ~/Library/Keychains/login.keychain-db"
fi

step "Verifying the signature"
codesign --verify --deep --strict --verbose=2 "$APP"
# The Rust core must be inside the binary: a dylib reference to this checkout
# would break the app on every other machine.
if otool -L "$APP/Contents/MacOS/$APP_NAME" | grep -q "target/.*libdh_ffi"; then
    fail "the app links libdh_ffi dynamically from this checkout; it must be
       linked statically (see build-rust.sh)"
fi
lipo -archs "$APP/Contents/MacOS/$APP_NAME"

step "Notarizing the app"
if [ -z "${DH_SKIP_NOTARIZE:-}" ]; then
    if ! xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
        fail "no notarytool keychain profile named \"$NOTARY_PROFILE\".
       Create it once with:
         xcrun notarytool store-credentials $NOTARY_PROFILE \\
           --apple-id <your-apple-id> --team-id 2U378HJ7FG \\
           --password <app-specific-password>
       App-specific passwords come from https://account.apple.com → Sign-In
       and Security. Or re-run with DH_SKIP_NOTARIZE=1 to skip."
    fi
    APP_ZIP="$BUILD_DIR/$APP_NAME-app.zip"
    rm -f "$APP_ZIP"
    ditto -c -k --keepParent "$APP" "$APP_ZIP"
    xcrun notarytool submit "$APP_ZIP" --keychain-profile "$NOTARY_PROFILE" --wait
    rm -f "$APP_ZIP"

    # The DMG gets its own ticket below, but that one is left behind when
    # the app is dragged out of the image; staple the app itself too, so it
    # validates offline wherever it ends up.
    step "Stapling the app"
    xcrun stapler staple "$APP" || fail "could not staple the app; is it notarized?"
fi

step "Building the disk image"
DMG_STAGE="$BUILD_DIR/dmg"
DMG="$BUILD_DIR/$APP_NAME.dmg"
DMG_RW="$BUILD_DIR/$APP_NAME-rw.dmg"
rm -rf "$DMG_STAGE" "$DMG" "$DMG_RW"
mkdir -p "$DMG_STAGE/.background"
cp -R "$APP" "$DMG_STAGE/"
ln -s /Applications "$DMG_STAGE/Applications"
cp Packaging/dmg-background.tiff "$DMG_STAGE/.background/background.tiff"

# A read-write image first: the window layout below is stored in the volume,
# so it has to be written before the image is compressed and sealed.
hdiutil create -volname "$APP_NAME" -srcfolder "$DMG_STAGE" \
    -fs HFS+ -format UDRW -ov "$DMG_RW" >/dev/null
MOUNT=$(hdiutil attach "$DMG_RW" -nobrowse -noverify -noautoopen \
    | grep -o '/Volumes/.*' | head -1)
[ -n "$MOUNT" ] || fail "could not mount the working image"

# Finder arranges the window: app on the left, Applications on the right,
# the background art drawing the arrow between them. Driving Finder needs
# Automation permission, so a refusal here is a warning, not a failure:
# the image still installs correctly, it just opens unstyled.
if ! osascript >/dev/null 2>&1 <<APPLESCRIPT
tell application "Finder"
    tell disk "$APP_NAME"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set the bounds of container window to {200, 120, 860, 520}
        set viewOptions to the icon view options of container window
        set arrangement of viewOptions to not arranged
        set icon size of viewOptions to 128
        set text size of viewOptions to 12
        set background picture of viewOptions to file ".background:background.tiff"
        set position of item "$APP_NAME.app" of container window to {165, 205}
        set position of item "Applications" of container window to {495, 205}
        close
        open
        update without registering applications
        delay 1
    end tell
end tell
APPLESCRIPT
then
    printf '\033[33mwarning:\033[0m Finder would not arrange the window (Automation\n'
    printf '         permission?). The image is still valid, just unstyled.\n'
fi

chmod -Rf go-w "$MOUNT" 2>/dev/null || true
sync
hdiutil detach "$MOUNT" -quiet

hdiutil convert "$DMG_RW" -format UDZO -imagekey zlib-level=9 -o "$DMG" >/dev/null
rm -f "$DMG_RW"
codesign --sign "$IDENTITY" --timestamp "$DMG"

if [ -n "${DH_SKIP_NOTARIZE:-}" ]; then
    step "Done (notarization skipped)"
    echo "$DMG"
    echo
    echo "Without notarization macOS will warn on other machines."
    exit 0
fi

step "Notarizing the disk image"
xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait

step "Stapling the ticket"
xcrun stapler staple "$DMG"
xcrun stapler validate "$DMG"

step "Publishing the update manifest"
# The in-app check reads this; upload it alongside the DMG.
cat > "$BUILD_DIR/updates.json" <<MANIFEST
{
  "version": "$VERSION",
  "url": "$DOWNLOAD_BASE/$APP_NAME.dmg",
  "notes": null
}
MANIFEST
echo "$BUILD_DIR/updates.json"
echo
echo "Upload BOTH files to the GitHub release so the in-app check sees it:"
echo "  $DMG"
echo "  $BUILD_DIR/updates.json"

step "Ready to share"
echo "$DMG"
echo
echo "Recipients can open this on any Mac (Apple Silicon or Intel) with no"
echo "Gatekeeper warnings."
