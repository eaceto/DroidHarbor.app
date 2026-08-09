//! The rqs_lib ↔ dh-domain adapter.
//!
//! Mapping summary (rqs_lib → domain):
//! - `State::WaitingForUserConsent` + metadata → `Connected` (with the
//!   4-digit PIN as the token) followed by `Introduction` (per-file names
//!   and sizes from `file_infos`).
//! - `State::ReceivingFiles` + metadata → `Progress` (acknowledged bytes,
//!   current file).
//! - `State::Finished` → one `FileStaged` per completed file, using the
//!   exact staged paths reported by rqs_lib, then `Ended(Completed)`.
//! - `State::Rejected` / `Cancelled` / `Disconnected` → `Ended` with the
//!   matching reason (consent timeouts map to `TimedOut`, disconnects carry
//!   the fork's error classification); staged leftovers are deleted.
//!
//! And (domain → rqs_lib): advertising toggles map to mDNS visibility;
//! `Respond`/`Cancel` become `ChannelMessage` actions on the shared broadcast
//! bus, filtered by session id inside rqs_lib.
//!
//! Each session's files land in their own staging subdirectory
//! (`with_session_subdirs`), so cleanup after a failed transfer is exact.

use std::collections::HashMap;
use std::path::PathBuf;

use rqs_lib::channel::{ChannelAction, ChannelDirection, ChannelMessage, TransferError};
use rqs_lib::{
    DeviceType, EndpointInfo, OutboundPayload, OutboundTextType, SendInfo, State, TextPayloadType,
    Visibility, RQS,
};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use dh_domain::{
    EndReason, FileOffer, FileProgress, FrontDoorChannels, FrontDoorControl, FrontDoorSignal,
    SessionId,
};

#[derive(Debug, thiserror::Error)]
pub enum FrontDoorError {
    #[error("failed to create staging directory: {0}")]
    Staging(#[from] std::io::Error),
    #[error("quick share service failed to start: {0}")]
    Service(String),
}

#[derive(Debug, Clone)]
pub struct QuickShareConfig {
    /// Application-owned directory where rqs_lib writes incoming files
    /// (one subdirectory per session). Cleared of leftovers on startup.
    pub staging_dir: PathBuf,
    /// Fixed TCP port; `None` picks an ephemeral one.
    pub port: Option<u16>,
    /// Name shown in the phone's share sheet; `None` advertises the system
    /// hostname. Fixed for the lifetime of the service (rename = restart).
    pub device_name: Option<String>,
    /// How long an unanswered transfer waits before it is auto-rejected.
    pub consent_timeout: std::time::Duration,
}

/// Start the Quick Share service (initially invisible) and the adapter task.
///
/// Returns the channels to hand to `dh_domain::spawn` plus the adapter's
/// join handle. The service stops when the domain side closes its control
/// channel (engine shutdown).
pub async fn spawn(
    config: QuickShareConfig,
) -> Result<(FrontDoorChannels, JoinHandle<()>), FrontDoorError> {
    std::fs::create_dir_all(&config.staging_dir)?;
    clear_directory(&config.staging_dir);

    let mut rqs = RQS::new(
        Visibility::Invisible,
        config.port.map(u32::from),
        Some(config.staging_dir.clone()),
    )
    .with_session_subdirs(true)
    .with_consent_timeout(config.consent_timeout);
    if let Some(name) = &config.device_name {
        rqs = rqs.with_device_name(name.clone());
    }
    let (send_tx, _ble_rx) = rqs
        .run()
        .await
        .map_err(|err| FrontDoorError::Service(err.to_string()))?;

    let (channels, control_rx, signal_tx) = FrontDoorChannels::pair(64);
    let message_rx = rqs.message_sender.subscribe();

    let adapter = Adapter {
        rqs,
        send_tx,
        staging_dir: config.staging_dir,
        signal_tx,
        sessions: HashMap::new(),
        next_session: 1,
        discovery_rx: None,
        endpoints: HashMap::new(),
        outbound: HashMap::new(),
    };
    let task = tokio::spawn(adapter.run(control_rx, message_rx));

    Ok((channels, task))
}

struct SessionEntry {
    id: SessionId,
    /// Set when the domain asked us to cancel, to attribute the terminal
    /// state to the right side.
    cancel_requested: bool,
}

struct OutboundEntry {
    id: SessionId,
    cancel_requested: bool,
}

struct Adapter {
    rqs: RQS,
    /// Feed for outbound transfers (consumed by rqs_lib's TCP server).
    send_tx: mpsc::Sender<SendInfo>,
    staging_dir: PathBuf,
    signal_tx: mpsc::Sender<FrontDoorSignal>,
    /// rqs_lib session id (remote address string) → our session state.
    sessions: HashMap<String, SessionEntry>,
    next_session: u64,
    /// Live while discovery is on.
    discovery_rx: Option<broadcast::Receiver<EndpointInfo>>,
    /// Endpoint id → last discovery info (needs ip/port to connect).
    endpoints: HashMap<String, EndpointInfo>,
    /// rqs_lib outbound session id (the peer address) → our session state.
    outbound: HashMap<String, OutboundEntry>,
}

impl Adapter {
    async fn run(
        mut self,
        mut control_rx: mpsc::Receiver<FrontDoorControl>,
        mut message_rx: broadcast::Receiver<ChannelMessage>,
    ) {
        loop {
            tokio::select! {
                control = control_rx.recv() => {
                    match control {
                        None => break, // domain engine shut down
                        Some(control) => self.handle_control(control).await,
                    }
                }
                message = message_rx.recv() => {
                    match message {
                        Ok(msg) if msg.direction == ChannelDirection::LibToFront => {
                            self.handle_message(msg).await;
                        }
                        Ok(_) => {} // our own FrontToLib actions echoing back
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("dropped {n} rqs_lib messages (lagged)");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                endpoint = recv_endpoint(&mut self.discovery_rx), if self.discovery_rx.is_some() => {
                    match endpoint {
                        Some(info) => self.on_endpoint(info).await,
                        None => self.discovery_rx = None,
                    }
                }
            }
        }
        tracing::info!("quick share front door stopping");
        self.rqs.stop().await;
    }

    async fn handle_control(&mut self, control: FrontDoorControl) {
        match control {
            FrontDoorControl::StartDiscovery => {
                if self.discovery_rx.is_none() {
                    let (tx, rx) = broadcast::channel(32);
                    match self.rqs.discovery(tx) {
                        Ok(()) => self.discovery_rx = Some(rx),
                        Err(err) => tracing::error!(%err, "failed to start discovery"),
                    }
                }
            }
            FrontDoorControl::StopDiscovery => {
                if self.discovery_rx.take().is_some() {
                    self.rqs.stop_discovery();
                }
                self.endpoints.clear();
            }
            FrontDoorControl::SendFiles {
                session,
                endpoint,
                files,
            } => {
                self.start_send(session, &endpoint, files).await;
            }
            FrontDoorControl::SendText {
                session,
                endpoint,
                kind,
                description,
                content,
            } => {
                self.start_send_text(session, &endpoint, &kind, description, content)
                    .await;
            }
            FrontDoorControl::StartAdvertising { device_name } => {
                // The advertised name is fixed at service construction
                // (QuickShareConfig::device_name); renaming needs a restart.
                tracing::info!(name = %device_name, "advertising enabled");
                self.set_visibility(Visibility::Visible);
            }
            FrontDoorControl::StopAdvertising => {
                self.set_visibility(Visibility::Invisible);
            }
            FrontDoorControl::Respond { session, accept } => {
                let action = if accept {
                    ChannelAction::AcceptTransfer
                } else {
                    ChannelAction::RejectTransfer
                };
                self.send_action(session, action);
            }
            FrontDoorControl::Cancel { session } => {
                if let Some(entry) = self.sessions.values_mut().find(|entry| entry.id == session) {
                    entry.cancel_requested = true;
                }
                if let Some(entry) = self.outbound.values_mut().find(|entry| entry.id == session) {
                    entry.cancel_requested = true;
                }
                self.send_action(session, ChannelAction::CancelTransfer);
            }
        }
    }

    async fn handle_message(&mut self, msg: ChannelMessage) {
        let Some(state) = msg.state.clone() else {
            return;
        };
        if self.outbound.contains_key(&msg.id) {
            self.handle_outbound_message(msg, state).await;
            return;
        }
        match state {
            State::WaitingForUserConsent => self.on_consent_requested(msg).await,
            State::ReceivingFiles => {
                if let (Some(entry), Some(meta)) = (self.sessions.get(&msg.id), &msg.meta) {
                    let current_file = meta
                        .file_infos
                        .as_ref()
                        .and_then(|infos| {
                            infos
                                .iter()
                                .find(|f| !f.completed && f.bytes_transferred > 0)
                                .or_else(|| infos.iter().find(|f| !f.completed))
                        })
                        .map(|f| f.name.clone())
                        .unwrap_or_default();
                    let files = meta
                        .file_infos
                        .as_ref()
                        .map(|infos| {
                            infos
                                .iter()
                                .map(|f| FileProgress {
                                    name: f.name.clone(),
                                    bytes_transferred: f.bytes_transferred,
                                    size: f.size,
                                    completed: f.completed,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    self.signal(FrontDoorSignal::Progress {
                        session: entry.id,
                        bytes_received: meta.ack_bytes,
                        current_file,
                        files,
                    })
                    .await;
                }
            }
            State::Finished => self.on_finished(msg).await,
            State::Rejected => {
                // A consent timeout surfaces as Rejected + ConsentTimeout.
                let reason = if msg.error == Some(TransferError::ConsentTimeout) {
                    EndReason::TimedOut
                } else {
                    EndReason::DeclinedByUser
                };
                self.on_failed(&msg.id, reason).await;
            }
            State::Cancelled => {
                let by_user = self
                    .sessions
                    .get(&msg.id)
                    .is_some_and(|entry| entry.cancel_requested);
                let reason = if by_user {
                    EndReason::CancelledByUser
                } else {
                    EndReason::CancelledBySender
                };
                self.on_failed(&msg.id, reason).await;
            }
            State::Disconnected => {
                let detail = match msg.error {
                    Some(TransferError::Io) => "connection lost",
                    Some(TransferError::Decode) => "protocol error",
                    Some(TransferError::ConsentTimeout) => "timed out",
                    Some(TransferError::Other) | None => "sender disconnected",
                };
                self.on_failed(&msg.id, EndReason::Failed(detail.into()))
                    .await;
            }
            // Handshake progress; nothing user-visible.
            other => tracing::trace!(session = %msg.id, state = ?other, "protocol state"),
        }
    }

    async fn on_consent_requested(&mut self, msg: ChannelMessage) {
        let Some(meta) = msg.meta else {
            tracing::warn!(session = %msg.id, "consent request without metadata; ignoring");
            return;
        };
        if self.sessions.contains_key(&msg.id) {
            return; // duplicate consent message
        }

        let id = SessionId(self.next_session);
        self.next_session += 1;

        let sender_name = meta
            .source
            .as_ref()
            .map(|source| source.name.clone())
            .unwrap_or_else(|| "Unknown device".into());
        let token = meta.pin_code.clone().unwrap_or_else(|| "????".into());
        // Text, links and Wi-Fi credentials arrive with no file list; the
        // title is all there is to show before the user accepts.
        let text_preview = meta
            .text_description
            .clone()
            .filter(|_| meta.files.as_ref().map(|f| f.is_empty()).unwrap_or(true));

        // Per-file details from the fork; fall back to bare names.
        let files: Vec<FileOffer> = match &meta.file_infos {
            Some(infos) => infos
                .iter()
                .map(|f| FileOffer {
                    name: f.name.clone(),
                    size: f.size,
                    mime_type: None,
                })
                .collect(),
            None => meta
                .files
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|name| FileOffer {
                    name,
                    size: 0,
                    mime_type: None,
                })
                .collect(),
        };

        self.sessions.insert(
            msg.id.clone(),
            SessionEntry {
                id,
                cancel_requested: false,
            },
        );
        // The previous session's directory can only be removed once the
        // domain has finished draining it, which happens after we emit
        // `Ended`; clean it up now that a new session is starting.
        self.sweep_stale_session_dirs(&msg.id);

        self.signal(FrontDoorSignal::Connected { session: id, token })
            .await;
        self.signal(FrontDoorSignal::Introduction {
            session: id,
            sender_name,
            files,
            total_bytes: meta.total_bytes,
            text_preview,
        })
        .await;
    }

    /// Completed transfer: hand every finished file to the domain using the
    /// exact staged path rqs_lib reported, under the sender's original name.
    async fn on_finished(&mut self, msg: ChannelMessage) {
        let Some(entry) = self.sessions.remove(&msg.id) else {
            return;
        };

        let infos = msg
            .meta
            .as_ref()
            .and_then(|meta| meta.file_infos.clone())
            .unwrap_or_default();
        let staged: Vec<_> = infos.iter().filter(|f| f.completed).collect();
        if staged.is_empty() {
            // No files: either a text/link/Wi-Fi payload, which the UI hands
            // to the clipboard, or metadata we could not read.
            let text = msg.meta.as_ref().and_then(|meta| {
                meta.text_payload.as_ref().map(|content| {
                    let kind = text_kind(meta.text_type.as_ref());
                    (
                        kind.to_string(),
                        meta.text_description.clone().unwrap_or_default(),
                        content.clone(),
                    )
                })
            });
            match text {
                Some((kind, description, content)) => {
                    self.signal(FrontDoorSignal::TextReceived {
                        session: entry.id,
                        kind,
                        description,
                        content,
                    })
                    .await;
                    self.signal(FrontDoorSignal::Ended {
                        session: entry.id,
                        reason: EndReason::Completed,
                    })
                    .await;
                }
                None => {
                    tracing::warn!(session = %msg.id, "transfer completed with nothing to save");
                    self.signal(FrontDoorSignal::Ended {
                        session: entry.id,
                        reason: EndReason::Failed("nothing was received".into()),
                    })
                    .await;
                }
            }
            let _ = std::fs::remove_dir_all(self.session_dir(&msg.id));
            return;
        }
        for info in staged {
            self.signal(FrontDoorSignal::FileStaged {
                session: entry.id,
                staged_path: PathBuf::from(&info.path),
                desired_name: info.name.clone(),
            })
            .await;
        }
        self.signal(FrontDoorSignal::Ended {
            session: entry.id,
            reason: EndReason::Completed,
        })
        .await;
        // The staging subdirectory is drained by the domain after these
        // signals, so it is removed by the next session's sweep or at
        // startup, not here, where it would still hold the files.
    }

    /// Terminal failure: delete the session's partial files and report why.
    async fn on_failed(&mut self, rqs_id: &str, reason: EndReason) {
        let Some(entry) = self.sessions.remove(rqs_id) else {
            return;
        };
        let dir = self.session_dir(rqs_id);
        if dir.exists() {
            if let Err(err) = std::fs::remove_dir_all(&dir) {
                tracing::warn!(path = %dir.display(), %err, "failed to delete partial files");
            }
        }
        self.signal(FrontDoorSignal::Ended {
            session: entry.id,
            reason,
        })
        .await;
    }

    /// Remove staging subdirectories from finished sessions, keeping the one
    /// currently in use.
    fn sweep_stale_session_dirs(&self, current_rqs_id: &str) {
        let keep = self.session_dir(current_rqs_id);
        let Ok(entries) = std::fs::read_dir(&self.staging_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path == keep || !path.is_dir() {
                continue;
            }
            if let Err(err) = std::fs::remove_dir_all(&path) {
                tracing::warn!(path = %path.display(), %err, "failed to remove stale session dir");
            }
        }
    }

    /// Mirror of rqs_lib's per-session subdirectory naming.
    fn session_dir(&self, rqs_id: &str) -> PathBuf {
        self.staging_dir.join(rqs_id.replace([':', '%', '/'], "-"))
    }

    async fn on_endpoint(&mut self, info: EndpointInfo) {
        let present = info.present.unwrap_or(true) && info.ip.is_some() && info.port.is_some();
        let name = info.name.clone().unwrap_or_else(|| "Android device".into());
        let kind = device_kind(info.rtype);
        let endpoint = info.id.clone();
        if present {
            self.endpoints.insert(endpoint.clone(), info);
        } else {
            self.endpoints.remove(&endpoint);
        }
        self.signal(FrontDoorSignal::EndpointUpdated {
            endpoint,
            name,
            kind,
            present,
        })
        .await;
    }

    async fn start_send(&mut self, session: SessionId, endpoint: &str, files: Vec<String>) {
        self.dispatch_send(session, endpoint, OutboundPayload::Files(files))
            .await;
    }

    async fn start_send_text(
        &mut self,
        session: SessionId,
        endpoint: &str,
        kind: &str,
        description: String,
        content: String,
    ) {
        let payload = OutboundPayload::Text {
            kind: outbound_text_type(kind),
            description,
            content,
        };
        self.dispatch_send(session, endpoint, payload).await;
    }

    /// Resolve the endpoint to an address and hand the payload to rqs_lib.
    /// Files and text differ only in what they carry, so both arrive here.
    async fn dispatch_send(
        &mut self,
        session: SessionId,
        endpoint: &str,
        payload: OutboundPayload,
    ) {
        let Some(info) = self.endpoints.get(endpoint) else {
            self.signal(FrontDoorSignal::Ended {
                session,
                reason: EndReason::Failed("device is no longer nearby".into()),
            })
            .await;
            return;
        };
        let (Some(ip), Some(port)) = (info.ip.clone(), info.port.clone()) else {
            self.signal(FrontDoorSignal::Ended {
                session,
                reason: EndReason::Failed("device has no reachable address".into()),
            })
            .await;
            return;
        };
        let addr = format!("{ip}:{port}");
        let name = info.name.clone().unwrap_or_else(|| "Android device".into());

        self.outbound.insert(
            addr.clone(),
            OutboundEntry {
                id: session,
                cancel_requested: false,
            },
        );
        let send = SendInfo {
            // rqs_lib keys outbound channel messages by this id; using the
            // peer address keeps every message (including the manager's
            // rtype-less Disconnected) routable back to this session.
            id: addr.clone(),
            name,
            addr,
            ob: payload,
        };
        if let Err(err) = self.send_tx.send(send).await {
            self.outbound.clear();
            self.signal(FrontDoorSignal::Ended {
                session,
                reason: EndReason::Failed(format!("send channel closed: {err}")),
            })
            .await;
        }
    }

    async fn handle_outbound_message(&mut self, msg: ChannelMessage, state: State) {
        let Some(entry) = self.outbound.get(&msg.id) else {
            return;
        };
        let session = entry.id;
        match state {
            State::SentIntroduction => {
                let total_bytes = msg.meta.as_ref().map(|m| m.total_bytes).unwrap_or(0);
                // The phone puts this on screen and asks its user to check
                // it against the sending device. It is derived during the
                // key exchange, which happens before the introduction, so it
                // is already on the metadata by the time this state arrives.
                let token = msg
                    .meta
                    .as_ref()
                    .and_then(|m| m.pin_code.clone())
                    .unwrap_or_default();
                self.signal(FrontDoorSignal::SendAwaitingConsent {
                    session,
                    total_bytes,
                    token,
                })
                .await;
            }
            State::SendingFiles => {
                if let Some(meta) = &msg.meta {
                    self.signal(FrontDoorSignal::Progress {
                        session,
                        bytes_received: meta.ack_bytes,
                        current_file: String::new(),
                        files: Vec::new(),
                    })
                    .await;
                }
            }
            State::Finished => {
                self.outbound.remove(&msg.id);
                self.signal(FrontDoorSignal::Ended {
                    session,
                    reason: EndReason::Completed,
                })
                .await;
            }
            State::Rejected => {
                self.outbound.remove(&msg.id);
                self.signal(FrontDoorSignal::Ended {
                    session,
                    reason: EndReason::DeclinedByUser,
                })
                .await;
            }
            State::Cancelled => {
                let by_user = self
                    .outbound
                    .remove(&msg.id)
                    .is_some_and(|entry| entry.cancel_requested);
                let reason = if by_user {
                    EndReason::CancelledByUser
                } else {
                    EndReason::CancelledBySender
                };
                self.signal(FrontDoorSignal::Ended { session, reason })
                    .await;
            }
            State::Disconnected => {
                self.outbound.remove(&msg.id);
                let detail = match msg.error {
                    Some(TransferError::Io) => "connection lost",
                    Some(TransferError::Decode) => "protocol error",
                    _ => "device disconnected",
                };
                self.signal(FrontDoorSignal::Ended {
                    session,
                    reason: EndReason::Failed(detail.into()),
                })
                .await;
            }
            other => tracing::trace!(session = %msg.id, state = ?other, "outbound protocol state"),
        }
    }

    fn send_action(&self, session: SessionId, action: ChannelAction) {
        let inbound = self
            .sessions
            .iter()
            .find(|(_, entry)| entry.id == session)
            .map(|(rqs_id, _)| rqs_id.clone());
        let Some(rqs_id) = inbound.or_else(|| {
            self.outbound
                .iter()
                .find(|(_, entry)| entry.id == session)
                .map(|(rqs_id, _)| rqs_id.clone())
        }) else {
            tracing::warn!(%session, "control for unknown session");
            return;
        };
        let message = ChannelMessage {
            id: rqs_id,
            direction: ChannelDirection::FrontToLib,
            action: Some(action),
            ..Default::default()
        };
        if self.rqs.message_sender.send(message).is_err() {
            tracing::error!("rqs_lib message bus closed");
        }
    }

    fn set_visibility(&self, visibility: Visibility) {
        match self.rqs.visibility_sender.lock() {
            Ok(sender) => {
                if sender.send(visibility).is_err() {
                    tracing::error!("rqs_lib visibility channel closed");
                }
            }
            Err(err) => tracing::error!(%err, "visibility sender poisoned"),
        }
    }

    async fn signal(&self, signal: FrontDoorSignal) {
        if self.signal_tx.send(signal).await.is_err() {
            tracing::warn!("domain signal channel closed");
        }
    }
}

/// Stable, UI-friendly name for a received text payload's type.
fn text_kind(text_type: Option<&TextPayloadType>) -> String {
    match text_type {
        Some(TextPayloadType::Url) => "link",
        Some(TextPayloadType::Wifi) => "wifi",
        Some(TextPayloadType::Text) | None => "text",
    }
    .to_string()
}

/// The reverse: what the UI asked us to send, as a protocol text type. The
/// phone uses it to decide what to offer (a browser, a map, or the dialer),
/// so an unrecognised name falls back to plain text rather than failing.
fn outbound_text_type(kind: &str) -> OutboundTextType {
    match kind {
        "link" => OutboundTextType::Url,
        "address" | "map" => OutboundTextType::Address,
        "phone" => OutboundTextType::PhoneNumber,
        _ => OutboundTextType::Text,
    }
}

/// Stable, UI-friendly name for a discovered device's type.
fn device_kind(rtype: Option<DeviceType>) -> String {
    match rtype {
        Some(DeviceType::Phone) => "phone",
        Some(DeviceType::Tablet) => "tablet",
        Some(DeviceType::Laptop) => "laptop",
        Some(DeviceType::Unknown) | None => "unknown",
    }
    .to_string()
}

/// Remove leftovers (crashed runs, undrained session dirs) from the staging
/// directory. Only ever called on our own application-owned staging dir.
fn clear_directory(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let result = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(err) = result {
            tracing::warn!(path = %path.display(), %err, "failed to clear staging leftover");
        }
    }
}

/// Await the next endpoint from an optional discovery subscription. Only
/// called when the receiver is `Some` (guarded in the select).
async fn recv_endpoint(rx: &mut Option<broadcast::Receiver<EndpointInfo>>) -> Option<EndpointInfo> {
    match rx.as_mut() {
        Some(rx) => loop {
            match rx.recv().await {
                Ok(info) => return Some(info),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        },
        None => std::future::pending().await,
    }
}
