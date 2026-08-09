# DroidHarbor

Receive files from Android on your desktop using the sharing built into every
Android phone: no app to install on the phone, no cable, no cloud.

On the phone: **Share → Quick Share → tap your computer**. Full guide at
[eaceto.github.io/DroidHarbor.app](https://eaceto.github.io/DroidHarbor.app/). On the desktop: an
accept dialog with a matching 4-digit code, then the files land in a folder
you chose.

> **Status: Alpha.** Both directions work on real hardware (verified with a
> Pixel 8): receiving from Android's share sheet, and sending back to it:
> files, and text such as links, addresses and phone numbers. The
> macOS app ([`apps/macos`](apps/macos)) ships as a signed, notarized
> universal DMG. Interfaces and storage formats may still change.

> **Disclaimer.** This project implements an unofficial, reverse-engineered
> protocol (Nearby Sharing, marketed as "Quick Share") and may stop working
> without notice after any Play Services update. It is not affiliated with or
> endorsed by Google.

## Architecture

Three layers (spec §5). The two lower layers are shared Rust with one source
tree for macOS and Linux; UIs are fully native per platform.

```text
┌──────────────────────────────────────────────────────────────┐
│ UI (native per OS)                                           │
│  macOS: Swift/SwiftUI via UniFFI     Linux: Rust + GTK4      │
├──────────────────────────────────────────────────────────────┤
│ DOMAIN  dh-domain: commands in, events out; owns sessions,   │
│         policy, limits; the only API a UI ever sees          │
├──────────────────────────────────────────────────────────────┤
│ DATA    dh-qs-core: Quick Share front door over rqs_lib      │
│         dh-core: staging · atomic finalize · name safety     │
└──────────────────────────────────────────────────────────────┘
```

| Crate / dir | Role |
|---|---|
| `crates/dh-core` | Filesystem safety: sanitization, limits, atomic finalization (implemented + tested) |
| `crates/dh-qs-core` | Quick Share front door: adapts the rev-pinned GPL `rqs_lib` (protocol, crypto, mDNS) to the domain seam |
| `crates/dh-domain` | Session orchestration; the command/event API (implemented + tested) |
| `crates/dh-ffi` | UniFFI surface for Swift (M1) |
| `integration-tests` | Headless tests driving `dh-domain` exactly as a UI would |
| `apps/macos` | DroidHarbor: SwiftUI menu-bar app (M1) |
| `apps/linux` | GTK4 app (future development) |

Building requires `protoc` (`brew install protobuf` / `apt install
protobuf-compiler`) for `rqs_lib`'s vendored protocol definitions.

## Building

```sh
cargo test --workspace     # data + domain layers, all platforms
cargo clippy --workspace --all-targets -- -D warnings
```

Requires stable Rust. The macOS app (M1) will additionally require Xcode.

## Roadmap

- **M0 (protocol spike)**: wire the `rqs_lib` front door to the domain and
  receive one file from a real phone via a headless CLI. Go/no-go gate for
  the whole approach.
- **M1 (trusted receiver)**: UniFFI bindings, SwiftUI menu-bar app, accept
  dialog with token, notifications, Reveal in Finder.
- **M2 (hardened + packaged)**: limits/timeouts everywhere, loopback CI
  sender, fuzzing, firewall hints, signed + notarized DMG.
- **Alpha (now)**: both directions verified on real hardware; signed and
  notarized DMG; English, Spanish, Italian and French.
- **Next**: a CI app build, folder sending, richer diagnostics.
- **Later**: Linux UI, daemon split (Finder/Share extensions), QR+HTTP
  browser fallback front door.

## License

GPL-3.0-or-later (see [LICENSE](LICENSE)). Chosen deliberately: the protocol
layer builds on [rquickshare](https://github.com/Martichou/rquickshare)'s
GPL-3.0 `rqs_lib` rather than reimplementing the reverse-engineered protocol
from scratch.
