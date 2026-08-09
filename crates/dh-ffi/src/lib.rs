//! UniFFI boundary for the Swift (macOS) UI.
//!
//! Mirrors `dh-domain`'s command/event surface with FFI-friendly types
//! (plain records and enums, so the domain crate stays free of uniffi). Tokio
//! runs inside this library; Swift sees a constructor, plain methods, and a
//! callback interface it adapts into an `AsyncStream`.

use std::fs::OpenOptions;
use std::sync::Arc;

use dh_core::limits::Limits;
use dh_domain::{Command, DomainConfig, DomainHandle, Event, SessionId, Settings};
use dh_qs_core::QuickShareConfig;

uniffi::setup_scaffolding!();

/// Send Rust-side logs to ~/Library/Logs/DroidHarbor/droidharbor.log.
///
/// A GUI app has no console, so without this every warning from the protocol
/// and filesystem layers is invisible, including the ones that explain why a
/// file did not arrive.
fn init_logging() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let dir = std::path::Path::new(&home).join("Library/Logs/DroidHarbor");
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let Ok(file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("droidharbor.log"))
        else {
            return;
        };
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,rqs_lib=info".into());
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .try_init();
    });
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FileOffer {
    pub name: String,
    /// 0 when the front door only reports batch totals.
    pub size: u64,
    pub mime_type: Option<String>,
}

/// Live progress for one file inside a transfer.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FileProgress {
    pub name: String,
    pub bytes_transferred: u64,
    pub size: u64,
    pub completed: bool,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum SessionOutcome {
    Completed,
    Rejected,
    Cancelled,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum ErrorCode {
    LimitsExceeded,
    FinalizeFailed,
    UnknownSession,
    InvalidPhase,
    Busy,
}

/// One ordered stream of everything the UI needs to render.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum DHEvent {
    AdvertisingChanged {
        on: bool,
    },
    SessionConnected {
        session: u64,
    },
    IntroductionReceived {
        session: u64,
        sender_name: String,
        files: Vec<FileOffer>,
        total_bytes: u64,
        /// 4-digit confirmation code; must match the sender's screen.
        token: String,
        /// Set when the payload is text, a link or Wi-Fi credentials.
        text_preview: Option<String>,
    },
    Progress {
        session: u64,
        bytes_received: u64,
        total_bytes: u64,
        current_file: String,
        files: Vec<FileProgress>,
    },
    FileFinalized {
        session: u64,
        path: String,
    },
    SessionEnded {
        session: u64,
        outcome: SessionOutcome,
    },
    ErrorOccurred {
        session: Option<u64>,
        code: ErrorCode,
        message: String,
    },
    DiscoveringChanged {
        on: bool,
    },
    /// A nearby Android device appeared (`present: true`) or vanished.
    EndpointUpdated {
        endpoint: String,
        name: String,
        /// "phone" | "tablet" | "laptop" | "unknown".
        kind: String,
        present: bool,
    },
    /// Text, a link or Wi-Fi credentials arrived; nothing was written to
    /// disk and the UI decides what to do with the content.
    TextReceived {
        session: u64,
        /// "text" | "link" | "wifi"
        kind: String,
        description: String,
        content: String,
    },
    /// An outbound transfer awaits acceptance on the phone, which is
    /// showing `token` for the user to compare against this screen.
    SendAwaitingConsent {
        session: u64,
        total_bytes: u64,
        token: String,
    },
}

/// Implemented in Swift; called from a Rust worker thread, so hop to the main
/// actor on the Swift side before touching UI state.
#[uniffi::export(with_foreign)]
pub trait EventListener: Send + Sync {
    fn on_event(&self, event: DHEvent);
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum DHError {
    #[error("service failed: {message}")]
    Service { message: String },
    #[error("service is shut down")]
    Stopped,
}

/// The running receiver: front door + domain engine on an embedded runtime.
#[derive(uniffi::Object)]
pub struct DHService {
    runtime: tokio::runtime::Runtime,
    handle: DomainHandle,
}

#[uniffi::export]
impl DHService {
    /// Start the service (not yet advertising; call `set_receiving(true)`).
    ///
    /// `staging_dir` should live on the same filesystem as `destination` so
    /// finalization stays a pure rename.
    #[uniffi::constructor]
    pub fn start(
        destination: String,
        staging_dir: String,
        device_name: String,
        port: Option<u16>,
        enable_logging: bool,
    ) -> Result<Arc<Self>, DHError> {
        // Off unless the caller asks: the log names files and directories, so
        // a shipped build should not be writing it to disk at all.
        if enable_logging {
            init_logging();
            tracing::info!(%destination, %staging_dir, %device_name, "starting service");
        }
        let runtime = tokio::runtime::Runtime::new().map_err(|err| DHError::Service {
            message: err.to_string(),
        })?;

        let limits = Limits::default();
        let consent_timeout = std::time::Duration::from_secs(limits.accept_timeout_secs);
        let handle = runtime.block_on(async {
            let (channels, _frontdoor_task) = dh_qs_core::spawn(QuickShareConfig {
                staging_dir: staging_dir.clone().into(),
                port,
                device_name: Some(device_name.clone()),
                consent_timeout,
            })
            .await
            .map_err(|err| DHError::Service {
                message: err.to_string(),
            })?;

            let config = DomainConfig {
                settings: Settings::new(device_name, destination.into(), staging_dir.into()),
                limits,
            };
            let (handle, _engine_task) = dh_domain::spawn(config, channels);
            Ok::<_, DHError>(handle)
        })?;

        Ok(Arc::new(Self { runtime, handle }))
    }

    /// Register an event listener. Each listener gets every event from the
    /// moment of registration; events arrive on a Rust thread.
    pub fn add_listener(&self, listener: Arc<dyn EventListener>) {
        let mut events = self.handle.subscribe();
        self.runtime.spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => listener.on_event(convert_event(event)),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    pub fn set_receiving(&self, on: bool) -> Result<(), DHError> {
        self.send(Command::SetReceiving(on))
    }

    pub fn set_destination(&self, path: String) -> Result<(), DHError> {
        self.send(Command::SetDestination(path))
    }

    pub fn accept(&self, session: u64) -> Result<(), DHError> {
        self.send(Command::Accept(SessionId(session)))
    }

    pub fn decline(&self, session: u64) -> Result<(), DHError> {
        self.send(Command::Decline(SessionId(session)))
    }

    pub fn cancel(&self, session: u64) -> Result<(), DHError> {
        self.send(Command::Cancel(SessionId(session)))
    }

    /// Turn receiving off automatically after this many idle minutes;
    /// `0` keeps it on until the user says otherwise.
    pub fn set_auto_off_minutes(&self, minutes: u64) -> Result<(), DHError> {
        self.send(Command::SetAutoOffMinutes(minutes))
    }

    /// Toggle discovery of nearby Android devices (for sending). The phone
    /// must have its Quick Share screen open to be discoverable.
    pub fn set_discovering(&self, on: bool) -> Result<(), DHError> {
        self.send(Command::SetDiscovering(on))
    }

    /// Send files (absolute paths) to a discovered endpoint.
    pub fn send_files(&self, endpoint: String, files: Vec<String>) -> Result<(), DHError> {
        self.send(Command::SendFiles { endpoint, files })
    }

    /// Send text to a discovered endpoint. `kind` is
    /// "text" | "link" | "address" | "phone", and decides what the phone
    /// offers to do with it: open a browser, a map, or the dialer.
    /// `description` is the title shown while the phone asks to accept.
    pub fn send_text(
        &self,
        endpoint: String,
        kind: String,
        description: String,
        content: String,
    ) -> Result<(), DHError> {
        self.send(Command::SendText {
            endpoint,
            kind,
            description,
            content,
        })
    }

    /// Stop advertising, abort any transfer, and end the engine. The object
    /// is unusable afterwards.
    pub fn shutdown(&self) -> Result<(), DHError> {
        self.send(Command::Shutdown)
    }
}

impl DHService {
    fn send(&self, command: Command) -> Result<(), DHError> {
        self.runtime
            .block_on(self.handle.send(command))
            .map_err(|_| DHError::Stopped)
    }
}

fn convert_event(event: Event) -> DHEvent {
    match event {
        Event::AdvertisingChanged(on) => DHEvent::AdvertisingChanged { on },
        Event::SessionConnected { session } => DHEvent::SessionConnected { session: session.0 },
        Event::IntroductionReceived {
            session,
            sender_name,
            files,
            total_bytes,
            token,
            text_preview,
        } => DHEvent::IntroductionReceived {
            session: session.0,
            sender_name,
            files: files
                .into_iter()
                .map(|f| FileOffer {
                    name: f.name,
                    size: f.size,
                    mime_type: f.mime_type,
                })
                .collect(),
            total_bytes,
            token,
            text_preview,
        },
        Event::TextReceived {
            session,
            kind,
            description,
            content,
        } => DHEvent::TextReceived {
            session: session.0,
            kind,
            description,
            content,
        },
        Event::Progress {
            session,
            bytes_received,
            total_bytes,
            current_file,
            files,
        } => DHEvent::Progress {
            session: session.0,
            bytes_received,
            total_bytes,
            current_file,
            files: files
                .into_iter()
                .map(|f| FileProgress {
                    name: f.name,
                    bytes_transferred: f.bytes_transferred,
                    size: f.size,
                    completed: f.completed,
                })
                .collect(),
        },
        Event::FileFinalized { session, path } => DHEvent::FileFinalized {
            session: session.0,
            path,
        },
        Event::SessionEnded { session, outcome } => DHEvent::SessionEnded {
            session: session.0,
            outcome: match outcome {
                dh_domain::SessionOutcome::Completed => SessionOutcome::Completed,
                dh_domain::SessionOutcome::Rejected => SessionOutcome::Rejected,
                dh_domain::SessionOutcome::Cancelled => SessionOutcome::Cancelled,
                dh_domain::SessionOutcome::TimedOut => SessionOutcome::TimedOut,
                dh_domain::SessionOutcome::Failed => SessionOutcome::Failed,
            },
        },
        Event::DiscoveringChanged(on) => DHEvent::DiscoveringChanged { on },
        Event::EndpointUpdated {
            endpoint,
            name,
            kind,
            present,
        } => DHEvent::EndpointUpdated {
            endpoint,
            name,
            kind,
            present,
        },
        Event::SendAwaitingConsent {
            session,
            total_bytes,
            token,
        } => DHEvent::SendAwaitingConsent {
            session: session.0,
            total_bytes,
            token,
        },
        Event::ErrorOccurred {
            session,
            code,
            message,
        } => DHEvent::ErrorOccurred {
            session: session.map(|s| s.0),
            code: match code {
                dh_domain::ErrorCode::LimitsExceeded => ErrorCode::LimitsExceeded,
                dh_domain::ErrorCode::FinalizeFailed => ErrorCode::FinalizeFailed,
                dh_domain::ErrorCode::UnknownSession => ErrorCode::UnknownSession,
                dh_domain::ErrorCode::InvalidPhase => ErrorCode::InvalidPhase,
                dh_domain::ErrorCode::Busy => ErrorCode::Busy,
            },
            message,
        },
    }
}
