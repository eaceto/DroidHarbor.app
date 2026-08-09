#!/usr/bin/env bash
# Make the development app's share extension available in Finder.
#
# Run this after building, whenever the extension changes: macOS caches the
# registered copy and will otherwise keep launching the previous build.
#
# Both channels can be offered at once, each under its own name, which is
# usually what you want while working. Pass --only to hide the other one when
# you need to be certain which build a test exercised.
set -euo pipefail

ONLY=false
[ "${1:-}" = "--only" ] && ONLY=true

cd "$(dirname "$0")"
source ./channel-tools.sh

APP=$(dev_app_path) || fail "no development build found. Build it first:
       xcodebuild -workspace DroidHarbor.xcworkspace -scheme DroidHarbor build"

step "Development app"
echo "  $APP"
require_extension "$APP"

step "Registering it with LaunchServices"
lsregister -f "$APP"

step "Enabling its share extension"
enable_extension "$DEV_EXTENSION_ID"

if $ONLY; then
    step "Hiding the release copy (--only)"
    disable_extension "$RELEASE_EXTENSION_ID"
fi

step "Restarting Finder so the Share menu is rebuilt"
killall Finder 2>/dev/null || true
sleep 1

report_state
cat <<EOF

Ready. Right-click a file in Finder → Share → "DroidHarbor Dev".
The release app, if enabled, appears beside it as "DroidHarbor".

To check the app half on its own, without the share sheet:
  open "droidharbor-dev://send?path=\$HOME/Desktop/some-file.png"

To watch the extension, which logs nowhere near Xcode's console:
  log stream --predicate 'process == "DroidHarborShare"' --info
EOF
