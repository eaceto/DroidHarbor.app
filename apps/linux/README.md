# DroidHarbor Linux app

Rust + GTK4/libadwaita via [Relm4](https://relm4.org), linking `dh-domain`
directly with no FFI layer. Relm4's message loop takes `DomainHandle::subscribe()`
straight into the UI: `Command`s in, `Event`s out, the same contract the Swift
app reaches through UniFFI.

The data and domain layers are Linux-portable and CI-checked on Ubuntu, so this
app is purely UI and desktop-integration work.

## Design

**Window-first, tray optional.** GNOME has no native tray, so the app must be
fully usable without one: the window is the primary surface, consent arrives as
notification actions, and the tray is an enhancement where the desktop provides
it. This is the one structural difference from the macOS app, which is
menu-bar-first.

| Concern | Mechanism |
|---|---|
| Tray | `ksni` (StatusNotifierItem over D-Bus) — native on KDE, needs the AppIndicator extension on GNOME |
| Consent + progress | `org.freedesktop.Notifications`, with action buttons |
| Folder picker | XDG Desktop Portal |
| Reveal received file | `org.freedesktop.FileManager1` |
| Launch at login | `~/.config/autostart/droidharbor.desktop` (see the AppImage caveat below) |

The tray is never load-bearing: `ksni`'s `Watcher`/`WontShow` errors are the
runtime signal to run window-only, and any other error is a genuinely broken
session bus. On a desktop with no tray, GNOME's **Background Apps** menu — fed
by the background portal's status line — is the presence and quit affordance.

The portal answers in URIs rather than paths, so picker results are
percent-decoded before use. The 60s consent auto-reject comes from the
notification server's own expiry timer, and every outcome that is not an
explicit accept resolves to a rejection.

## Packaging

**AppImage, x86_64 and aarch64.** Two consequences drive real work:

- GTK4 does not travel by itself. The bundle must carry the gdk-pixbuf loader
  cache, compiled GSettings schemas, and an icon theme, or the app launches and
  renders nothing. `linuxdeploy-plugin-gtk` covers that, built against the
  `ubuntu:24.04` container as the glibc floor — 22.04 would reach further back
  but only offers libadwaita 1.1, without `AdwDialog` or `AdwToolbarView`.
- An AppImage is never installed, so the autostart entry points at an absolute
  path the user can move or delete — it needs a health check and a repair path.
  The app also won't appear in the launcher without `appimaged`, so Settings
  offers an explicit "install desktop entry" action. The tray is unaffected:
  `ksni` is pure D-Bus and needs no installation.

The bundle carries its shared libraries plus the gdk-pixbuf loader cache,
compiled GSettings schemas and an icon theme. Its glibc floor is **2.39**,
inherited from the 24.04 image, so it runs on Ubuntu 24.04+, Debian 13+ and
Fedora 40+ but not older.

## Building

The AppImage is built in the container, never against the host, so the glibc
floor and the bundled GTK are the same wherever the build runs:

```sh
docker run --rm -v "$PWD":/src \
    -v droidharbor-cargo-registry:/root/.cargo/registry \
    -v droidharbor-target:/target -e CARGO_TARGET_DIR=/target \
    -w /src/apps/linux droidharbor-build:24.04 ./Packaging/build-appimage.sh
```

`./release.sh <version>` wraps that, verifies the bundle, and writes
`SHA256SUMS` and the update manifest into `build/release/`.

Real transfers need a host on the same LAN segment as the phone, running
`avahi-daemon`, with UDP 5353 and the data port open. A container behind NAT
will never be discovered, because mDNS does not cross it.

## Running on macOS

The app also builds and runs natively on a Mac. Linux remains the product:
DroidHarbor on macOS ships as the SwiftUI app, and nothing here is meant for
distribution.

```sh
brew install gtk4 libadwaita adwaita-icon-theme protobuf
cargo run --bin droidharbor
```

`adwaita-icon-theme` is required, not optional. Homebrew's `gtk4` pulls only
`hicolor-icon-theme`, which is an empty base, so without it every icon in the
view switcher silently fails to render. Ubuntu ships Adwaita by definition,
which is why this only bites on the Mac.

Everything platform-specific lives behind [`src/platform/`](src/platform), which
is a `cfg` switch over two implementations:

| Capability | Linux (ships) | macOS (does not ship) |
|---|---|---|
| Consent | notification with Accept/Reject | in-window `AdwAlertDialog` |
| File and folder pickers | XDG portal, answers in URIs | `gtk4::FileDialog`, answers in paths |
| Background running | Background portal + Background Apps status | no-op; closing the window quits |
| Tray | `ksni` where a host exists | none |

**Consent is a different code path on macOS**, which is the design Linux
deliberately rejected. Consent behaviour, the 60s expiry, the tray and
background presence are only real on Linux.

## Localization

The en/es/it/fr strings already exist in the macOS
[`Localizable.xcstrings`](../macos/Resources/Localizable.xcstrings); GTK wants
gettext, so they are extractable rather than retranslatable.
