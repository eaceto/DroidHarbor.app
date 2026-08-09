#!/usr/bin/env bash
# Shared by use-dev-extension.sh and use-release-extension.sh.

BASE_ID="dev.eaceto.apps.macos.droidharbor"
DEV_ID="$BASE_ID.dev"
RELEASE_ID="$BASE_ID"
DEV_EXTENSION_ID="$DEV_ID.share"
RELEASE_EXTENSION_ID="$RELEASE_ID.share"
RELEASE_APP="/Applications/DroidHarbor.app"

LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

step() { printf '\n\033[1m%s\033[0m\n' "$1"; }
fail() { printf '\n\033[31merror:\033[0m %s\n' "$1" >&2; exit 1; }

lsregister() { "$LSREGISTER" "$@"; }

# The most recently built development app, wherever Xcode put it.
dev_app_path() {
    local found
    found=$(ls -dt "$HOME"/Library/Developer/Xcode/DerivedData/DroidHarbor-*/Build/Products/*/DroidHarborDev.app 2>/dev/null | head -1)
    [ -n "$found" ] || return 1
    printf '%s' "$found"
}

require_extension() {
    [ -d "$1/Contents/PlugIns/DroidHarborShare.appex" ] \
        || fail "$1 has no share extension inside it.
       It was built before the extension existed; build again."
}

enable_extension() {
    pluginkit -e use -i "$1" 2>/dev/null || true
}

disable_extension() {
    pluginkit -e ignore -i "$1" 2>/dev/null || true
}

# What macOS thinks right now, which is the only opinion that matters.
report_state() {
    step "State"
    printf '  %-14s %s\n' "Share menu:" "$(pluginkit -m -p com.apple.share-services -vvv 2>/dev/null \
        | grep -E "^[+!-]?\s+$BASE_ID" \
        | sed -E 's/^([+!-]?) +([^(]*).*/\2 [\1]/' | tr '\n' ' ' | sed 's/ $//')"
    for scheme in droidharbor droidharbor-dev; do
        printf '  %-14s %s\n' "$scheme://" "$(scheme_handler "$scheme")"
    done
    printf '\n  [+] offered in the Share menu   [-] or [!] hidden from it\n'
}

scheme_handler() {
    /usr/bin/swift - "$1" <<'SWIFT' 2>/dev/null || echo "unknown"
import AppKit
let scheme = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : ""
let url = URL(string: "\(scheme)://send")!
print(NSWorkspace.shared.urlForApplication(toOpen: url)?.path ?? "none")
SWIFT
}
