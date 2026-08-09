//! Quick Share front door.
//!
//! Wraps [rquickshare](https://github.com/Martichou/rquickshare)'s `rqs_lib`
//! (GPL-3.0, rev-pinned), which implements the reverse-engineered Nearby
//! Sharing protocol: mDNS advertisement, UKEY2 handshake, the encrypted
//! channel, and payload reception.
//!
//! This crate's job is adaptation, not protocol: it translates rqs_lib's
//! `ChannelMessage` stream into `dh-domain` front-door signals and drives
//! rqs_lib from front-door controls. rqs_lib writes incoming files into a
//! staging directory owned by us; on completion each file is handed to the
//! domain as `FileStaged`, so `dh-core`'s atomic finalization still owns the
//! last mile into the user's folder.
//!
//! The advertised device name is set at construction via
//! [`QuickShareConfig::device_name`] (our fork's `with_device_name`);
//! renaming while running requires a service restart.

pub mod adapter;

pub use adapter::{spawn, FrontDoorError, QuickShareConfig};
