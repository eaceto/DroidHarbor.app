#!/usr/bin/env bash
# Make the installed release app's share extension available in Finder.
#
# Pass --only to hide the development one, so what you are testing is exactly
# what a user would get.
set -euo pipefail

ONLY=false
[ "${1:-}" = "--only" ] && ONLY=true

cd "$(dirname "$0")"
source ./channel-tools.sh

[ -d "$RELEASE_APP" ] || fail "$RELEASE_APP is not installed.
       Build and install one first:  ./release.sh <version>"

step "Release app"
echo "  $RELEASE_APP  (version $(plutil -extract CFBundleShortVersionString raw "$RELEASE_APP/Contents/Info.plist" 2>/dev/null || echo "?"))"
require_extension "$RELEASE_APP"

step "Registering it with LaunchServices"
lsregister -f "$RELEASE_APP"

step "Enabling its share extension"
enable_extension "$RELEASE_EXTENSION_ID"

if $ONLY; then
    step "Hiding the development copy (--only)"
    disable_extension "$DEV_EXTENSION_ID"
fi

step "Restarting Finder so the Share menu is rebuilt"
killall Finder 2>/dev/null || true
sleep 1

report_state
cat <<EOF

Ready. Right-click a file in Finder → Share → "DroidHarbor".

Both can be offered at once: ./use-dev-extension.sh adds "DroidHarbor Dev"
beside it. Pass --only to either script to hide the other.
EOF
