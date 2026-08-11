# DroidHarbor

Receive files from Android on your desktop using the sharing built into every
Android phone: no app to install on the phone, no cable, no cloud.

On the phone: **Share → Quick Share → tap your computer**. Full guide at
[eaceto.github.io/DroidHarbor.app](https://eaceto.github.io/DroidHarbor.app/). On the desktop: an
accept dialog with a matching 4-digit code, then the files land in a folder
you chose.

> **Current release: 0.11.8.** Both directions work on real hardware (verified
> with a Pixel 8): receiving from Android's share sheet, and sending back to
> it: files, and text such as links, addresses and phone numbers. The
> macOS app ([`apps/macos`](apps/macos)) ships as a signed, notarized
> universal DMG. Being pre-1.0, interfaces and storage formats may still
> change between releases.

> **No warranty.** DroidHarbor is provided as is, without warranty of any kind
> (GPL sections 15–16). It moves your files over a protocol that can change
> underneath it: confirm that important transfers arrived intact, and never
> treat a transferred copy as your only copy.

> **Disclaimer.** This project implements an unofficial, reverse-engineered
> protocol (Nearby Sharing, marketed as "Quick Share") and may stop working
> without notice after any Play Services update. It is not affiliated with or
> endorsed by Google. DroidHarbor exists to let a desktop interoperate with a
> protocol Android already speaks: it circumvents no access control or copy
> protection, ships no Google code, and requires no modification of the phone.

> **Trademarks.** Android, Google Play and Quick Share are trademarks of
> Google LLC, used here only to describe what this software interoperates
> with. Protocol details come from independent community work: Martichou's
> [rquickshare](https://github.com/Martichou/rquickshare), by way of
> [our fork](https://github.com/eaceto/rquickshare) (see [License](#license)).

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
| `crates/dh-qs-core` | Quick Share front door: adapts the rev-pinned GPL `rqs_lib` from [our fork](https://github.com/eaceto/rquickshare) (protocol, crypto, mDNS) to the domain seam |
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
- **0.11.8 (current)**: both directions verified on real hardware; signed and
  notarized DMG; English, Spanish, Italian and French.
- **Next**: a CI app build, folder sending, richer diagnostics.
- **Later**: Linux UI, daemon split (Finder/Share extensions), QR+HTTP
  browser fallback front door.

## License

GPL-3.0-or-later (see [LICENSE](LICENSE)). Chosen deliberately: the protocol
layer builds on [rquickshare](https://github.com/Martichou/rquickshare)'s
GPL-3.0 `rqs_lib` rather than reimplementing the reverse-engineered protocol
from scratch. Credit for that protocol work belongs upstream.

DroidHarbor builds against a **fork**,
[eaceto/rquickshare](https://github.com/eaceto/rquickshare), pinned to an exact
revision in [`Cargo.toml`](Cargo.toml). The fork carries changes not yet merged
upstream — among them a configurable device name and type, an mDNS unregister
fix, per-file progress and staged paths, error classification, consent
auto-reject, per-session staging subdirectories, a synthetic SRV hostname for
privacy, opt-in IPv6, and key zeroization. The fork is public and GPL-3.0, so
the complete corresponding source for the modified library that ships inside
every DroidHarbor binary is available at that URL; the pin makes the exact
revision unambiguous. The intent is for these patches to land upstream.
