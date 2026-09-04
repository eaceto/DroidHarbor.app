//! Non-shipping macOS fallback: DroidHarbor on macOS is the SwiftUI app.
//!
//! None of the Linux services exist here — there is no StatusNotifierItem, no
//! XDG portal, and macOS notifications carry no action buttons through
//! `notify-rust`. Each is replaced with the closest GTK equivalent so the
//! window still runs, which means **these paths are deliberately not the ones
//! that ship**. Consent especially: an in-window dialog here, a notification
//! action on Linux.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use libadwaita::prelude::*;
use tokio::runtime::Handle;

use super::Decision;

/// In-window dialog standing in for the notification prompt.
///
/// `connect_response` takes an `Fn`, but the caller hands us an `FnOnce`, so
/// the callback lives in a `RefCell` and is taken on first use. Adw guarantees
/// exactly one response per dialog; the `Option` makes that explicit rather
/// than assumed.
pub fn ask_consent(
    window: &libadwaita::ApplicationWindow,
    summary: String,
    body: String,
    _timeout: Duration,
    respond: impl FnOnce(Decision) + Send + 'static,
) {
    let dialog = libadwaita::AlertDialog::new(Some(&summary), Some(&body));
    dialog.add_response("reject", "Reject");
    dialog.add_response("always", "Always accept");
    dialog.add_response("accept", "Accept");
    dialog.set_response_appearance("accept", libadwaita::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("reject"));
    dialog.set_close_response("reject");

    let respond = Rc::new(RefCell::new(Some(respond)));
    dialog.connect_response(None, move |_, response| {
        if let Some(respond) = respond.borrow_mut().take() {
            respond(match response {
                "accept" => Decision::Accept,
                "always" => Decision::AcceptAlways,
                _ => Decision::Reject,
            });
        }
    });
    dialog.present(Some(window));
}

/// Nothing is posted here: macOS notifications carry no action buttons through
/// notify-rust, and the shipping app has its own notification code.
pub fn notify_done(summary: String, _body: String, _reveal: Option<PathBuf>, _sound: bool) {
    tracing::info!(%summary, "notification suppressed on macOS");
}

/// GTK's own folder chooser, since there is no portal to route through.
pub fn choose_folder(
    window: &libadwaita::ApplicationWindow,
    _runtime: &Handle,
    respond: impl FnOnce(PathBuf) + Send + 'static,
) {
    let dialog = gtk4::FileDialog::builder()
        .title("Choose where DroidHarbor saves files")
        .accept_label("Use this folder")
        .build();

    dialog.select_folder(
        Some(window),
        gtk4::gio::Cancellable::NONE,
        move |result| match result {
            // GTK hands back a `GFile`, so no URI decoding is needed here —
            // that step exists only on the portal path.
            Ok(file) => {
                if let Some(path) = file.path() {
                    respond(path);
                }
            }
            Err(error) => tracing::debug!(%error, "no folder chosen"),
        },
    );
}

/// Multi-file picker. GTK returns a `GListModel` of `GFile`, which already
/// carries real paths, so there is no URI decoding on this path.
pub fn choose_files(
    window: &libadwaita::ApplicationWindow,
    _runtime: &Handle,
    respond: impl FnOnce(Vec<PathBuf>) + Send + 'static,
) {
    let dialog = gtk4::FileDialog::builder()
        .title("Choose files to send")
        .accept_label("Send")
        .build();

    dialog.open_multiple(
        Some(window),
        gtk4::gio::Cancellable::NONE,
        move |result| match result {
            Ok(files) => {
                let paths: Vec<PathBuf> = (0..files.n_items())
                    .filter_map(|index| files.item(index))
                    .filter_map(|object| object.downcast::<gtk4::gio::File>().ok())
                    .filter_map(|file| file.path())
                    .collect();
                if !paths.is_empty() {
                    respond(paths);
                }
            }
            Err(error) => tracing::debug!(%error, "no files chosen"),
        },
    );
}

/// No StatusNotifierItem on macOS. The shipping app uses the menu bar; here
/// there is simply no tray, which is also the vanilla-GNOME case, so the same
/// window-only code path applies.
#[derive(Debug)]
pub struct Tray;

pub struct TrayActions {
    #[allow(dead_code)]
    pub receive_for: Box<dyn Fn(u64) + Send + 'static>,
    #[allow(dead_code)]
    pub send_files: Box<dyn Fn() + Send + 'static>,
    #[allow(dead_code)]
    pub send_clipboard: Box<dyn Fn() + Send + 'static>,
    #[allow(dead_code)]
    pub choose_destination: Box<dyn Fn() + Send + 'static>,
    #[allow(dead_code)]
    pub about: Box<dyn Fn() + Send + 'static>,
    // Same shape as the Linux type so callers need no cfg of their own. With
    // no tray to attach them to, nothing here is ever invoked.
    #[allow(dead_code)]
    pub set_receiving: Box<dyn Fn(bool) + Send + 'static>,
    #[allow(dead_code)]
    pub present: Box<dyn Fn() + Send + 'static>,
    #[allow(dead_code)]
    pub quit: Box<dyn Fn() + Send + 'static>,
}

pub fn start_tray(
    _runtime: &Handle,
    _destination: String,
    _actions: TrayActions,
    on_ready: impl FnOnce(Option<Tray>) + Send + 'static,
) {
    on_ready(None);
}

pub fn update_tray(
    _runtime: &Handle,
    _tray: &Tray,
    _receiving: bool,
    _status: Option<String>,
    _destination: String,
) {
}

/// `open -R` selects the file in Finder, the same gesture `FileManager1`
/// provides on Linux.
pub fn reveal(_runtime: &Handle, path: PathBuf) {
    if let Err(error) = std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn()
    {
        tracing::info!(%error, "could not reveal the file");
    }
}

/// Launch-at-login belongs to the shipping SwiftUI app, which uses
/// `SMAppService`. Nothing here touches the user's login items.
pub fn set_launch_at_login(_enabled: bool) -> anyhow::Result<()> {
    tracing::info!("launch at login is a no-op on macOS");
    Ok(())
}

pub fn launch_at_login_is_healthy() -> bool {
    true
}

pub fn launch_at_login_enabled() -> bool {
    false
}

/// No background portal on macOS. Closing the window simply hides it and the
/// process keeps running.
pub fn request_background(_runtime: &Handle, status: String, _need_permission: bool) {
    tracing::info!(%status, "background status is a no-op on macOS");
}
