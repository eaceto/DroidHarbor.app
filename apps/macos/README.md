# DroidHarbor macOS app (M1)

SwiftUI menu-bar app over `dh-domain`, via the UniFFI bindings from
`crates/dh-ffi`. All native integration lives here: the accept dialog with
the 4-digit code, notifications, the destination picker, Reveal in Finder,
and the `com.apple.quarantine` xattr on received files.

## Building

Requires Xcode, Rust, `protoc` (`brew install protobuf`) and
[Tuist](https://tuist.io). The project files and `Generated/` bindings are
not committed; generate them:

```sh
cd apps/macos
./build-rust.sh        # Rust staticlib + UniFFI Swift bindings
tuist generate         # creates DroidHarbor.xcworkspace and opens it
# or: xcodebuild -workspace DroidHarbor.xcworkspace -scheme DroidHarbor build
```

An Xcode pre-build phase re-runs `build-rust.sh`, so Rust changes are picked
up on every app build.

## Two channels

`tuist generate` builds **DroidHarbor Dev**, a separate app from the one that
ships. Only `release.sh` sets `TUIST_DH_CHANNEL=release`, so a normal build
can never claim the shipping identity.

| | development | release |
| --- | --- | --- |
| Bundle id | `dev.eaceto.apps.macos.droidharbor.dev` | `dev.eaceto.apps.macos.droidharbor` |
| App | `DroidHarbor Dev.app` | `DroidHarbor.app` |
| URL scheme | `droidharbor-dev` | `droidharbor` |
| History and state | `~/.droidharbor-dev` | `~/.droidharbor` |
| Received files | `~/Downloads/droidharbor-dev` | `~/Downloads/droidharbor` |
| Name on the phone | hostname + " Dev" | hostname |
| Preferences | follow the bundle id, so they never mix | |

This exists because macOS resolves URL schemes and share extensions per
bundle identifier. When two copies shared one, LaunchServices
picked whichever it considered canonical, which was usually the installed
copy rather than the one being tested: the share extension opened a URL that
went to the wrong app, and testing wrote into the real history.

The Swift side never hardcodes any of this. `Project.swift` writes the values
into Info.plist and `AppInfo` reads them back, so adding a channel is a change
in one file. `PRODUCT_MODULE_NAME` stays `DroidHarbor` in both, so
`@testable import DroidHarbor` works either way.

Both copies can be installed and run at once.

## Layout

Sources are grouped by role:

| Group | Contents |
|---|---|
| `App/` | `DroidHarborApp`, the delegate owning every AppKit surface (status item, popover, window lifecycle and activation policy, notifications) and `MainWindowController`. |
| `Model/` | `AppState` (the single source of truth: owns `DHService`, turns domain events into published state), `AppState+Notifications`, `Transfer` (value types), `History` (persistence). |
| `Views/` | `MenuView` (menu-bar popover), `MainWindow` + `TransfersView` / `SendView` / `SettingsView`, `OnboardingView`. |
| `Platform/` | Native odds and ends: `StatusDropView` (click + drop overlay), `DropHintPanel`, `Quarantine`, `LaunchAtLogin`, `Format`, `DHTypes`. |
| `Resources/` | `Localizable.xcstrings`: English, Spanish, Italian and French. |

`Project.swift` is the Tuist manifest; `build-rust.sh` builds the universal
static library and regenerates the UniFFI bindings into `Generated/`.

### Adding UI text

SwiftUI literals are localization keys automatically. Anything reaching
AppKit (alerts, notifications, panel labels) needs `String(localized:)`, and
custom views should take `LocalizedStringKey` rather than `String` for
static labels. After adding strings, build once and run the catalog update
described in the repository README so the four languages stay complete.

## Tests

```sh
xcodebuild test -workspace DroidHarbor.xcworkspace -scheme DroidHarbor \
  -destination 'platform=macOS'
```

`Tests/` covers the layer the views sit on: domain-event handling in
`AppState` (transfers, trust, discovery, text payloads, history), the pure
formatting and version-comparison helpers, and regression tests for the two
bugs that lost or broke received files: the notification preview must copy
rather than consume the file, and the quarantine flags must not include the
sandbox bit.

`AppState(defaults:startService:)` is the seam: tests pass a scratch
`UserDefaults` suite and skip the transfer service, so no sockets, mDNS or
phone are involved. The app hosts the test bundle, so its delegate returns
early when `XCTestCase` is present.

## Icon

Three candidates live in `Packaging/icons`, rendered from
`Packaging/make-app-icons.swift` (vector code, re-rendered at every size so
16pt stays legible). `Packaging/preview.png`-style sheet:
`Packaging/icons/preview.png`.

```sh
swift Packaging/make-app-icons.swift   # regenerate after editing the artwork
Packaging/use-icon.sh harbor           # harbor | beacon | downlink
```

`use-icon.sh` copies the chosen `.icns` to `Resources/AppIcon.icns`, which
`CFBundleIconFile` points at. The Dock caches icons hard; if a rebuild still
shows the old one, `killall Dock`.

## Releasing

```sh
cd apps/macos
./release.sh --check                  # what is set up, what is missing
./release.sh 1.2.0                    # signed, notarized, stapled DMG
DH_SKIP_NOTARIZE=1 ./release.sh 1.2.0 # local build, no notarization
```

The version argument is the marketing version; the build number is derived
from it so it always increases. `release.sh` also writes
`build/updates.json`; upload it beside the DMG and the in-app update check
will notice the release. `DH_DOWNLOAD_BASE` overrides the download URL it points at.

One-time setup:

1. **Developer ID Application certificate**: Xcode → Settings → Accounts →
   your Apple ID → Manage Certificates → **+** → *Developer ID Application*.
   Requires the paid Individual/Organization team; the Apple Development
   certificate used for local builds cannot sign apps for other machines.
2. **notarytool credentials**, stored once in the keychain:

   ```sh
   xcrun notarytool store-credentials droidharbor \
     --apple-id <your-apple-id> --team-id 2U378HJ7FG \
     --password <app-specific-password>
   ```

   App-specific passwords come from <https://account.apple.com> → Sign-In and
   Security → App-Specific Passwords.

`release.sh` archives universal (Apple Silicon + Intel), exports with
Developer ID, verifies the Rust core is *statically* linked (a dylib
reference to this checkout would break the app everywhere else), then
notarizes and staples **both the app and the DMG**. Stapling the app matters:
a ticket on the disk image alone is left behind when the app is dragged out,
and the first launch would then need internet to pass Gatekeeper.

`./release.sh --check` reports which prerequisites are missing before you
commit to a full build.

If the export ever fails with `errSecInternalComponent`, codesign could not
reach the signing key: macOS asks for permission the first time and the
export fails if that dialog is dismissed. Grant it permanently with:

```sh
security set-key-partition-list -S apple-tool:,apple:,codesign: \
  -s -k <your-login-password> ~/Library/Keychains/login.keychain-db
```

Day-to-day signing is automatic under team 2U378HJ7FG.
