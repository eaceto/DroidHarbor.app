# dh-ffi

UniFFI boundary exposing `dh-domain` to the Swift (macOS) UI. Builds as a
static library consumed by `apps/macos` as an arm64 XCFramework via SPM.

Only macOS pays this FFI cost: the Linux UI links `dh-domain` directly as a
normal Rust crate.

## API

Currently a placeholder re-exporting `dh_domain`.

**M1:** a thin UniFFI-annotated wrapper over `dh_domain::DomainHandle`, with
async command methods mirroring `Command`, and an event callback interface
the Swift side adapts into an `AsyncStream<Event>`. Tokio runs inside this
library; Swift never sees it. Bindings are generated in CI, never committed.
