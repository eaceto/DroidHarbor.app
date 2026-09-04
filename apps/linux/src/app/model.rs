//! The application model: everything the window renders, plus the handles
//! that keep the domain alive, and the helpers `update` and the event
//! translation lean on.

use std::path::PathBuf;

use tokio::runtime::Runtime;

use dh_domain::{Command, DomainHandle};

use crate::{domain, history, platform, prefs, transfer};

/// Minutes offered by the idle timer, matching the macOS picker.
pub const AUTO_OFF_CHOICES: [u64; 4] = [0, 10, 30, 60];

/// A nearby device that advertised itself while discovery was on.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    pub kind: String,
}

impl Endpoint {
    pub fn icon(&self) -> &'static str {
        // Called while rendering, so a display always exists.
        match self.kind.as_str() {
            // Resolved against the running theme: names differ between
            // Adwaita and Yaru, and a missing one draws a broken image.
            "phone" => crate::ui::resolved_icon(&["phone-symbolic", "computer-symbolic"]),
            "tablet" => crate::ui::resolved_icon(&["tablet-symbolic", "computer-symbolic"]),
            _ => "computer-symbolic",
        }
    }
}

/// What is waiting to be sent, chosen before a device is picked — the order the
/// macOS app uses, since the payload is what the user came to send.
#[derive(Debug, Clone)]
pub enum Staged {
    Files(Vec<PathBuf>),
    Text(String),
}

impl Staged {
    pub fn headline(&self) -> String {
        match self {
            Staged::Files(paths) if paths.len() == 1 => "1 file ready".into(),
            Staged::Files(paths) => format!("{} files ready", paths.len()),
            Staged::Text(_) => "Text ready".into(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Staged::Files(paths) => paths
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(", "),
            Staged::Text(text) => text.clone(),
        }
    }
}

/// Text and links never touch the disk, so they are held here until dismissed.
#[derive(Debug, Clone)]
pub struct ReceivedText {
    pub kind: String,
    pub content: String,
}

pub struct App {
    /// Owns every domain task, so it must outlive the window. Dropping it
    /// would abort transfers mid-flight.
    pub runtime: Runtime,
    pub handle: DomainHandle,
    /// Taken on quit, since `shutdown` consumes the backend.
    pub backend: Option<domain::Backend>,
    /// Parent for dialogs the platform layer may need to present.
    pub window: libadwaita::ApplicationWindow,
    /// Present where the desktop provides a StatusNotifierItem host; absent on
    /// vanilla GNOME, where the Background Apps menu serves the same purpose.
    pub tray: Option<platform::Tray>,

    pub prefs: prefs::Prefs,
    pub prefs_path: PathBuf,
    pub history: Vec<history::Entry>,
    pub history_path: PathBuf,
    /// Bumped whenever the history or its filters change, so the list is only
    /// rebuilt when it actually differs — progress events fire many times a
    /// second and must not tear down rows.
    pub history_revision: u64,

    pub receiving: bool,
    /// When the engine's temporary receiving window ends (Unix epoch
    /// seconds), as `ReceivingUntilChanged` reported it; `None` outside a
    /// window. The engine owns the clock — this is display state only.
    pub receiving_until: Option<u64>,
    pub discovering: bool,
    pub device_name: String,
    pub destination: PathBuf,

    pub active: Option<transfer::Active>,
    pub active_revision: u64,
    pub received_text: Option<ReceivedText>,

    pub endpoints: Vec<Endpoint>,
    pub selected_endpoint: Option<String>,
    pub staged: Option<Staged>,
    /// The last thing sent and where to, kept so an unfinished send can be
    /// offered again rather than silently discarded.
    pub last_send: Option<(String, Staged)>,
    /// Set when that send did not complete.
    pub retry_available: bool,

    pub query: String,
    pub category: history::Category,
    /// A transient line under the header, for things worth saying once.
    pub notice: Option<String>,
    /// Identifies the current notice, so a scheduled expiry only clears the
    /// message it was scheduled for and not a newer one.
    pub notice_id: u64,
    pub background_granted: bool,
}

pub struct AppInit {
    pub runtime: Runtime,
    pub backend: domain::Backend,
    pub prefs: prefs::Prefs,
    pub prefs_path: PathBuf,
    pub history: Vec<history::Entry>,
    pub history_path: PathBuf,
    pub device_name: String,
    pub destination: PathBuf,
}

impl App {
    pub fn dispatch(&self, command: Command) {
        let handle = self.handle.clone();
        self.runtime.spawn(async move {
            if handle.send(command).await.is_err() {
                tracing::error!("domain engine stopped; command dropped");
            }
        });
    }

    /// Push the state the tray displays: the icon's lit/dim state, the line
    /// describing any live transfer, and the destination shown in its menu.
    pub fn sync_tray(&self) {
        let Some(tray) = &self.tray else {
            return;
        };
        let status = self.active.as_ref().map(|active| {
            if active.running {
                format!(
                    "{} — {}%",
                    active.title(),
                    (active.fraction() * 100.0) as u32
                )
            } else {
                active.title()
            }
        });
        platform::update_tray(
            self.runtime.handle(),
            tray,
            self.receiving,
            status,
            self.destination.display().to_string(),
        );
    }

    pub fn save_prefs(&self) {
        prefs::save(&self.prefs_path, &self.prefs);
    }

    pub fn remember(&mut self, entry: history::Entry) {
        self.history.insert(0, entry);
        self.history_revision += 1;
        history::save(&self.history_path, &self.history);
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.notice = Some(text.into());
        self.notice_id += 1;
    }

    pub fn visible_history(&self) -> Vec<&history::Entry> {
        self.history
            .iter()
            .filter(|entry| {
                self.category == history::Category::All || entry.category() == self.category
            })
            .filter(|entry| entry.matches(&self.query))
            .collect()
    }

    /// Whether "what is shown" is a subset of the history worth offering to
    /// clear on its own.
    pub fn history_is_filtered(&self) -> bool {
        self.category != history::Category::All || !self.query.trim().is_empty()
    }

    /// Drop received-file entries whose files have since been moved or
    /// deleted: a row whose every action would fail is not worth keeping.
    /// Runs at launch and whenever the window regains focus — the moments
    /// something may have changed behind the app's back.
    pub fn prune_missing_history(&mut self) {
        let destination = self.destination.clone();
        let before = self.history.len();
        self.history
            .retain(|entry| !entry.is_missing_from_disk(&destination));
        if self.history.len() != before {
            self.history_revision += 1;
            history::save(&self.history_path, &self.history);
        }
    }

    /// Only offer categories something actually falls into, so the control
    /// never advertises an empty filter.
    pub fn available_categories(&self) -> Vec<history::Category> {
        history::Category::ALL
            .into_iter()
            .filter(|category| {
                *category == history::Category::All
                    || self
                        .history
                        .iter()
                        .any(|entry| entry.category() == *category)
            })
            .collect()
    }
}
