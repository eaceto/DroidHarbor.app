# dh-core

Data layer: filesystem safety and transfer limits, shared by every front door
(Quick Share today, QR+HTTP later). Protocol-agnostic, no UI, no per-OS code.

## API

- `paths::sanitize_file_name(raw: &str) -> String` reduces an untrusted,
  sender-supplied name to one safe path component: takes the basename (drops
  traversal), NFC-normalizes, strips control chars, caps length at 255 bytes
  preserving the extension. Always returns a usable name (`"unnamed"` as a
  last resort); a weird name must not abort a transfer.
- `paths::split_extension(name: &str) -> (&str, &str)` splits a name into
  stem and extension, used for collision suffixes.
- `limits::Limits` holds policy values (max files, free-space headroom,
  timeouts) with the spec §10 defaults; `check_introduction(file_count,
  total_bytes, available_bytes)` validates an incoming batch before it
  reaches the user. Free space is injected by the caller: querying it is a
  platform concern.
- `finalize::finalize_file(staged, dest_dir, desired_name) -> PathBuf`
  atomically places a fully-received staged file into the destination: fsync,
  race-safe collision claim (`name (n).ext`, never overwrites), `rename(2)`
  with a copy-within-destination fallback across filesystems, directory fsync.
- `CoreError` is the error enum for all of the above.
