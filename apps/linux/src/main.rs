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
//! Widgets are built by hand rather than through relm4's `view!` macro. The
//! header's view switcher has to reference the stack that the switcher's own
//! parent contains, which the macro's declaration-order construction makes
//! awkward; explicit construction keeps that wiring obvious.

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
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use libadwaita::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmApp, SimpleComponent};
use tokio::runtime::Runtime;

use dh_domain::{Command, DomainHandle, Event, SessionId};

pub const APP_ID: &str = "dev.eaceto.apps.linux.droidharbor";

const CONSENT_TIMEOUT: Duration = Duration::from_secs(60);

/// A nearby device that advertised itself while discovery was on.
#[derive(Debug, Clone)]
struct Endpoint {
    id: String,
    name: String,
    kind: String,
}

impl Endpoint {
    fn icon(&self) -> &'static str {
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
enum Staged {
    Files(Vec<PathBuf>),
    Text(String),
}

impl Staged {
    fn headline(&self) -> String {
        match self {
            Staged::Files(paths) if paths.len() == 1 => "1 file ready".into(),
            Staged::Files(paths) => format!("{} files ready", paths.len()),
            Staged::Text(_) => "Text ready".into(),
        }
    }

    fn detail(&self) -> String {
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
struct ReceivedText {
    kind: String,
    content: String,
}

struct App {
    /// Owns every domain task, so it must outlive the window. Dropping it
    /// would abort transfers mid-flight.
    runtime: Runtime,
    handle: DomainHandle,
    /// Taken on quit, since `shutdown` consumes the backend.
    backend: Option<domain::Backend>,
    /// Parent for dialogs the platform layer may need to present.
    window: libadwaita::ApplicationWindow,
    /// Present where the desktop provides a StatusNotifierItem host; absent on
    /// vanilla GNOME, where the Background Apps menu serves the same purpose.
    tray: Option<platform::Tray>,

    prefs: prefs::Prefs,
    prefs_path: PathBuf,
    history: Vec<history::Entry>,
    history_path: PathBuf,
    /// Bumped whenever the history or its filters change, so the list is only
    /// rebuilt when it actually differs — progress events fire many times a
    /// second and must not tear down rows.
    history_revision: u64,

    receiving: bool,
    discovering: bool,
    device_name: String,
    destination: PathBuf,

    active: Option<transfer::Active>,
    active_revision: u64,
    received_text: Option<ReceivedText>,

    endpoints: Vec<Endpoint>,
    selected_endpoint: Option<String>,
    staged: Option<Staged>,
    /// The last thing sent and where to, kept so an unfinished send can be
    /// offered again rather than silently discarded.
    last_send: Option<(String, Staged)>,
    /// Set when that send did not complete.
    retry_available: bool,

    query: String,
    category: history::Category,
    /// A transient line under the header, for things worth saying once.
    notice: Option<String>,
    /// Identifies the current notice, so a scheduled expiry only clears the
    /// message it was scheduled for and not a newer one.
    notice_id: u64,
    background_granted: bool,
}

#[derive(Debug)]
enum Msg {
    /// Does nothing but reach `update_view`. relm4 does not render after
    /// `init`, so without this the loaded history and the initial switch
    /// states would not appear until some other event happened to arrive.
    Refresh,
    Domain(Event),
    SetReceiving(bool),
    SetDiscovering(bool),
    Consent {
        session: SessionId,
        decision: platform::Decision,
    },
    Cancel(SessionId),
    HideToBackground,
    ChooseDestination,
    DestinationChosen(PathBuf),
    RenameDevice(String),
    SetAutoOff(u64),
    SetLaunchAtLogin(bool),
    SetPlaySounds(bool),
    /// Linux only: an AppImage is run, not installed.
    #[cfg(target_os = "linux")]
    InstallDesktopEntry,
    Revoke(String),
    SelectEndpoint(String),
    StageFiles,
    Staged(Vec<PathBuf>),
    StageText(String),
    ClearStaged,
    Send,
    RetrySend,
    DismissRetry,
    ShowOnboarding,
    FinishOnboarding,
    CheckForUpdates,
    UpdateAvailable(Option<update::Available>),
    Search(String),
    Filter(history::Category),
    RemoveEntry(uuid::Uuid),
    ClearHistory,
    Reveal(PathBuf),
    CopyText(String),
    DismissText,
    DismissNotice,
    /// Fired by the timer started when a notice appeared.
    ExpireNotice(u64),
    /// The tray registered (or did not).
    TrayReady(Option<platform::Tray>),
    /// Bring the window back from the tray or a second launch.
    Present,
    ShowAbout,
    /// Turn receiving on and let it lapse after `minutes`.
    ReceiveFor(u64),
    SendClipboardText,
    Quit,
}

struct AppInit {
    runtime: Runtime,
    backend: domain::Backend,
    prefs: prefs::Prefs,
    prefs_path: PathBuf,
    history: Vec<history::Entry>,
    history_path: PathBuf,
    device_name: String,
    destination: PathBuf,
}

/// Minutes offered by the idle timer, matching the macOS picker.
const AUTO_OFF_CHOICES: [u64; 4] = [0, 10, 30, 60];

impl App {
    fn dispatch(&self, command: Command) {
        let handle = self.handle.clone();
        self.runtime.spawn(async move {
            if handle.send(command).await.is_err() {
                tracing::error!("domain engine stopped; command dropped");
            }
        });
    }

    /// Push the state the tray displays: the icon's lit/dim state, the line
    /// describing any live transfer, and the destination shown in its menu.
    fn sync_tray(&self) {
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

    fn save_prefs(&self) {
        prefs::save(&self.prefs_path, &self.prefs);
    }

    fn remember(&mut self, entry: history::Entry) {
        self.history.insert(0, entry);
        self.history_revision += 1;
        history::save(&self.history_path, &self.history);
    }

    fn notice(&mut self, text: impl Into<String>) {
        self.notice = Some(text.into());
        self.notice_id += 1;
    }

    fn visible_history(&self) -> Vec<&history::Entry> {
        self.history
            .iter()
            .filter(|entry| {
                self.category == history::Category::All || entry.category() == self.category
            })
            .filter(|entry| entry.matches(&self.query))
            .collect()
    }

    /// Only offer categories something actually falls into, so the control
    /// never advertises an empty filter.
    fn available_categories(&self) -> Vec<history::Category> {
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

impl SimpleComponent for App {
    type Init = AppInit;
    type Input = Msg;
    type Output = ();
    type Root = libadwaita::ApplicationWindow;
    type Widgets = ui::AppWidgets;

    fn init_root() -> Self::Root {
        libadwaita::ApplicationWindow::builder()
            .title("DroidHarbor")
            .default_width(820)
            .default_height(620)
            .build()
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = App {
            handle: init.backend.handle.clone(),
            runtime: init.runtime,
            backend: Some(init.backend),
            window: root.clone(),
            tray: None,
            prefs: init.prefs,
            prefs_path: init.prefs_path,
            history: init.history,
            history_path: init.history_path,
            history_revision: 1,
            receiving: false,
            discovering: false,
            device_name: init.device_name,
            destination: init.destination,
            active: None,
            active_revision: 0,
            received_text: None,
            endpoints: Vec::new(),
            selected_endpoint: None,
            staged: None,
            last_send: None,
            retry_available: false,
            query: String::new(),
            category: history::Category::All,
            notice: None,
            notice_id: 0,
            background_granted: false,
        };

        // An autostart entry pointing at a binary that has since moved does
        // nothing at all, which is worse than not having one.
        if model.prefs.launch_at_login && !platform::launch_at_login_is_healthy() {
            model.prefs.launch_at_login = false;
            model.save_prefs();
            model.notice("Open at login was switched off: the application file moved.");
        }

        {
            let input = sender.input_sender().clone();
            let mut events = model.handle.subscribe();
            model.runtime.spawn(async move {
                loop {
                    match events.recv().await {
                        Ok(event) => {
                            if input.send(Msg::Domain(event)).is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, "event stream lagged");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        {
            let sender = sender.clone();
            root.connect_close_request(move |window| {
                if cfg!(target_os = "linux") {
                    window.set_visible(false);
                    sender.input(Msg::HideToBackground);
                } else {
                    // Off Linux there is neither background portal nor tray, so
                    // hiding would strand the process with no way back.
                    sender.input(Msg::Quit);
                }
                gtk4::glib::Propagation::Stop
            });
        }

        if let Some(app) = root.application() {
            let window = root.clone();
            app.connect_activate(move |_| window.present());
        }

        let widgets = ui::build(&sender, &model, &root);

        // Apply persisted choices that the domain has to know about.
        if model.prefs.auto_off_minutes > 0 {
            let minutes = model.prefs.auto_off_minutes;
            model.dispatch(Command::SetAutoOffMinutes(minutes));
        }

        // Inside an AppImage the bundled theme is the only one guaranteed to
        // exist, and relying on the host's XDG_DATA_DIRS to surface it has
        // proved unreliable. Point the icon theme at it explicitly, and report
        // what resolves — an icon that silently fails to render otherwise
        // leaves nothing at all in the logs.
        {
            let display = gtk4::prelude::WidgetExt::display(&root);
            let theme = gtk4::IconTheme::for_display(&display);
            if let Some(appdir) = std::env::var_os("APPDIR") {
                let bundled = PathBuf::from(appdir).join("usr/share/icons");
                theme.add_search_path(&bundled);
                tracing::info!(path = %bundled.display(), "added bundled icon path");
            }
            tracing::info!(
                theme = %theme.theme_name(),
                search_path = ?theme.search_path(),
                "icon theme"
            );
            for name in [
                "send-to-symbolic",
                "document-send-symbolic",
                "folder-download-symbolic",
                "document-open-recent-symbolic",
                "preferences-system-symbolic",
                "phone-symbolic",
                APP_ID,
            ] {
                if !theme.has_icon(name) {
                    tracing::warn!(icon = name, "icon missing from the theme");
                }
            }
        }

        // The tray is an enhancement: registration is asynchronous and may
        // simply not be possible, which is the ordinary vanilla-GNOME case.
        {
            let for_receiving = sender.input_sender().clone();
            let for_duration = sender.input_sender().clone();
            let for_files = sender.input_sender().clone();
            let for_clipboard = sender.input_sender().clone();
            let for_destination = sender.input_sender().clone();
            let for_about = sender.input_sender().clone();
            let for_present = sender.input_sender().clone();
            let for_quit = sender.input_sender().clone();
            let for_ready = sender.input_sender().clone();
            platform::start_tray(
                model.runtime.handle(),
                model.destination.display().to_string(),
                platform::TrayActions {
                    set_receiving: Box::new(move |on| {
                        let _ = for_receiving.send(Msg::SetReceiving(on));
                    }),
                    receive_for: Box::new(move |minutes| {
                        let _ = for_duration.send(Msg::ReceiveFor(minutes));
                    }),
                    send_files: Box::new(move || {
                        let _ = for_files.send(Msg::StageFiles);
                    }),
                    send_clipboard: Box::new(move || {
                        let _ = for_clipboard.send(Msg::SendClipboardText);
                    }),
                    choose_destination: Box::new(move || {
                        let _ = for_destination.send(Msg::ChooseDestination);
                    }),
                    about: Box::new(move || {
                        let _ = for_about.send(Msg::ShowAbout);
                    }),
                    present: Box::new(move || {
                        let _ = for_present.send(Msg::Present);
                    }),
                    quit: Box::new(move || {
                        let _ = for_quit.send(Msg::Quit);
                    }),
                },
                move |tray| {
                    let _ = for_ready.send(Msg::TrayReady(tray));
                },
            );
        }

        if !model.prefs.onboarded {
            sender.input(Msg::ShowOnboarding);
        }

        // Force one render, so the loaded history and the persisted switch
        // positions are on screen before anything else happens.
        sender.input(Msg::Refresh);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Msg::Refresh => {}

            // Guarded so a switch reacting to its own state change does not
            // bounce a redundant command back into the domain.
            Msg::SetReceiving(on) if on == self.receiving => {}
            Msg::SetReceiving(on) => self.dispatch(Command::SetReceiving(on)),

            Msg::SetDiscovering(on) if on == self.discovering => {}
            Msg::SetDiscovering(on) => self.dispatch(Command::SetDiscovering(on)),

            Msg::Consent { session, decision } => {
                if decision == platform::Decision::AcceptAlways {
                    if let Some(peer) = self.active.as_ref().map(|active| active.peer.clone()) {
                        self.prefs.trust(&peer);
                        self.save_prefs();
                        self.notice(format!(
                            "“{peer}” will be accepted automatically from now on."
                        ));
                    }
                }
                let accepted = decision != platform::Decision::Reject;
                self.dispatch(if accepted {
                    Command::Accept(session)
                } else {
                    Command::Decline(session)
                });
                if !accepted {
                    self.active = None;
                    self.active_revision += 1;
                }
            }

            Msg::Cancel(session) => self.dispatch(Command::Cancel(session)),

            Msg::HideToBackground => {
                let status = if self.receiving {
                    "Receiving files"
                } else {
                    "Idle"
                };
                let first_time = !self.background_granted;
                self.background_granted = true;
                platform::request_background(self.runtime.handle(), status.into(), first_time);
            }

            Msg::ChooseDestination => {
                let input = sender.input_sender().clone();
                platform::choose_folder(&self.window, self.runtime.handle(), move |path| {
                    let _ = input.send(Msg::DestinationChosen(path));
                });
            }

            Msg::DestinationChosen(path) => {
                self.destination = path.clone();
                self.prefs.destination = Some(path.display().to_string());
                self.save_prefs();
                self.dispatch(Command::SetDestination(path.display().to_string()));
            }

            Msg::RenameDevice(name) => {
                let name = name.trim().to_string();
                if name.is_empty() || name == self.device_name {
                    return;
                }
                self.device_name = name.clone();
                self.prefs.device_name = Some(name.clone());
                self.save_prefs();
                self.dispatch(Command::SetDeviceName(name));
                self.notice("Renamed. The receiver restarts to advertise the new name.");
            }

            Msg::SetAutoOff(minutes) => {
                if self.prefs.auto_off_minutes == minutes {
                    return;
                }
                self.prefs.auto_off_minutes = minutes;
                self.save_prefs();
                self.dispatch(Command::SetAutoOffMinutes(minutes));
            }

            Msg::SetLaunchAtLogin(on) => {
                if self.prefs.launch_at_login == on {
                    return;
                }
                match platform::set_launch_at_login(on) {
                    Ok(()) => {
                        self.prefs.launch_at_login = on;
                        self.save_prefs();
                    }
                    Err(error) => self.notice(format!("Could not change open at login: {error}")),
                }
            }

            #[cfg(target_os = "linux")]
            Msg::InstallDesktopEntry => match platform::install_desktop_entry() {
                Ok(path) => self.notice(format!(
                    "Added to the launcher ({}).",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )),
                Err(error) => self.notice(format!("Could not add to the launcher: {error}")),
            },

            Msg::SetPlaySounds(on) => {
                if self.prefs.play_sounds != on {
                    self.prefs.play_sounds = on;
                    self.save_prefs();
                }
            }

            Msg::Revoke(device) => {
                self.prefs.revoke(&device);
                self.save_prefs();
            }

            Msg::SelectEndpoint(id) => self.selected_endpoint = Some(id),

            Msg::StageFiles => {
                let input = sender.input_sender().clone();
                platform::choose_files(&self.window, self.runtime.handle(), move |paths| {
                    let _ = input.send(Msg::Staged(paths));
                });
            }

            Msg::Staged(paths) => self.staged = Some(Staged::Files(paths)),

            Msg::StageText(text) => {
                let text = text.trim().to_string();
                self.staged = if text.is_empty() {
                    None
                } else {
                    Some(Staged::Text(text))
                };
            }

            Msg::ClearStaged => self.staged = None,

            Msg::Send => {
                let (Some(staged), Some(endpoint)) =
                    (self.staged.clone(), self.selected_endpoint.clone())
                else {
                    return;
                };
                let peer = self
                    .endpoints
                    .iter()
                    .find(|e| e.id == endpoint)
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| endpoint.clone());

                self.last_send = Some((endpoint.clone(), staged.clone()));
                self.retry_available = false;
                match staged {
                    Staged::Files(paths) => {
                        // The domain takes absolute paths as strings, per the
                        // plain-data rule that keeps this API usable over FFI.
                        let files: Vec<String> = paths
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect();
                        self.remember(history::Entry::new(
                            history::Direction::Sent,
                            peer,
                            files.clone(),
                        ));
                        self.dispatch(Command::SendFiles { endpoint, files });
                    }
                    Staged::Text(content) => {
                        // A web address is worth announcing as a link: the
                        // phone then offers to open it rather than only copy it.
                        let is_link =
                            content.starts_with("http://") || content.starts_with("https://");
                        let kind = if is_link { "link" } else { "text" };
                        self.remember(history::Entry::text(
                            history::Direction::Sent,
                            peer,
                            history::Kind::from_domain(kind),
                            content.clone(),
                        ));
                        self.dispatch(Command::SendText {
                            endpoint,
                            kind: kind.into(),
                            description: content.chars().take(60).collect(),
                            content,
                        });
                    }
                }
                self.staged = None;
            }

            Msg::RetrySend => {
                let Some((endpoint, staged)) = self.last_send.clone() else {
                    return;
                };
                self.retry_available = false;
                self.selected_endpoint = Some(endpoint);
                self.staged = Some(staged);
                self.notice("Ready to try again — press Send.");
            }

            Msg::DismissRetry => self.retry_available = false,

            Msg::ShowOnboarding => ui::present_onboarding(&self.window, sender.clone()),

            Msg::CheckForUpdates => {
                self.notice("Checking for updates…");
                let input = sender.input_sender().clone();
                let current = env!("CARGO_PKG_VERSION").to_string();
                // Blocking HTTP, so it runs on a worker rather than the GTK
                // loop or an async task it would stall.
                self.runtime.spawn_blocking(move || {
                    let found = update::check(&current, update::MANIFEST);
                    let _ = input.send(Msg::UpdateAvailable(found));
                });
            }

            Msg::UpdateAvailable(Some(release)) => {
                let mut text =
                    format!("Version {} is available — {}", release.version, release.url);
                if let Some(notes) = release.notes.as_deref().map(str::trim) {
                    if !notes.is_empty() {
                        // One line: the banner is not a changelog viewer.
                        text.push_str(" · ");
                        text.push_str(notes.lines().next().unwrap_or_default());
                    }
                }
                self.notice(text);
            }

            Msg::UpdateAvailable(None) => self.notice("DroidHarbor is up to date."),

            Msg::FinishOnboarding => {
                self.prefs.onboarded = true;
                self.save_prefs();
            }

            Msg::Search(text) => {
                self.query = text;
                self.history_revision += 1;
            }

            Msg::Filter(category) => {
                self.category = category;
                self.history_revision += 1;
            }

            Msg::RemoveEntry(id) => {
                self.history.retain(|entry| entry.id != id);
                self.history_revision += 1;
                history::save(&self.history_path, &self.history);
            }

            Msg::ClearHistory => {
                self.history.clear();
                self.history_revision += 1;
                history::save(&self.history_path, &self.history);
            }

            Msg::Reveal(path) => platform::reveal(self.runtime.handle(), path),

            Msg::CopyText(text) => {
                // `display` exists on both RootExt and WidgetExt; name the one
                // we mean rather than relying on inference.
                gtk4::prelude::WidgetExt::display(&self.window)
                    .clipboard()
                    .set_text(&text);
                self.notice("Copied to the clipboard.");
            }

            Msg::DismissText => self.received_text = None,
            Msg::TrayReady(tray) => {
                self.tray = tray;
                // The icon starts idle; push whatever it missed.
                self.sync_tray();
            }

            Msg::ShowAbout => ui::present_about(&self.window),

            Msg::ReceiveFor(minutes) => {
                // Deliberately not saved: this is a one-off, not a change to
                // the preference the Settings page shows.
                self.dispatch(Command::SetAutoOffMinutes(minutes));
                self.dispatch(Command::SetReceiving(true));
                self.notice(format!("Receiving for {minutes} minutes."));
            }

            Msg::SendClipboardText => {
                let input = sender.input_sender().clone();
                gtk4::prelude::WidgetExt::display(&self.window)
                    .clipboard()
                    .read_text_async(gtk4::gio::Cancellable::NONE, move |result| {
                        match result {
                            Ok(Some(text)) if !text.trim().is_empty() => {
                                let _ = input.send(Msg::StageText(text.to_string()));
                                let _ = input.send(Msg::Present);
                            }
                            // Nothing usable on the clipboard is not an error
                            // worth a dialog; the window simply opens empty.
                            _ => {
                                let _ = input.send(Msg::Present);
                            }
                        }
                    });
            }

            Msg::Present => {
                self.window.set_visible(true);
                self.window.present();
            }

            Msg::DismissNotice => self.notice = None,

            // Ignored when a newer notice has since replaced this one.
            Msg::ExpireNotice(id) if id == self.notice_id => self.notice = None,
            Msg::ExpireNotice(_) => {}

            Msg::Quit => {
                // Blocking briefly is deliberate: the front door must
                // unregister from mDNS before the process disappears, or the
                // phone keeps offering a device that is no longer listening.
                if let Some(backend) = self.backend.take() {
                    self.runtime.block_on(backend.shutdown());
                }
                relm4::main_application().quit();
            }

            Msg::Domain(event) => self.on_domain_event(event, &sender),
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        ui::render(self, widgets, &sender);
    }
}

impl App {
    fn on_domain_event(&mut self, event: Event, sender: &ComponentSender<Self>) {
        match event {
            Event::AdvertisingChanged(on) => {
                self.receiving = on;
                self.sync_tray();
                if !on {
                    self.notice("Receiving is off.");
                }
            }

            Event::SessionConnected { .. } => {}

            Event::IntroductionReceived {
                session,
                sender_name,
                files,
                total_bytes,
                token,
                text_preview,
            } => {
                let lines = files
                    .iter()
                    .map(|file| transfer::FileLine {
                        name: file.name.clone(),
                        transferred: 0,
                        size: file.size,
                        completed: false,
                    })
                    .collect();
                self.active = Some(transfer::Active::incoming(
                    session,
                    sender_name.clone(),
                    token.clone(),
                    total_bytes,
                    text_preview.clone(),
                    lines,
                ));
                self.active_revision += 1;

                // A trusted sender skips the prompt entirely, which is the
                // whole point of trusting it.
                if self.prefs.trusts(&sender_name) {
                    self.notice(format!("Accepted from “{sender_name}” automatically."));
                    self.dispatch(Command::Accept(session));
                    return;
                }

                let summary = format!("Accept files from {sender_name}?");
                // The code goes on its own line: it is the one thing the user
                // has to compare with the phone, and buried mid-sentence it
                // reads as decoration.
                let what = match &text_preview {
                    Some(preview) if !preview.is_empty() => preview.clone(),
                    Some(_) => "A link or text".to_string(),
                    None => format!(
                        "{} file{} · {}",
                        files.len(),
                        if files.len() == 1 { "" } else { "s" },
                        format::bytes(total_bytes),
                    ),
                };
                let body = if token.is_empty() {
                    what
                } else {
                    format!("{what}\n\nCode {token} — must match the phone")
                };
                let sender = sender.clone();
                platform::ask_consent(&self.window, summary, body, CONSENT_TIMEOUT, move |d| {
                    sender.input(Msg::Consent {
                        session,
                        decision: d,
                    });
                });
            }

            Event::Progress {
                bytes_received,
                total_bytes,
                current_file,
                files,
                ..
            } => {
                let Some(active) = self.active.as_mut() else {
                    return;
                };
                active.record_progress(bytes_received, total_bytes, Instant::now());
                active.current_file = current_file;
                if !files.is_empty() {
                    if files.len() != active.files.len() {
                        self.active_revision += 1;
                    }
                    active.files = files
                        .iter()
                        .map(|file| transfer::FileLine {
                            name: file.name.clone(),
                            transferred: file.bytes_transferred,
                            size: file.size,
                            completed: file.completed,
                        })
                        .collect();
                }
            }

            Event::FileFinalized { .. } => {}

            Event::TextReceived {
                kind,
                content,
                description,
                ..
            } => {
                let peer = self
                    .active
                    .as_ref()
                    .map(|a| a.peer.clone())
                    .unwrap_or_else(|| "a nearby device".into());
                // Text never touches the disk, so the clipboard is where it
                // becomes useful.
                gtk4::prelude::WidgetExt::display(&self.window)
                    .clipboard()
                    .set_text(&content);
                self.received_text = Some(ReceivedText {
                    kind: kind.clone(),
                    content: content.clone(),
                });
                self.remember(history::Entry::text(
                    history::Direction::Received,
                    peer,
                    history::Kind::from_domain(&kind),
                    if content.is_empty() {
                        description
                    } else {
                        content
                    },
                ));
            }

            Event::SessionEnded { outcome, .. } => {
                let finished = self.active.take();
                let was_outgoing = finished.as_ref().is_some_and(|active| active.outgoing);
                self.active_revision += 1;

                if let Some(active) = &finished {
                    if active.text_preview.is_none() && !active.outgoing {
                        let paths: Vec<String> =
                            active.files.iter().map(|file| file.name.clone()).collect();
                        if !paths.is_empty() {
                            self.remember(history::Entry::new(
                                history::Direction::Received,
                                active.peer.clone(),
                                paths.clone(),
                            ));
                        }
                    }
                }

                let text = outcome_text(outcome, was_outgoing);
                self.notice(text);

                // The window may well be closed, so say it where it will be
                // seen rather than only in a banner nobody is looking at.
                if let Some(active) = &finished {
                    let reveal = if outcome == dh_domain::SessionOutcome::Completed
                        && !was_outgoing
                        && active.text_preview.is_none()
                    {
                        active
                            .files
                            .first()
                            .map(|file| self.destination.join(&file.name))
                    } else {
                        None
                    };
                    platform::notify_done(
                        format!("{} · DroidHarbor", active.peer),
                        text.to_string(),
                        reveal,
                        self.prefs.play_sounds,
                    );
                }

                // Anything short of completion leaves the payload worth
                // another attempt.
                if was_outgoing && outcome != dh_domain::SessionOutcome::Completed {
                    self.retry_available = self.last_send.is_some();
                }
                self.sync_tray();
            }

            Event::ErrorOccurred { code, message, .. } => {
                self.notice(format!("{message} ({code:?})"));
            }

            Event::DiscoveringChanged(on) => {
                self.discovering = on;
                if !on {
                    // Endpoints only exist while discovery runs; keeping stale
                    // ones would offer devices that can no longer be reached.
                    self.endpoints.clear();
                    self.selected_endpoint = None;
                }
            }

            Event::EndpointUpdated {
                endpoint,
                name,
                kind,
                present,
            } => {
                if present {
                    match self.endpoints.iter_mut().find(|e| e.id == endpoint) {
                        // Re-advertisements update in place, so the row does
                        // not jump to the bottom of the list.
                        Some(existing) => {
                            existing.name = name;
                            existing.kind = kind;
                        }
                        None => self.endpoints.push(Endpoint {
                            id: endpoint,
                            name,
                            kind,
                        }),
                    }
                } else {
                    self.endpoints.retain(|e| e.id != endpoint);
                    if self.selected_endpoint.as_deref() == Some(endpoint.as_str()) {
                        self.selected_endpoint = None;
                    }
                }
            }

            Event::SendAwaitingConsent {
                session,
                total_bytes,
                token,
            } => {
                let peer = self
                    .selected_endpoint
                    .as_ref()
                    .and_then(|id| self.endpoints.iter().find(|e| &e.id == id))
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| "the phone".into());
                self.active = Some(transfer::Active::outgoing(
                    session,
                    peer,
                    token.clone(),
                    total_bytes,
                ));
                self.active_revision += 1;
                self.notice(if token.is_empty() {
                    "Waiting for the phone to accept…".to_string()
                } else {
                    format!("Waiting for the phone to accept — code {token}")
                });
            }
        }
    }
}

/// Outcomes reach the user as sentences, not as enum variants.
fn outcome_text(outcome: dh_domain::SessionOutcome, outgoing: bool) -> &'static str {
    use dh_domain::SessionOutcome::*;
    match (outcome, outgoing) {
        (Completed, false) => "Transfer complete.",
        (Completed, true) => "Sent.",
        (Rejected, false) => "Declined.",
        (Rejected, true) => "The phone declined it.",
        (Cancelled, _) => "Cancelled.",
        (TimedOut, _) => "Timed out with no answer.",
        (Failed, _) => "The transfer failed.",
    }
}

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
