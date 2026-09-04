//! Bringing up `dh-domain` and its Quick Share front door.
//!
//! Same sequence the CLI uses (`apps/cli/src/main.rs`): build the front door,
//! hand its channels to the domain engine, keep both task handles so shutdown
//! can wait on them. Linux links the domain directly, so unlike the Swift app
//! there is no FFI in between — `Command`s go in and `Event`s come out as
//! ordinary Rust types.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::task::JoinHandle;

use dh_core::limits::Limits;
use dh_domain::{DomainConfig, DomainHandle, Settings};
use dh_qs_core::QuickShareConfig;

/// A running engine plus the tasks to wait on when stopping.
pub struct Backend {
    pub handle: DomainHandle,
    engine_task: JoinHandle<()>,
    frontdoor_task: JoinHandle<()>,
}

impl Backend {
    /// Stop advertising, end both tasks, and give them a moment to finish.
    /// Bounded because a wedged front door must not hang application exit.
    pub async fn shutdown(self) {
        self.handle.send(dh_domain::Command::Shutdown).await.ok();
        let grace = Duration::from_secs(5);
        let _ = tokio::time::timeout(grace, self.engine_task).await;
        let _ = tokio::time::timeout(grace, self.frontdoor_task).await;
    }
}

/// Where received files land by default.
///
/// Matches the macOS app's `~/Downloads/droidharbor`, but honours
/// `XDG_DOWNLOAD_DIR` first so a localized or relocated Downloads folder is
/// respected rather than a second English one being created beside it.
pub fn default_destination() -> PathBuf {
    let downloads = std::env::var_os("XDG_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Downloads")))
        .unwrap_or_else(std::env::temp_dir);
    downloads.join("droidharbor")
}

/// The name shown in the phone's share sheet.
pub fn default_device_name() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "DroidHarbor".into())
}

/// Start the front door and the domain engine.
///
/// Staging sits inside the destination so finalization stays a rename within
/// one filesystem; `dh-core` depends on that for atomicity.
pub async fn start(device_name: String, destination: PathBuf) -> Result<Backend> {
    let destination = std::path::absolute(&destination)
        .with_context(|| format!("cannot resolve destination {}", destination.display()))?;
    let staging = destination.join(".dh-staging");
    std::fs::create_dir_all(&destination)
        .with_context(|| format!("cannot create {}", destination.display()))?;

    let limits = Limits::default();
    let (channels, frontdoor_task) = dh_qs_core::spawn(QuickShareConfig {
        staging_dir: staging.clone(),
        port: None,
        device_name: Some(device_name.clone()),
        consent_timeout: Duration::from_secs(limits.accept_timeout_secs),
    })
    .await
    .context("could not start the Quick Share front door")?;

    let config = DomainConfig {
        settings: Settings::new(device_name, destination, staging),
        limits,
    };
    let (handle, engine_task) = dh_domain::spawn(config, channels);

    Ok(Backend {
        handle,
        engine_task,
        frontdoor_task,
    })
}
