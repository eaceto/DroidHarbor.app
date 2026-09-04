//! The relm4 wiring: how the component starts, and what each message does.

use std::path::PathBuf;

use gtk4::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

use dh_domain::Command;

use super::model::{App, AppInit, Staged};
use super::Msg;
use crate::{history, platform, ui, update, APP_ID};

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
            receiving_until: None,
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

        // Entries pointing at files deleted since the last run would offer
        // only actions that fail.
        model.prune_missing_history();

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

        // Files get moved and deleted in the file manager while the app sits
        // in the background; regaining focus is the moment to notice.
        {
            let sender = sender.clone();
            root.connect_is_active_notify(move |window| {
                if window.is_active() {
                    sender.input(Msg::PruneHistory);
                }
            });
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

            // Dispatched unconditionally: comparing against the model here
            // would swallow the second half of a fast off-on-off. The domain
            // acknowledges every command, and a redundant one is absorbed by
            // its own is-it-already-so check.
            //
            // The model moves now rather than waiting for that acknowledgement
            // so the next render agrees with the switch the user just flipped.
            // Left to lag, render would push the switch back to the old value
            // for the length of the round trip — a visible bounce, and one the
            // acknowledgements then chase.
            Msg::SetReceiving(on) => {
                self.receiving = on;
                self.dispatch(Command::SetReceiving(on));
            }

            Msg::SetDiscovering(on) => {
                self.discovering = on;
                self.dispatch(Command::SetDiscovering(on));
            }

            Msg::Consent { session, decision } => {
                // The same consent can be answered twice — the window card
                // and a notification that outlived it — or arrive for a
                // session long gone. Only the still-pending active session
                // may be answered: a stale notification's expiry otherwise
                // declined a transfer the user had already accepted, and
                // wiped its card mid-flight.
                let pending = self
                    .active
                    .as_ref()
                    .is_some_and(|a| a.session == session && !a.outgoing && !a.running);
                if !pending {
                    tracing::debug!(%session, ?decision, "ignoring consent for a settled session");
                    return;
                }
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
                if accepted {
                    // Marked running straight away, so a second answer from
                    // the other surface no longer counts as pending.
                    if let Some(active) = self.active.as_mut() {
                        active.running = true;
                        self.active_revision += 1;
                    }
                } else {
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
                ui::confirm_clear_history(
                    &self.window,
                    self.visible_history().len(),
                    self.history_is_filtered(),
                    sender.clone(),
                );
            }

            Msg::ClearHistoryShown => {
                let shown: std::collections::HashSet<uuid::Uuid> = self
                    .visible_history()
                    .iter()
                    .map(|entry| entry.id)
                    .collect();
                self.history.retain(|entry| !shown.contains(&entry.id));
                self.history_revision += 1;
                history::save(&self.history_path, &self.history);
            }

            Msg::ClearHistoryAll => {
                self.history.clear();
                self.history_revision += 1;
                history::save(&self.history_path, &self.history);
            }

            Msg::PruneHistory => self.prune_missing_history(),

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
                // The engine owns the window and reports its deadline, so
                // nothing here touches the auto-off preference — the old
                // SetAutoOffMinutes overwrite left the domain stuck on the
                // temporary value for the life of the process.
                self.dispatch(Command::ReceiveTemporarily { minutes });
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
