//! User-configurable settings held by the domain layer.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Settings {
    /// Name shown in the phone's share sheet.
    pub device_name: String,
    /// Folder where finalized files are placed.
    pub destination: PathBuf,
    /// Application-owned staging directory for partial files.
    pub staging_dir: PathBuf,
}

impl Settings {
    pub fn new(device_name: impl Into<String>, destination: PathBuf, staging_dir: PathBuf) -> Self {
        Self {
            device_name: device_name.into(),
            destination,
            staging_dir,
        }
    }
}
