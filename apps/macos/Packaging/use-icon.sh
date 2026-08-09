#!/bin/bash
# Choose which candidate icon the app ships with.
#
#   Packaging/use-icon.sh harbor | beacon | downlink
#
# Copies the chosen .icns to Resources/AppIcon.icns, which Project.swift
# points CFBundleIconFile at. Rebuild afterwards to see it.
set -euo pipefail

cd "$(dirname "$0")/.."

name="${1:-}"
available=$(ls Packaging/icons/*.icns 2>/dev/null | xargs -n1 basename | sed 's/\.icns$//' | tr '\n' ' ')

if [ -z "$name" ] || [ ! -f "Packaging/icons/$name.icns" ]; then
    printf 'usage: Packaging/use-icon.sh <name>\navailable: %s\n' "$available" >&2
    printf '(regenerate with: swift Packaging/make-app-icons.swift)\n' >&2
    exit 1
fi

mkdir -p Resources
cp "Packaging/icons/$name.icns" Resources/AppIcon.icns
printf 'using the "%s" icon, rebuild to see it\n' "$name"

# The Dock and Finder cache icons aggressively; a rebuilt app with the same
# bundle id often keeps showing the old one until the cache is nudged.
touch Resources/AppIcon.icns
