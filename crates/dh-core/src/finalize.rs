//! Atomic placement of completed files (spec §14 heritage).
//!
//! A received file lives in an application-owned staging directory until every
//! byte is on disk. Finalization then makes it visible in the destination
//! folder in one step: claim a collision-free name, `rename(2)` onto it, and
//! fsync the directory. If staging and destination are on different
//! filesystems (EXDEV), fall back to copy + sync + rename *within* the
//! destination filesystem so a partially copied file is never visible under
//! its final name.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::paths::{sanitize_file_name, split_extension};
use crate::CoreError;

/// Move a fully-received staged file into `dest_dir` under (a collision-free
/// variant of) `desired_name`. Returns the final path.
///
/// Never overwrites: collisions get a ` (n)` suffix before the extension.
pub fn finalize_file(
    staged: &Path,
    dest_dir: &Path,
    desired_name: &str,
) -> Result<PathBuf, CoreError> {
    let name = sanitize_file_name(desired_name);
    fs::create_dir_all(dest_dir)?;

    // Ensure the staged bytes are durable before they become visible.
    File::open(staged)?.sync_all()?;

    let target = claim_target(dest_dir, &name)?;

    match fs::rename(staged, &target) {
        Ok(()) => {}
        Err(err) if is_cross_device(&err) => {
            if let Err(copy_err) = copy_into_place(staged, dest_dir, &target) {
                let _ = fs::remove_file(&target);
                return Err(copy_err);
            }
            fs::remove_file(staged)?;
        }
        Err(err) => {
            let _ = fs::remove_file(&target);
            return Err(err.into());
        }
    }

    sync_dir(dest_dir)?;
    Ok(target)
}

/// Reserve a collision-free path in `dir` by atomically creating an empty
/// placeholder (`create_new`), so two concurrent finalizations cannot claim
/// the same name. The placeholder is replaced by the subsequent rename.
fn claim_target(dir: &Path, name: &str) -> Result<PathBuf, CoreError> {
    let (stem, ext) = split_extension(name);
    for n in 0u32.. {
        let candidate = if n == 0 {
            dir.join(name)
        } else {
            dir.join(format!("{stem} ({n}){ext}"))
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => return Ok(candidate),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!("u32 collision space exhausted");
}

/// Cross-filesystem fallback: copy into a temporary file *inside* the
/// destination directory, sync it, then rename onto the claimed target.
fn copy_into_place(staged: &Path, dest_dir: &Path, target: &Path) -> Result<(), CoreError> {
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unnamed".into());
    let tmp = dest_dir.join(format!(".aft-partial-{file_name}"));

    let result = (|| -> Result<(), CoreError> {
        fs::copy(staged, &tmp)?;
        File::open(&tmp)?.sync_all()?;
        fs::rename(&tmp, target)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn is_cross_device(err: &io::Error) -> bool {
    err.raw_os_error() == Some(libc::EXDEV)
}

/// Fsync a directory so the rename itself is durable. Best-effort on
/// filesystems that do not support opening directories.
fn sync_dir(dir: &Path) -> Result<(), CoreError> {
    match File::open(dir) {
        Ok(f) => match f.sync_all() {
            Ok(()) => Ok(()),
            // Some filesystems refuse fsync on directories; the rename is
            // still atomic, just not yet durable, which is acceptable.
            Err(err) if err.raw_os_error() == Some(libc::EINVAL) => Ok(()),
            Err(err) => Err(err.into()),
        },
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn stage(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, contents).expect("write staged");
        p
    }

    #[test]
    fn finalizes_into_destination() {
        let staging = tempfile::tempdir().expect("staging");
        let dest = tempfile::tempdir().expect("dest");
        let staged = stage(staging.path(), "s1", b"hello");

        let out = finalize_file(&staged, dest.path(), "photo.jpg").expect("finalize");

        assert_eq!(out, dest.path().join("photo.jpg"));
        assert_eq!(fs::read(&out).expect("read"), b"hello");
        assert!(!staged.exists(), "staged file must be gone");
    }

    #[test]
    fn never_overwrites_existing_files() {
        let staging = tempfile::tempdir().expect("staging");
        let dest = tempfile::tempdir().expect("dest");
        fs::write(dest.path().join("photo.jpg"), b"original").expect("existing");

        let staged = stage(staging.path(), "s1", b"new");
        let out = finalize_file(&staged, dest.path(), "photo.jpg").expect("finalize");

        assert_eq!(out, dest.path().join("photo (1).jpg"));
        assert_eq!(
            fs::read(dest.path().join("photo.jpg")).expect("read"),
            b"original"
        );
        assert_eq!(fs::read(&out).expect("read"), b"new");
    }

    #[test]
    fn collision_suffix_counts_upward() {
        let staging = tempfile::tempdir().expect("staging");
        let dest = tempfile::tempdir().expect("dest");
        fs::write(dest.path().join("a.txt"), b"0").expect("seed");
        fs::write(dest.path().join("a (1).txt"), b"1").expect("seed");

        let staged = stage(staging.path(), "s1", b"2");
        let out = finalize_file(&staged, dest.path(), "a.txt").expect("finalize");
        assert_eq!(out, dest.path().join("a (2).txt"));
    }

    #[test]
    fn sanitizes_hostile_names() {
        let staging = tempfile::tempdir().expect("staging");
        let dest = tempfile::tempdir().expect("dest");
        let staged = stage(staging.path(), "s1", b"x");

        let out = finalize_file(&staged, dest.path(), "../../escape.txt").expect("finalize");
        assert_eq!(out, dest.path().join("escape.txt"));
        assert!(out.starts_with(dest.path()));
    }

    #[test]
    fn creates_destination_if_missing() {
        let staging = tempfile::tempdir().expect("staging");
        let dest = tempfile::tempdir().expect("dest");
        let nested = dest.path().join("Received");
        let staged = stage(staging.path(), "s1", b"x");

        let out = finalize_file(&staged, &nested, "f.bin").expect("finalize");
        assert_eq!(out, nested.join("f.bin"));
    }
}
