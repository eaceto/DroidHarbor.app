//! Noticing a newer release.
//!
//! Deliberately not an auto-updater: it reads the same small JSON manifest the
//! macOS app checks, published beside each release, and points at the download.
//! Installing stays the user's business — an AppImage lives wherever they put
//! it, and replacing it behind their back would be presumptuous.

use serde::Deserialize;

/// Read from the release assets rather than a file in the repository, so
/// "latest" always resolves to whichever release is current and the manifest
/// cannot describe a version the download does not match.
///
/// Deliberately **not** the macOS app's `updates.json`: that one names a DMG,
/// and pointing a Linux user at it is worse than saying nothing. The two
/// platforms publish separate manifests into the same release, so neither
/// release process has to know about the other.
pub const MANIFEST: &str =
    "https://github.com/eaceto/DroidHarbor.app/releases/latest/download/updates-linux.json";

/// One downloadable build.
#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub url: String,
    /// Published so a download can be checked; the app only reports it.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// The shape `apps/linux/release.sh` writes.
#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    /// Keyed by architecture, as `std::env::consts::ARCH` spells it.
    #[serde(default)]
    artifacts: std::collections::BTreeMap<String, Artifact>,
    /// A single URL, for a manifest that predates per-architecture builds.
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Available {
    pub version: String,
    pub url: String,
    pub notes: Option<String>,
    pub sha256: Option<String>,
}

/// Fetch the manifest and report a release newer than `current`.
///
/// Blocking, so it belongs on a worker thread. Every failure — offline, a
/// rewritten URL, malformed JSON — is `None`: an update check is a courtesy and
/// must never interrupt anything.
pub fn check(current: &str, manifest: &str) -> Option<Available> {
    let response: Manifest = ureq::get(manifest)
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;

    select(&response, current, std::env::consts::ARCH)
}

/// Pick the build for `arch`, if the manifest describes one and it is newer.
///
/// A release that ships no build for this architecture reports nothing: an
/// update the user cannot run is not an update.
fn select(manifest: &Manifest, current: &str, arch: &str) -> Option<Available> {
    if !is_newer(&manifest.version, current) {
        return None;
    }
    let (url, sha256) = match manifest.artifacts.get(arch) {
        Some(artifact) => (artifact.url.clone(), artifact.sha256.clone()),
        None => (manifest.url.clone()?, None),
    };
    Some(Available {
        version: manifest.version.clone(),
        url,
        notes: manifest.notes.clone(),
        sha256,
    })
}

/// Compare dotted numeric versions, so 1.10.0 beats 1.9.3.
///
/// Anything non-numeric is treated as zero rather than rejected: a manifest
/// with a tag like "1.2.0-beta" should still be understood as 1.2.0.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(version: &str) -> Vec<u32> {
        version
            .trim_start_matches('v')
            .split(['.', '-', '+'])
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    }

    let (candidate, current) = (parts(candidate), parts(current));
    for index in 0..candidate.len().max(current.len()) {
        let left = candidate.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(json: &str) -> Manifest {
        serde_json::from_str(json).expect("manifest parses")
    }

    const TWO_ARCHES: &str = r#"{
        "version": "1.2.0",
        "notes": "Faster transfers",
        "artifacts": {
            "x86_64":  { "url": "https://example.test/DroidHarbor-1.2.0-x86_64.AppImage",  "sha256": "aa" },
            "aarch64": { "url": "https://example.test/DroidHarbor-1.2.0-aarch64.AppImage", "sha256": "bb" }
        }
    }"#;

    #[test]
    fn picks_the_build_for_this_architecture() {
        let found = select(&manifest(TWO_ARCHES), "1.1.0", "aarch64").expect("an update");
        assert!(found.url.ends_with("aarch64.AppImage"));
        assert_eq!(found.sha256.as_deref(), Some("bb"));
        assert_eq!(found.notes.as_deref(), Some("Faster transfers"));
    }

    #[test]
    fn offers_nothing_when_this_architecture_is_absent() {
        // A release built only for x86_64 must not be offered to an arm user.
        let only_intel = manifest(
            r#"{"version":"1.2.0","artifacts":{"x86_64":{"url":"https://example.test/x.AppImage"}}}"#,
        );
        assert!(select(&only_intel, "1.1.0", "aarch64").is_none());
    }

    #[test]
    fn falls_back_to_a_single_url() {
        let flat = manifest(r#"{"version":"1.2.0","url":"https://example.test/any.AppImage"}"#);
        let found = select(&flat, "1.1.0", "aarch64").expect("an update");
        assert_eq!(found.url, "https://example.test/any.AppImage");
        assert_eq!(found.sha256, None);
    }

    #[test]
    fn says_nothing_when_current_is_up_to_date() {
        assert!(select(&manifest(TWO_ARCHES), "1.2.0", "aarch64").is_none());
        assert!(select(&manifest(TWO_ARCHES), "1.3.0", "aarch64").is_none());
    }

    #[test]
    fn numeric_ordering_not_lexical() {
        assert!(is_newer("1.10.0", "1.9.3"), "10 is above 9, not below it");
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.2.3", "1.2.3"), "the same version is not newer");
        assert!(!is_newer("1.2.3", "1.2.4"));
    }

    #[test]
    fn shorter_versions_are_padded() {
        assert!(is_newer("1.3", "1.2.9"));
        assert!(!is_newer("1.2", "1.2.0"));
        assert!(is_newer("1.2.1", "1.2"));
    }

    #[test]
    fn tolerates_prefixes_and_suffixes() {
        assert!(is_newer("v1.3.0", "1.2.0"));
        // A pre-release of the next version still reads as that version.
        assert!(is_newer("1.3.0-beta", "1.2.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }
}
