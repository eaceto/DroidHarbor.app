//! The shipping implementation: notification actions, XDG portals, background
//! portal.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use ashpd::desktop::background::{Background, BackgroundProxy, SetStatusOptions};
use notify_rust::{Hint, Notification, NotificationResponse, Timeout};
use tokio::runtime::Handle;

use super::Decision;
use crate::{uri, APP_ID};

/// Consent as a notification, even when the window is focused, so there is
/// exactly one consent path — the same choice the macOS app makes.
///
/// Three actions is the most GNOME will render; a fourth would be silently
/// dropped, so Accept / Always accept / Reject is the whole vocabulary.
///
/// Every outcome that is not an explicit accept resolves to `Reject`: a
/// dismissed notification, an expired one, a missing notification server. For
/// something that writes files to disk, silence must never mean yes.
///
/// `notify-rust` is synchronous and `wait_for_response` blocks until the user
/// answers, so each prompt owns a thread.
pub fn ask_consent(
    _window: &libadwaita::ApplicationWindow,
    summary: String,
    body: String,
    timeout: Duration,
    respond: impl FnOnce(Decision) + Send + 'static,
) {
    thread::spawn(move || {
        let shown = Notification::new()
            .appname("DroidHarbor")
            .summary(&summary)
            .body(&body)
            .icon("folder-download")
            .hint(Hint::DesktopEntry(APP_ID.into()))
            .action("accept", "Accept")
            .action("always", "Always accept")
            .action("reject", "Reject")
            // The server's own timer provides the expiry, so there is no
            // second timeout of ours to keep in sync with it.
            .timeout(Timeout::Milliseconds(timeout.as_millis() as u32))
            .show();

        let handle = match shown {
            Ok(handle) => handle,
            Err(error) => {
                tracing::error!(%error, "no notification server; rejecting");
                respond(Decision::Reject);
                return;
            }
        };

        let (tx, rx) = mpsc::channel();
        let waited = handle.wait_for_response(move |response: &NotificationResponse| {
            // Anything that is not an explicit acceptance is a rejection:
            // dismissed, expired, or a server that dropped the prompt.
            let decision = match response {
                NotificationResponse::Action(key) if key == "accept" => Decision::Accept,
                NotificationResponse::Action(key) if key == "always" => Decision::AcceptAlways,
                _ => Decision::Reject,
            };
            let _ = tx.send(decision);
        });

        if let Err(error) = waited {
            tracing::error!(%error, "notification server dropped the prompt; rejecting");
            respond(Decision::Reject);
            return;
        }

        respond(rx.recv().unwrap_or(Decision::Reject));
    });
}

/// Announce something that already happened.
///
/// The window can be closed while transfers continue, so without this a file
/// arriving while the app runs in the background is completely silent. A
/// `reveal` path adds a button that opens the file manager on it.
pub fn notify_done(summary: String, body: String, reveal: Option<PathBuf>, sound: bool) {
    thread::spawn(move || {
        let mut notification = Notification::new();
        notification
            .appname("DroidHarbor")
            .summary(&summary)
            .body(&body)
            .icon("folder-download")
            .hint(Hint::DesktopEntry(APP_ID.into()))
            .timeout(Timeout::Default);
        if sound {
            // The freedesktop sound naming spec; servers without it ignore it.
            notification.hint(Hint::SoundName("message-new-instant".into()));
        }
        if reveal.is_some() {
            notification.action("show", "Show");
        }

        let handle = match notification.show() {
            Ok(handle) => handle,
            Err(error) => {
                tracing::info!(%error, "could not post a notification");
                return;
            }
        };

        let Some(path) = reveal else {
            return;
        };
        // Blocks until the notification closes, which is why this owns a
        // thread of its own.
        let _ = handle.wait_for_response(move |response: &NotificationResponse| {
            if matches!(response, NotificationResponse::Action(key) if key == "show") {
                // A fresh connection rather than the app's runtime: this
                // thread outlives the notification and nothing else needs it.
                if let Err(error) = std::process::Command::new("xdg-open")
                    .arg(path.parent().unwrap_or(&path))
                    .spawn()
                {
                    tracing::info!(%error, "could not open the folder");
                }
            }
        });
    });
}

/// Folder picker through the XDG portal, which keeps working if the app is
/// ever run sandboxed. The portal answers with URIs, so this is where a URI
/// becomes a path.
pub fn choose_folder(
    _window: &libadwaita::ApplicationWindow,
    runtime: &Handle,
    respond: impl FnOnce(PathBuf) + Send + 'static,
) {
    runtime.spawn(async move {
        let chosen = ashpd::desktop::file_chooser::OpenFileRequest::default()
            .title("Choose where DroidHarbor saves files")
            .accept_label("Use this folder")
            .directory(true)
            .modal(false)
            .send()
            .await
            .and_then(|request| request.response());

        let uri = match chosen {
            Ok(files) => match files.uris().first() {
                Some(uri) => uri.as_str().to_string(),
                None => return,
            },
            // Cancelling is an ordinary outcome, not an error.
            Err(error) => {
                tracing::debug!(%error, "no folder chosen");
                return;
            }
        };

        match uri::file_uri_to_path(&uri) {
            Ok(path) => respond(path),
            Err(error) => tracing::warn!(%error, "portal returned an unusable location"),
        }
    });
}

/// Multi-file picker for outbound transfers.
///
/// Locations the portal can offer but the domain cannot open — `trash://`, a
/// remote share — are dropped rather than failing the whole selection, so
/// picking eight files and one odd one still sends the eight.
pub fn choose_files(
    _window: &libadwaita::ApplicationWindow,
    runtime: &Handle,
    respond: impl FnOnce(Vec<PathBuf>) + Send + 'static,
) {
    runtime.spawn(async move {
        let chosen = ashpd::desktop::file_chooser::OpenFileRequest::default()
            .title("Choose files to send")
            .accept_label("Send")
            .multiple(true)
            .modal(false)
            .send()
            .await
            .and_then(|request| request.response());

        let files = match chosen {
            Ok(files) => files,
            Err(error) => {
                tracing::debug!(%error, "no files chosen");
                return;
            }
        };

        let paths: Vec<PathBuf> = files
            .uris()
            .iter()
            .filter_map(|uri| match uri::file_uri_to_path(uri.as_str()) {
                Ok(path) => Some(path),
                Err(error) => {
                    tracing::warn!(%error, "skipping a location that is not a local file");
                    None
                }
            })
            .collect();

        if !paths.is_empty() {
            respond(paths);
        }
    });
}

/// What the tray menu can ask the application to do. Boxed rather than a
/// generic parameter so the tray's own type stays free of the UI's types.
pub struct TrayActions {
    pub set_receiving: Box<dyn Fn(bool) + Send + 'static>,
    /// Turn receiving on for a while, then let it lapse.
    pub receive_for: Box<dyn Fn(u64) + Send + 'static>,
    pub send_files: Box<dyn Fn() + Send + 'static>,
    pub send_clipboard: Box<dyn Fn() + Send + 'static>,
    pub choose_destination: Box<dyn Fn() + Send + 'static>,
    pub present: Box<dyn Fn() + Send + 'static>,
    pub about: Box<dyn Fn() + Send + 'static>,
    pub quit: Box<dyn Fn() + Send + 'static>,
}

/// The app's own icon as ARGB32 pixels, at two opacities.
///
/// The tray host resolves `icon_name` against the *system* icon theme, and an
/// AppImage has installed nothing there — which is why the icon fell back to a
/// generic network glyph. Handing over the pixels sidesteps the theme entirely.
struct TrayArtwork {
    lit: ksni::Icon,
    dim: ksni::Icon,
}

fn render_artwork() -> Option<TrayArtwork> {
    use gtk4::gdk_pixbuf::Pixbuf;

    const SIZE: i32 = 64;
    let pixbuf = Pixbuf::from_read(std::io::Cursor::new(ICON_PNG)).ok()?;
    let pixbuf = pixbuf
        .scale_simple(SIZE, SIZE, gtk4::gdk_pixbuf::InterpType::Bilinear)
        .unwrap_or(pixbuf);

    let channels = pixbuf.n_channels() as usize;
    let stride = pixbuf.rowstride() as usize;
    let width = pixbuf.width();
    let height = pixbuf.height();
    // SAFETY: the pixbuf is alive for the length of this borrow and is not
    // mutated while the bytes are copied out.
    let pixels = unsafe { pixbuf.pixels() };

    let convert = |fade: bool| {
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height as usize {
            for x in 0..width as usize {
                let offset = y * stride + x * channels;
                let alpha = if channels == 4 {
                    pixels[offset + 3]
                } else {
                    255
                };
                let alpha = if fade {
                    (alpha as u16 * 45 / 100) as u8
                } else {
                    alpha
                };
                // ksni wants ARGB32 in network byte order.
                data.push(alpha);
                data.push(pixels[offset]);
                data.push(pixels[offset + 1]);
                data.push(pixels[offset + 2]);
            }
        }
        ksni::Icon {
            width,
            height,
            data,
        }
    };

    Some(TrayArtwork {
        lit: convert(false),
        dim: convert(true),
    })
}

struct TrayState {
    receiving: bool,
    /// What is happening right now, shown as a disabled first line the way the
    /// macOS menu shows its transfer row.
    status: Option<String>,
    destination: String,
    artwork: Option<TrayArtwork>,
    actions: TrayActions,
}

impl ksni::Tray for TrayState {
    fn id(&self) -> String {
        APP_ID.into()
    }

    fn title(&self) -> String {
        "DroidHarbor".into()
    }

    /// Empty whenever artwork is available.
    ///
    /// Hosts such as GNOME's AppIndicator prefer `IconName` over `IconPixmap`
    /// and will happily render a themed glyph while ignoring the pixels we
    /// supplied — which is why the tray kept showing a network icon instead of
    /// the app's own. Withholding the name leaves the pixmap as the only
    /// option.
    fn icon_name(&self) -> String {
        if self.artwork.is_some() {
            return String::new();
        }
        // Only reached when the icon could not be decoded. Sharing, not
        // networking: the app is about handing files between two devices.
        if self.receiving {
            "folder-download-symbolic".into()
        } else {
            "emblem-shared-symbolic".into()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let Some(artwork) = &self.artwork else {
            return Vec::new();
        };
        // Dimmed while idle, so the tray says at a glance whether the machine
        // is visible to nearby phones.
        let icon = if self.receiving {
            &artwork.lit
        } else {
            &artwork.dim
        };
        vec![icon.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "DroidHarbor".into(),
            description: if self.receiving {
                "Visible to nearby Android devices".into()
            } else {
                "Receiving is off".into()
            },
            ..Default::default()
        }
    }

    /// Clicking the icon brings the window back, which is the gesture people
    /// try first when an app has disappeared into the tray.
    fn activate(&mut self, _x: i32, _y: i32) {
        (self.actions.present)();
    }

    fn watcher_online(&self) {
        tracing::info!("a tray host appeared; the icon is now visible");
    }

    /// Keep waiting rather than giving up: on GNOME the AppIndicator extension
    /// can be enabled, or the shell restarted, long after the app started.
    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        tracing::info!(
            ?reason,
            "tray host went away; will re-register if it returns"
        );
        true
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};

        let mut items: Vec<ksni::MenuItem<Self>> = Vec::new();

        // Whatever is in flight goes first and is not clickable, mirroring the
        // transfer row at the top of the macOS menu.
        if let Some(status) = &self.status {
            items.push(
                StandardItem {
                    label: status.clone(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            items.push(ksni::MenuItem::Separator);
        }

        items.push(
            CheckmarkItem {
                label: "Receiving".into(),
                checked: self.receiving,
                activate: Box::new(|this: &mut Self| {
                    // Report the intent and let the domain's event settle the
                    // state, so the tick never disagrees with the window.
                    (this.actions.set_receiving)(!this.receiving);
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            SubMenu {
                label: "Receive for a while".into(),
                submenu: [10u64, 30, 60]
                    .into_iter()
                    .map(|minutes| {
                        StandardItem {
                            label: if minutes < 60 {
                                format!("{minutes} minutes")
                            } else {
                                "1 hour".to_string()
                            },
                            activate: Box::new(move |this: &mut Self| {
                                (this.actions.receive_for)(minutes)
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
        );

        items.push(ksni::MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Send files…".into(),
                icon_name: "document-send-symbolic".into(),
                activate: Box::new(|this: &mut Self| (this.actions.send_files)()),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Send clipboard text".into(),
                icon_name: "edit-paste-symbolic".into(),
                activate: Box::new(|this: &mut Self| (this.actions.send_clipboard)()),
                ..Default::default()
            }
            .into(),
        );

        items.push(ksni::MenuItem::Separator);
        items.push(
            StandardItem {
                label: format!("Save to: {}", self.destination),
                icon_name: "folder-symbolic".into(),
                activate: Box::new(|this: &mut Self| (this.actions.choose_destination)()),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Open DroidHarbor…".into(),
                icon_name: "window-new-symbolic".into(),
                activate: Box::new(|this: &mut Self| (this.actions.present)()),
                ..Default::default()
            }
            .into(),
        );

        items.push(ksni::MenuItem::Separator);
        items.push(
            StandardItem {
                label: "About DroidHarbor".into(),
                icon_name: "help-about-symbolic".into(),
                activate: Box::new(|this: &mut Self| (this.actions.about)()),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|this: &mut Self| (this.actions.quit)()),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

/// A live tray icon. Absent on desktops with no StatusNotifierItem host, which
/// is the normal case on vanilla GNOME and must never be treated as an error.
pub struct Tray(ksni::Handle<TrayState>);

impl std::fmt::Debug for Tray {
    // relm4 requires Debug on messages; ksni's handle has none, and there is
    // nothing about it worth printing anyway.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tray")
    }
}

/// Try to show a tray icon.
///
/// Returns immediately; the result arrives through `on_ready` because
/// registration is asynchronous and may fail for reasons that are not failures
/// of the app — most often that the desktop simply has no tray.
pub fn start_tray(
    runtime: &Handle,
    destination: String,
    actions: TrayActions,
    on_ready: impl FnOnce(Option<Tray>) + Send + 'static,
) {
    use ksni::TrayMethods;

    // Decoded here, on the caller's thread, because a Pixbuf cannot cross into
    // the tray task — only the finished bytes can.
    let artwork = render_artwork();
    if artwork.is_none() {
        tracing::warn!("could not decode the tray icon; falling back to a themed one");
    }

    runtime.spawn(async move {
        let state = TrayState {
            receiving: false,
            status: None,
            destination,
            artwork,
            actions,
        };
        // `assume_sni_available` keeps a missing watcher a soft error: the item
        // is created and appears if a host shows up later. Without it, starting
        // before the shell's extension finished loading meant no tray for the
        // rest of the session, which is what "no tray on this desktop" was
        // really reporting.
        match state.assume_sni_available(true).spawn().await {
            Ok(handle) => {
                tracing::info!("tray registered (it appears once a host exists)");
                on_ready(Some(Tray(handle)));
            }
            Err(error) => {
                tracing::info!(%error, "no tray available; the window is the only surface");
                on_ready(None);
            }
        }
    });
}

pub fn update_tray(
    runtime: &Handle,
    tray: &Tray,
    receiving: bool,
    status: Option<String>,
    destination: String,
) {
    let handle = tray.0.clone();
    runtime.spawn(async move {
        handle
            .update(move |state: &mut TrayState| {
                state.receiving = receiving;
                state.status = status;
                state.destination = destination;
            })
            .await;
    });
}

/// Show a file in the file manager, selected rather than opened.
///
/// Nautilus and Dolphin both implement `FileManager1`; where nothing does, the
/// loss is cosmetic, so this never surfaces an error to the user.
pub fn reveal(runtime: &Handle, path: PathBuf) {
    runtime.spawn(async move {
        let uri = format!("file://{}", path.display());
        let call = async {
            zbus::Connection::session()
                .await?
                .call_method(
                    Some("org.freedesktop.FileManager1"),
                    "/org/freedesktop/FileManager1",
                    Some("org.freedesktop.FileManager1"),
                    "ShowItems",
                    &(vec![uri.as_str()], ""),
                )
                .await
        }
        .await;

        if let Err(error) = call {
            tracing::info!(%error, "no file manager answered; nothing revealed");
        }
    });
}

/// The icon, carried in the binary rather than looked up on disk.
///
/// An AppImage has no install step, so at the moment the user asks for a
/// desktop entry there is no icon anywhere in the icon theme to point at, and
/// the tray host has nothing to resolve a name against. The only copy
/// guaranteed to exist is the one compiled in.
///
/// Taken from `apps/macos/Resources/AppIcon.icns`, the artwork that actually
/// ships. `assets/icons/DroidHarbor.iconset/` holds an older concept and is not
/// what the Mac app uses.
const ICON_PNG: &[u8] = include_bytes!("../../Resources/icons/icon_512.png");

/// The sizes installed into the icon theme, so each surface picks one rendered
/// at its size instead of downscaling a 512 and blurring it.
const ICON_SIZES: [(u32, &[u8]); 6] = [
    (16, include_bytes!("../../Resources/icons/icon_16.png")),
    (24, include_bytes!("../../Resources/icons/icon_24.png")),
    (32, include_bytes!("../../Resources/icons/icon_32.png")),
    (48, include_bytes!("../../Resources/icons/icon_48.png")),
    (128, include_bytes!("../../Resources/icons/icon_128.png")),
    (256, include_bytes!("../../Resources/icons/icon_256.png")),
];

/// Put DroidHarbor in the application launcher.
///
/// AppImages are run, not installed, so nothing registers them with the
/// desktop. This writes the entry and icon into the per-user XDG directories,
/// which needs no privileges and is undone by deleting two files.
pub fn install_desktop_entry() -> anyhow::Result<PathBuf> {
    let executable = std::env::current_exe()?;

    let theme = data_home().join("icons").join("hicolor");
    for (size, bytes) in ICON_SIZES.iter().chain(&[(512u32, ICON_PNG)]) {
        let dir = theme.join(format!("{size}x{size}")).join("apps");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{APP_ID}.png")), bytes)?;
    }

    let applications = data_home().join("applications");
    std::fs::create_dir_all(&applications)?;
    let entry_path = applications.join(format!("{APP_ID}.desktop"));
    std::fs::write(
        &entry_path,
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=DroidHarbor\n\
             GenericName=File Transfer\n\
             Comment=Receive files from Android's built-in sharing\n\
             Exec=\"{}\"\n\
             Icon={APP_ID}\n\
             Terminal=false\n\
             Categories=Network;FileTransfer;GTK;\n\
             Keywords=android;quick share;nearby;transfer;\n\
             StartupNotify=true\n\
             StartupWMClass={APP_ID}\n",
            executable.display()
        ),
    )?;

    // Without this the launcher may not notice the new entry until the next
    // login. A missing update-desktop-database is not fatal.
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&applications)
        .status();

    Ok(entry_path)
}

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
        .unwrap_or_else(std::env::temp_dir)
}

/// The XDG autostart entry. An AppImage is never installed, so this records the
/// absolute path of the running binary — which the user can later move or
/// delete, hence [`launch_at_login_is_healthy`].
pub fn set_launch_at_login(enabled: bool) -> anyhow::Result<()> {
    let path = autostart_path();
    if !enabled {
        match std::fs::remove_file(&path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }

    let executable = std::env::current_exe()?;
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=DroidHarbor\n\
         Comment=Receive files from Android's built-in sharing\n\
         Exec=\"{}\"\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         Categories=Network;FileTransfer;\n\
         X-GNOME-Autostart-enabled=true\n",
        executable.display()
    );

    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)?;
    }
    std::fs::write(&path, entry)?;
    Ok(())
}

/// Whether the autostart entry still points at a binary that exists. An
/// AppImage that was moved or deleted leaves an entry that silently does
/// nothing, which is worse than not having one.
pub fn launch_at_login_is_healthy() -> bool {
    let Ok(entry) = std::fs::read_to_string(autostart_path()) else {
        return false;
    };
    entry
        .lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .map(|command| PathBuf::from(command.trim().trim_matches('"')).exists())
        .unwrap_or(false)
}

pub fn launch_at_login_enabled() -> bool {
    autostart_path().exists()
}

fn autostart_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("autostart").join(format!("{APP_ID}.desktop"))
}

/// Ask to keep running with the window closed, then advertise a status line.
///
/// `set_status` is what puts the app in GNOME's **Background Apps** menu, which
/// is the presence and quit affordance on a desktop with no tray — the thing
/// that makes the tray genuinely optional rather than merely absent.
///
/// `auto_start` stays false: launch-at-login is an explicit Settings choice,
/// not something acquired as a side effect of closing a window.
pub fn request_background(runtime: &Handle, status: String, need_permission: bool) {
    runtime.spawn(async move {
        if need_permission {
            let granted = Background::request()
                .reason("Keep receiving files while the window is closed")
                .auto_start(false)
                .send()
                .await
                .and_then(|request| request.response());

            match granted {
                Ok(response) if response.run_in_background() => {}
                Ok(_) => {
                    tracing::info!("background running was declined");
                    return;
                }
                Err(error) => {
                    tracing::debug!(%error, "no background portal");
                    return;
                }
            }
        }

        // Best effort: a desktop without this portal version simply shows
        // nothing, which is not worth failing over.
        let set = async {
            BackgroundProxy::new()
                .await?
                .set_status(SetStatusOptions::default().set_message(&status))
                .await
        }
        .await;

        if let Err(error) = set {
            tracing::debug!(%error, "could not set background status");
        }
    });
}
