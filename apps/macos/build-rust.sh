#!/bin/bash
# Builds the Rust static library and regenerates the UniFFI Swift bindings.
# Runs as an Xcode pre-build phase; also usable standalone.
#
# Produces a (possibly universal) static archive at
# target/universal/libdh_ffi.a, which the app links by absolute path. Linking
# the static archive, never the .dylib cargo also emits, keeps the Rust code
# inside the app binary, so a shared build runs on machines that have no
# checkout of this repo.
set -euo pipefail

cd "$(dirname "$0")/../.."

# Xcode's build environment has a minimal PATH; cargo and protoc live here.
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

# Build for whatever Xcode is building (ARCHS), or both when run by hand.
ARCHS_TO_BUILD="${ARCHS:-arm64 x86_64}"

targets=()
for arch in $ARCHS_TO_BUILD; do
    case "$arch" in
        arm64|arm64e) targets+=("aarch64-apple-darwin") ;;
        x86_64) targets+=("x86_64-apple-darwin") ;;
        *) echo "warning: unknown arch '$arch', skipping" >&2 ;;
    esac
done
[ ${#targets[@]} -gt 0 ] || targets=("aarch64-apple-darwin")

host_target="$(rustc -vV | awk '/^host:/{print $2}')"

# The bindings generator introspects a dynamic library, so the host target is
# always built even when cross-compiling for release.
build_targets=("${targets[@]}")
case " ${build_targets[*]} " in
    *" $host_target "*) ;;
    *) build_targets+=("$host_target") ;;
esac

for target in "${build_targets[@]}"; do
    rustup target add "$target" >/dev/null 2>&1 || true
    cargo build -p dh-ffi --release --target "$target"
done

archives=()
for target in "${targets[@]}"; do
    archives+=("target/$target/release/libdh_ffi.a")
done
mkdir -p target/universal
lipo -create "${archives[@]}" -output target/universal/libdh_ffi.a

cargo run -p dh-ffi --bin uniffi-bindgen --release --quiet -- \
    generate \
    --library "target/$host_target/release/libdh_ffi.dylib" \
    --language swift \
    --out-dir apps/macos/Generated
