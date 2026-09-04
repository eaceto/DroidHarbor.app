# Releasing DroidHarbor

One tag releases both apps. Pushing `vX.Y.Z` triggers the Linux workflow,
which builds version-stamped AppImages for both architectures and attaches
them — with their update manifest — to the release. The macOS DMG is built,
signed and notarized locally (the Developer ID key lives on one machine),
then uploaded to the same release. The release is published only when both
platforms' files are on it, because both apps read their update manifests
from `releases/latest/download/…` and "latest" must never be half-empty.

This is the exact sequence that shipped 1.0.0.

## One-time setup (the machine that builds the DMG)

1. **Developer ID Application certificate** in the login keychain:
   Xcode → Settings → Accounts → your Apple ID → Manage Certificates → **+**
   → Developer ID Application. (Needs the paid team; an "Apple Development"
   certificate cannot sign apps for other people's Macs.)
2. **notarytool credentials**, stored once as a keychain profile:

   ```sh
   xcrun notarytool store-credentials droidharbor \
     --apple-id <your-apple-id> --team-id 2U378HJ7FG \
     --password <app-specific-password>
   ```

   App-specific passwords come from <https://account.apple.com> →
   Sign-In and Security.
3. **Tools on PATH**: `tuist`, `cargo`, `protoc`, `gh` (authenticated:
   `gh auth status`).

Check all of it at any time with:

```sh
cd apps/macos && ./release.sh --check
```

## Cutting a release

### 1. Start from the merged `main`

```sh
git checkout main && git pull
git status          # must be clean; release.sh builds whatever is here
```

Pick the version, `X.Y.Z` only — the scripts refuse anything else, and the
update checkers compare it numerically.

### 2. Tag, then immediately draft the release

```sh
git tag v1.1.0
git push origin v1.1.0
gh release create v1.1.0 --draft --title "DroidHarbor 1.1.0"
```

Order matters, twice:

- The **tag push** is what fires `.github/workflows/linux-release.yml`.
  A draft release alone does not create the tag ref, so drafting first
  would build nothing.
- Creating the **draft right after** means the workflow attaches its
  assets to your draft instead of auto-publishing a release of its own.
  (The build takes several minutes; there is no race to worry about.)

While it runs, the workflow: injects the tag version into
`apps/linux/Cargo.toml` (so the About dialog and the update comparison
report the release's own number), builds x86_64 and aarch64 AppImages in
the same ubuntu:24.04 container used locally, verifies the bundles carry
GTK's runtime furniture, and — in a final job — merges both legs into
`updates-linux.json` and attaches everything to the release.

### 3. Meanwhile, build the macOS DMG locally

```sh
cd apps/macos
./release.sh 1.1.0
```

Expect 10–20 minutes, most of it Apple's notarization queue. The script
archives universal (arm64 + x86_64) on the **release channel** — the only
path that produces the shipping bundle identity — signs with the Developer
ID, notarizes and staples both the app and the DMG, and writes
`build/updates.json` next to `build/DroidHarbor.dmg`.

Two dialogs may appear on the way:

- Keychain: "codesign wants to access key…" → **Always Allow**.
- Finder Automation for the DMG window layout → allowing it styles the
  disk image; refusing only leaves it unstyled.

### 4. Attach the macOS files, confirm Linux, publish

```sh
gh release upload v1.1.0 build/DroidHarbor.dmg build/updates.json

gh run list --workflow=linux-release.yml --limit 1    # wait for ✓
gh release view v1.1.0
```

The release must list **seven assets** before publishing:

| Asset | Produced by |
|---|---|
| `DroidHarbor.dmg` | `apps/macos/release.sh` (local) |
| `updates.json` | `apps/macos/release.sh` (local) |
| `DroidHarbor-X.Y.Z-x86_64.AppImage` | CI |
| `DroidHarbor-X.Y.Z-aarch64.AppImage` | CI |
| `SHA256SUMS-x86_64.txt` | CI |
| `SHA256SUMS-aarch64.txt` | CI |
| `updates-linux.json` | CI (manifest job) |

Then:

```sh
gh release edit v1.1.0 --draft=false
```

Publishing is the moment `latest` flips: from here, both apps' in-app
update checks will offer the new version.

### 5. Post-release housekeeping

All hand-maintained; grep for the previous version if unsure:

- `README.md` — the "Current release" line and, if reality changed, the
  roadmap.
- `docs/index.html` — the **Version X.Y.Z** span in the hero meta line.
- `apps/linux/Cargo.toml` — bump `version` to match (CI injects the tag at
  release time, but a local `cargo run` build's About dialog shows the
  checked-in value). Run `cargo check` in `apps/linux` afterwards so
  `Cargo.lock` follows.
- `docs/images/*.png` — retake any screenshot whose UI changed this
  release (see the figure list in `docs/index.html`; dark mode, ⇧⌘4 then
  Space so the window shadow comes along).

Commit these to `main`; pushing also updates the GitHub Pages site.

### 6. Verify

- On a Mac running the previous version: Settings → **Check for
  Updates…** should offer the new one, and About should show a Download
  button.
- On Linux (or the VM): Settings → **Check for updates** should report
  the new version.
- Mount the DMG on a Mac that never saw the app: it must open with no
  Gatekeeper warning (that's the stapled ticket doing its job).

## Notes and troubleshooting

- **`errSecInternalComponent` during export** — codesign could not reach
  the signing key. Re-run and click "Always Allow", or grant it durably:

  ```sh
  security set-key-partition-list -S apple-tool:,apple:,codesign: \
    -s -k <login-password> ~/Library/Keychains/login.keychain-db
  ```

- **Local Linux builds** — `apps/linux/release.sh X.Y.Z` produces the same
  AppImage + manifest on a developer machine (one architecture per run;
  the script explains how to merge the second). Normally unnecessary: CI
  covers both arches.
- **`workflow_dispatch` runs** of the Linux workflow build AppImages
  versioned `0.0.0-<sha>` and attach nothing to any release — they exist
  to prove packaging changes before a tag.
- **A broken tag** — if the workflow fails or the DMG build does, fix on
  `main`, delete the tag and draft (`gh release delete v1.1.0`,
  `git push origin :refs/tags/v1.1.0`, `git tag -d v1.1.0`) and start
  over. Never reuse a tag that was ever published: "latest" downloads are
  cached by name.
- **Version discipline** — the tag is the single source of truth at build
  time. The checked-in numbers (`apps/linux/Cargo.toml`, README, docs) are
  documentation that follows it in step 5.
