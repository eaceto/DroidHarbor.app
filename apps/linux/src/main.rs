//! DroidHarbor for Linux.
//!
//! Window-first by decision, not by accident: vanilla GNOME has no
//! StatusNotifierItem host, so the window is the primary surface and the tray
//! is an enhancement added where the desktop provides one.
//!
//! The layout follows the macOS app closely — an incoming card above a
//! searchable history, a staged payload above a device list, the same settings
//! in the same order — with the view switcher taking the place of its sidebar.
//!
//! The crate is arranged by responsibility: `app` holds the relm4 component —
//! model, messages, update wiring and domain-event handling — and `ui` holds
//! widget construction and rendering, one page per module. The modules beside
//! them are support layers: persistence, formatting, the domain front door,
//! and the per-platform glue.

mod app;
mod domain;
mod format;
mod history;
mod platform;
mod prefs;
mod transfer;
mod ui;
mod update;
/// Only the XDG portal answers in URIs; GTK's own chooser returns paths, so
/// nothing calls this on macOS. The logic is plain Unix path handling, so it
/// still compiles there.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod uri;

use std::path::PathBuf;

use app::{App, AppInit};
use relm4::RelmApp;
use tokio::runtime::Runtime;

pub const APP_ID: &str = "dev.eaceto.apps.linux.droidharbor";

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,droidharbor=info,dh_qs_core=info".into()),
        )
        .init();

    let prefs_path = prefs::file_path();
    let mut stored = prefs::load(&prefs_path);
    // Keep the switch honest if the entry was removed outside the app.
    stored.launch_at_login = platform::launch_at_login_enabled();

    let history_path = history::file_path();
    let entries = history::load(&history_path);

    let device_name = stored
        .device_name
        .clone()
        .unwrap_or_else(domain::default_device_name);
    let destination = stored
        .destination
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(domain::default_destination);

    // The domain needs a runtime before any window exists, and that runtime has
    // to outlive the GTK main loop, so it is created here and moved into the
    // model rather than being spun up inside a component.
    let runtime = Runtime::new()?;
    let backend = runtime.block_on(domain::start(device_name.clone(), destination.clone()))?;

    let app = libadwaita::Application::builder()
        .application_id(APP_ID)
        .build();
    RelmApp::from_app(app).run::<App>(AppInit {
        runtime,
        backend,
        prefs: stored,
        prefs_path,
        history: entries,
        history_path,
        device_name,
        destination,
    });
    Ok(())
}
