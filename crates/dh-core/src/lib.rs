//! Data-layer filesystem safety and transfer limits.
//!
//! This crate is protocol-agnostic: both the Quick Share front door and the
//! future QR+HTTP front door finalize received files through it. It never
//! renders UI, never asks the user anything, and contains no per-OS logic.

pub mod error;
pub mod finalize;
pub mod limits;
pub mod paths;
pub mod space;

pub use error::CoreError;
