//! Desktop integration, which is the only part of this app that is not
//! portable.
//!
//! Linux is the target. Everything Linux-specific — StatusNotifierItem, XDG
//! portals, notification actions — has no macOS counterpart, so each platform
//! supplies its own implementation behind this one interface.
//!
//! The macOS side does not ship: DroidHarbor on macOS is the SwiftUI app. Its
//! consent path also differs from Linux — an in-window dialog rather than a
//! notification action.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{
    ask_consent, choose_files, choose_folder, install_desktop_entry, launch_at_login_enabled,
    launch_at_login_is_healthy, notify_done, request_background, reveal, set_launch_at_login,
    start_tray, update_tray, Tray, TrayActions,
};

#[cfg(not(target_os = "linux"))]
mod macos;
#[cfg(not(target_os = "linux"))]
pub use macos::{
    ask_consent, choose_files, choose_folder, launch_at_login_enabled, launch_at_login_is_healthy,
    notify_done, request_background, reveal, set_launch_at_login, start_tray, update_tray, Tray,
    TrayActions,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Accept,
    /// Accept, and trust this sender from now on. Offered next to Accept so
    /// the choice is made where the decision is, rather than as a checkbox on
    /// a panel behind the prompt.
    AcceptAlways,
    Reject,
}
