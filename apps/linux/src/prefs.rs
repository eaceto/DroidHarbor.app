//! Preferences that survive a restart.
//!
//! The macOS app keeps these in `UserDefaults`; the Linux equivalent is a small
//! JSON file under the XDG config directory. Only choices the user made are
//! stored — anything derivable (the hostname, the default folder) is computed
//! at startup instead, so a machine rename is picked up rather than frozen in.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// `None` means "use the hostname".
    pub device_name: Option<String>,
    /// `None` means the default `~/Downloads/droidharbor`.
    pub destination: Option<String>,
    /// Senders accepted without showing the confirmation code. Matched by the
    /// name a device announces, which the device chooses for itself — so this
    /// is a convenience, not an identity check.
    pub trusted_devices: Vec<String>,
    /// Turn receiving off after this many idle minutes; 0 disables the timer.
    pub auto_off_minutes: u64,
    pub launch_at_login: bool,
    /// Whether notifications ask for a sound. Defaults off: a file arriving is
    /// not urgent, and the visual notification already says so.
    pub play_sounds: bool,
    /// Cleared to show the introduction again.
    pub onboarded: bool,
}

impl Prefs {
    pub fn trusts(&self, device: &str) -> bool {
        self.trusted_devices
            .iter()
            .any(|trusted| trusted.eq_ignore_ascii_case(device))
    }

    pub fn trust(&mut self, device: &str) {
        if !self.trusts(device) {
            self.trusted_devices.push(device.to_string());
        }
    }

    pub fn revoke(&mut self, device: &str) {
        self.trusted_devices
            .retain(|trusted| !trusted.eq_ignore_ascii_case(device));
    }
}

/// `$XDG_CONFIG_HOME/droidharbor/settings.json`, falling back to `~/.config`.
pub fn file_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("droidharbor").join("settings.json")
}

/// Defaults on any failure. Preferences are conveniences; refusing to start
/// because one could not be read would be the wrong trade.
pub fn load(path: &Path) -> Prefs {
    let Ok(data) = std::fs::read_to_string(path) else {
        return Prefs::default();
    };
    serde_json::from_str(&data).unwrap_or_else(|error| {
        tracing::error!(%error, "preferences could not be read; using defaults");
        Prefs::default()
    })
}

pub fn save(path: &Path, prefs: &Prefs) {
    let Some(directory) = path.parent() else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(directory) {
        tracing::error!(%error, "could not create the config directory");
        return;
    }
    let Ok(json) = serde_json::to_string_pretty(prefs) else {
        return;
    };
    let temporary = path.with_extension("json.tmp");
    if std::fs::write(&temporary, json).is_ok() {
        if let Err(error) = std::fs::rename(&temporary, path) {
            tracing::error!(%error, "could not replace preferences");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_is_case_insensitive_and_idempotent() {
        let mut prefs = Prefs::default();
        prefs.trust("Pixel 8");
        prefs.trust("pixel 8");
        assert_eq!(prefs.trusted_devices.len(), 1);
        assert!(prefs.trusts("PIXEL 8"));

        prefs.revoke("pixel 8");
        assert!(!prefs.trusts("Pixel 8"));
        assert!(prefs.trusted_devices.is_empty());
    }

    #[test]
    fn unknown_and_missing_fields_fall_back() {
        // A file from a newer build, or one hand-edited badly.
        let prefs: Prefs = serde_json::from_str(r#"{"auto_off_minutes": 30}"#).unwrap();
        assert_eq!(prefs.auto_off_minutes, 30);
        assert_eq!(prefs.device_name, None);
        assert!(!prefs.launch_at_login);
    }

    #[test]
    fn round_trips() {
        let prefs = Prefs {
            device_name: Some("Studio".into()),
            destination: Some("/data/inbox".into()),
            trusted_devices: vec!["Pixel 8".into()],
            auto_off_minutes: 10,
            launch_at_login: true,
            play_sounds: true,
            onboarded: true,
        };
        let json = serde_json::to_string(&prefs).unwrap();
        assert_eq!(serde_json::from_str::<Prefs>(&json).unwrap(), prefs);
    }
}
