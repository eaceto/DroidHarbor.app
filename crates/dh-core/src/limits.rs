//! Transfer limits (spec §10). Pure policy, no syscalls.
//!
//! Free-space is passed in by the caller because querying it is a platform
//! concern (Swift uses `URLResourceKey`, Linux will use `statvfs`); keeping it
//! injected keeps this crate trivially testable.

use crate::CoreError;

#[derive(Debug, Clone)]
pub struct Limits {
    /// Maximum number of files in a single transfer.
    pub max_files: u64,
    /// Bytes of free disk space that must remain after the transfer.
    pub free_space_headroom: u64,
    /// Seconds to wait for the user to accept before auto-rejecting.
    pub accept_timeout_secs: u64,
    /// Minutes of idle advertising before receiving auto-disables (0 = never).
    pub auto_off_minutes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_files: 500,
            free_space_headroom: 2 * 1024 * 1024 * 1024, // 2 GiB
            accept_timeout_secs: 60,
            auto_off_minutes: 10,
        }
    }
}

impl Limits {
    /// Validate an incoming introduction before it is shown to the user.
    ///
    /// `available_bytes` is the free space at the destination, if known;
    /// `None` skips the space check (the caller could not determine it).
    pub fn check_introduction(
        &self,
        file_count: u64,
        total_bytes: u64,
        available_bytes: Option<u64>,
    ) -> Result<(), CoreError> {
        if file_count == 0 {
            return Err(CoreError::EmptyTransfer);
        }
        if file_count > self.max_files {
            return Err(CoreError::TooManyFiles {
                count: file_count,
                limit: self.max_files,
            });
        }
        if let Some(available) = available_bytes {
            let needed = total_bytes.saturating_add(self.free_space_headroom);
            if needed > available {
                return Err(CoreError::InsufficientSpace { total_bytes });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_reasonable_introduction() {
        let limits = Limits::default();
        assert!(limits
            .check_introduction(3, 100 * 1024 * 1024, Some(100 * 1024 * 1024 * 1024))
            .is_ok());
    }

    #[test]
    fn rejects_empty_transfer() {
        assert!(matches!(
            Limits::default().check_introduction(0, 0, None),
            Err(CoreError::EmptyTransfer)
        ));
    }

    #[test]
    fn rejects_too_many_files() {
        assert!(matches!(
            Limits::default().check_introduction(501, 1, None),
            Err(CoreError::TooManyFiles { .. })
        ));
    }

    #[test]
    fn rejects_when_space_would_drop_below_headroom() {
        let limits = Limits::default();
        // 1 GiB transfer into 2.5 GiB free with 2 GiB headroom → reject.
        let result = limits.check_introduction(1, 1024 * 1024 * 1024, Some(2_684_354_560));
        assert!(matches!(result, Err(CoreError::InsufficientSpace { .. })));
    }

    #[test]
    fn unknown_space_skips_the_check() {
        assert!(Limits::default()
            .check_introduction(1, u64::MAX / 2, None)
            .is_ok());
    }
}
