#!/usr/bin/env bash
# Build and package DroidHarbor for Linux distribution.
#
#   ./release.sh 1.2.0            # build this machine's architecture
#   ./release.sh 1.2.0 --notes "Faster transfers"
#   ./release.sh --check          # report what is set up and what is not
#
# Produces, in build/release/:
#   DroidHarbor-<version>-<arch>.AppImage
#   SHA256SUMS
#   updates-linux.json            # what the in-app update check reads
#
# The AppImage is built inside the ubuntu:24.04 container, never against the
# host, so the glibc floor and the bundled GTK are the same wherever this runs.
#
# One machine builds one architecture. Drop an AppImage built elsewhere — by CI,
# or on another machine — into build/release/ before running, and it is picked
# up and listed in the manifest alongside this one, so a single manifest can
# describe both architectures. Only files matching the version being released
# are considered; older builds in the directory are ignored.
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="DroidHarbor"
IMAGE="droidharbor-build:24.04"
DOWNLOAD_BASE="${DH_DOWNLOAD_BASE:-https://github.com/eaceto/DroidHarbor.app/releases/latest/download}"
BUILD_DIR="$PWD/build"
RELEASE_DIR="$BUILD_DIR/release"

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }
fail() { printf '\033[31merror:\033[0m %s\n' "$1" >&2; exit 1; }

check() {
    printf 'docker            '
    docker info >/dev/null 2>&1 && echo "ok" || echo "NOT RUNNING"
    printf 'build image       '
    docker image inspect "$IMAGE" >/dev/null 2>&1 \
        && echo "ok ($IMAGE)" \
        || echo "missing — docker build -t $IMAGE ."
    printf 'host architecture '
    uname -m | sed 's/^arm64$/arm64 (builds aarch64)/'
    printf 'existing builds   '
    # A glob, not `ls` or `find`: both fail when the directory is absent or
    # empty, and `set -e` with `pipefail` would take the script down with them.
    shopt -s nullglob
    local existing=("$RELEASE_DIR"/*.AppImage)
    echo "${#existing[@]}"
}

if [ "${1:-}" = "--check" ]; then
    check
    exit 0
fi

VERSION="${1:-}"
[ -n "$VERSION" ] || fail "usage: ./release.sh <version> [--notes \"…\"]"
# Refuse anything the update check cannot compare, rather than shipping a
# release nobody is offered.
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]] || fail "version must look like 1.2.0, got '$VERSION'"

NOTES=""
if [ "${2:-}" = "--notes" ]; then
    NOTES="${3:-}"
fi

docker info >/dev/null 2>&1 || fail "docker is not running"
docker image inspect "$IMAGE" >/dev/null 2>&1 || {
    step "Building the build image"
    docker build -t "$IMAGE" .
}

# The artifact is named by the container's architecture, not the host's. On an
# Apple Silicon Mac `uname -m` says arm64 while Linux calls the same thing
# aarch64, and the script would then look for a file that was never written.
step "Setting the crate version"
# The binary reports CARGO_PKG_VERSION in About and compares it against the
# manifest. Left at its old value, a freshly released build would compare older
# than the release it came from and offer itself as an update, forever.
# awk, not sed: `0,/re/` is a GNU extension that BSD sed on macOS ignores
# without complaint, silently leaving the version untouched.
awk -v v="$VERSION" '
    /^version = / && !done { print "version = \"" v "\""; done = 1; next }
    { print }
' Cargo.toml > Cargo.toml.new && mv Cargo.toml.new Cargo.toml
grep -m1 '^version' Cargo.toml
grep -q "version = \"$VERSION\"" Cargo.toml || fail "could not set the version in Cargo.toml"

ARCH="$(uname -m)"
case "$ARCH" in
    arm64) ARCH="aarch64" ;;
    amd64) ARCH="x86_64" ;;
esac
mkdir -p "$RELEASE_DIR"

step "Building the AppImage ($ARCH)"
docker run --rm \
    -v "$PWD/../..":/src \
    -v droidharbor-cargo-registry:/root/.cargo/registry \
    -v droidharbor-target:/target \
    -e CARGO_TARGET_DIR=/target \
    -e DH_VERSION="$VERSION" \
    -e DH_OUT=/src/apps/linux/build/release \
    -w /src/apps/linux \
    "$IMAGE" ./Packaging/build-appimage.sh

IMAGE_FILE="$RELEASE_DIR/$APP_NAME-$VERSION-$ARCH.AppImage"
[ -f "$IMAGE_FILE" ] || fail "expected $IMAGE_FILE"

step "Verifying the bundle"
# A bundle missing its runtime furniture still builds and still exits zero; it
# only fails when a user runs it.
docker run --rm -v "$RELEASE_DIR":/out -w /tmp "$IMAGE" sh -c "
    cp /out/$(basename "$IMAGE_FILE") . && ./$(basename "$IMAGE_FILE") --appimage-extract >/dev/null 2>&1
    missing=0
    for path in usr/share/glib-2.0/schemas/gschemas.compiled usr/share/icons/Adwaita usr/lib/gdk-pixbuf-2.0; do
        [ -e \"squashfs-root/\$path\" ] || { echo \"missing: \$path\"; missing=1; }
    done
    ls squashfs-root/usr/lib/gdk-pixbuf-2.0/*/loaders/ | grep -q svg \
        || { echo 'missing: the SVG pixbuf loader — every SVG-only icon would render broken'; missing=1; }
    exit \$missing
" || fail "the bundle is incomplete"

step "Checksums"
( cd "$RELEASE_DIR" && sha256sum "$APP_NAME-$VERSION"-*.AppImage | tee SHA256SUMS )

step "Publishing the update manifest"
# Written from whatever is present, so an AppImage built elsewhere and dropped
# in here is described alongside this one.
{
    printf '{\n  "version": "%s",\n' "$VERSION"
    if [ -n "$NOTES" ]; then
        printf '  "notes": "%s",\n' "${NOTES//\"/\\\"}"
    else
        printf '  "notes": null,\n'
    fi
    printf '  "artifacts": {\n'
    first=1
    # Scoped to this version: an older AppImage left in the directory would
    # otherwise appear under the same architecture key twice, and the last one
    # written would silently win.
    for file in "$RELEASE_DIR/$APP_NAME-$VERSION"-*.AppImage; do
        name="$(basename "$file")"
        # DroidHarbor-<version>-<arch>.AppImage
        arch="${name##*-}"; arch="${arch%.AppImage}"
        sum="$(sha256sum "$file" | cut -d' ' -f1)"
        [ $first -eq 1 ] || printf ',\n'
        first=0
        printf '    "%s": { "url": "%s/%s", "sha256": "%s" }' "$arch" "$DOWNLOAD_BASE" "$name" "$sum"
    done
    printf '\n  }\n}\n'
} > "$RELEASE_DIR/updates-linux.json"
cat "$RELEASE_DIR/updates-linux.json"

step "Ready to publish"
echo "Upload every file below to the GitHub release. The manifest is what the"
echo "in-app check reads; without it nobody is told the release exists."
echo
ls -1 "$RELEASE_DIR"
echo
echo "The manifest names only the architectures present here. To cover both,"
echo "fetch the other AppImage into $RELEASE_DIR and run this again."
