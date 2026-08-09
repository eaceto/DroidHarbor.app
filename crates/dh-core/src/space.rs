//! Free-space queries.
//!
//! `statvfs` is POSIX, so one implementation covers macOS and Linux and the
//! UI layers do not have to supply this themselves.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Bytes available to an unprivileged user on the filesystem holding `path`,
/// or `None` when it cannot be determined (missing path, unsupported
/// filesystem). Callers treat `None` as "skip the space check" rather than
/// failing a transfer over a diagnostic gap.
pub fn available_space(path: &Path) -> Option<u64> {
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;

    // SAFETY: `statvfs` writes into a fully-owned, zeroed struct and reads a
    // NUL-terminated path that outlives the call. The workspace denies unsafe
    // by default; this leaf call is the one exception, since the standard
    // library exposes no free-space API.
    #[allow(unsafe_code)]
    let stat = unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) != 0 {
            return None;
        }
        stat
    };

    // f_bavail is the count available to non-root users; f_frsize its unit.
    // Their widths differ per platform (u32 on macOS, u64 on Linux), so widen
    // with `as`, since a conversion would only compile on one of them.
    let blocks = stat.f_bavail as u64;
    let block_size = stat.f_frsize as u64;
    Some(blocks.saturating_mul(block_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_space_for_an_existing_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let available = available_space(dir.path()).expect("space is known");
        assert!(
            available > 0,
            "a writable filesystem should report free space"
        );
    }

    #[test]
    fn missing_paths_are_unknown_rather_than_zero() {
        assert_eq!(available_space(Path::new("/definitely/not/here")), None);
    }

    #[test]
    fn rejects_paths_containing_nul() {
        assert_eq!(available_space(Path::new("bad\0path")), None);
    }
}
