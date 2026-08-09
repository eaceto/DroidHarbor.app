//! The domain engine: a single task owning all session state.
//!
//! UIs hold a [`DomainHandle`]; front doors hold the other end of
//! [`FrontDoorChannels`]. The engine is the only writer of session state, so
//! there are no locks and event order is total.

use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use dh_core::limits::Limits;

use crate::command::Command;
use crate::event::Event;
use crate::frontdoor::{FrontDoorChannels, FrontDoorControl, FrontDoorSignal};
use crate::settings::Settings;
use crate::state::Phase;
use crate::types::{ErrorCode, SessionId, SessionOutcome};

/// Capacity of the UI-facing event stream. A lagging UI drops old events
/// rather than blocking transfers.
const EVENT_CAPACITY: usize = 256;
const COMMAND_CAPACITY: usize = 32;
/// Outbound session ids live in the top half of the id space so they can
/// never collide with inbound ids allocated by the front door.
const OUTBOUND_SESSION_BASE: u64 = 1 << 63;

#[derive(Debug, Clone)]
pub struct DomainConfig {
    pub settings: Settings,
    pub limits: Limits,
}

/// The engine stopped (shutdown or crash); commands can no longer be sent.
#[derive(Debug, thiserror::Error)]
#[error("domain engine is not running")]
pub struct EngineStopped;

/// UI-side handle: send commands, subscribe to events. Cloneable.
#[derive(Clone)]
pub struct DomainHandle {
    cmd_tx: mpsc::Sender<Command>,
    event_tx: broadcast::Sender<Event>,
}

impl DomainHandle {
    pub async fn send(&self, command: Command) -> Result<(), EngineStopped> {
        self.cmd_tx.send(command).await.map_err(|_| EngineStopped)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }
}

/// Spawn the engine onto the current Tokio runtime.
pub fn spawn(config: DomainConfig, frontdoor: FrontDoorChannels) -> (DomainHandle, JoinHandle<()>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (event_tx, _) = broadcast::channel(EVENT_CAPACITY);

    let engine = Engine {
        config,
        receiving: false,
        discovering: false,
        next_outbound: OUTBOUND_SESSION_BASE,
        active: None,
        idle_deadline: None,
        event_tx: event_tx.clone(),
        control_tx: frontdoor.control_tx,
    };
    let task = tokio::spawn(engine.run(cmd_rx, frontdoor.signal_rx));

    (DomainHandle { cmd_tx, event_tx }, task)
}

struct ActiveSession {
    id: SessionId,
    phase: Phase,
    token: String,
    total_bytes: u64,
}

impl ActiveSession {
    /// Move to `next`, refusing illegal moves. A rejected transition means a
    /// front door or UI sent something out of order, which is a bug worth
    /// seeing rather than a state to silently accept.
    fn advance(&mut self, next: Phase) {
        if self.phase.can_transition_to(next) {
            self.phase = next;
        } else {
            tracing::error!(
                session = %self.id, from = ?self.phase, to = ?next,
                "refused an illegal phase transition"
            );
        }
    }
}

struct Engine {
    config: DomainConfig,
    receiving: bool,
    discovering: bool,
    next_outbound: u64,
    active: Option<ActiveSession>,
    /// When advertising should stop by itself; `None` while disabled, mid
    /// transfer, or not receiving.
    idle_deadline: Option<Instant>,
    event_tx: broadcast::Sender<Event>,
    control_tx: mpsc::Sender<FrontDoorControl>,
}

impl Engine {
    async fn run(
        mut self,
        mut cmd_rx: mpsc::Receiver<Command>,
        mut signal_rx: mpsc::Receiver<FrontDoorSignal>,
    ) {
        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        None | Some(Command::Shutdown) => {
                            self.shutdown().await;
                            return;
                        }
                        Some(cmd) => self.handle_command(cmd).await,
                    }
                }
                _ = tokio::time::sleep_until(
                    self.idle_deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(86_400))
                ), if self.idle_deadline.is_some() => {
                    self.idle_deadline = None;
                    if self.receiving && self.active.is_none() {
                        tracing::info!("turning receiving off after idle timeout");
                        self.receiving = false;
                        self.control(FrontDoorControl::StopAdvertising).await;
                        self.emit(Event::AdvertisingChanged(false));
                    }
                }
                signal = signal_rx.recv() => {
                    match signal {
                        None => {
                            // Front door died. Fail any active session and stop.
                            if let Some(session) = self.active.take() {
                                self.emit(Event::SessionEnded {
                                    session: session.id,
                                    outcome: SessionOutcome::Failed,
                                });
                            }
                            self.emit(Event::AdvertisingChanged(false));
                            return;
                        }
                        Some(signal) => self.handle_signal(signal).await,
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::SetReceiving(true) => {
                if !self.receiving {
                    self.receiving = true;
                    self.control(FrontDoorControl::StartAdvertising {
                        device_name: self.config.settings.device_name.clone(),
                    })
                    .await;
                    self.emit(Event::AdvertisingChanged(true));
                }
                self.rearm_idle_timer();
            }
            Command::SetReceiving(false) => {
                if self.receiving {
                    self.receiving = false;
                    self.control(FrontDoorControl::StopAdvertising).await;
                    self.emit(Event::AdvertisingChanged(false));
                }
                self.idle_deadline = None;
            }
            Command::SetDeviceName(name) => {
                self.config.settings.device_name = name.clone();
                if self.receiving {
                    // Re-advertise under the new name.
                    self.control(FrontDoorControl::StopAdvertising).await;
                    self.control(FrontDoorControl::StartAdvertising { device_name: name })
                        .await;
                }
            }
            Command::SetDestination(path) => {
                self.config.settings.destination = path.into();
            }
            Command::Accept(id) => {
                if self.check_phase(id, Phase::AwaitingAccept) {
                    if let Some(session) = self.active.as_mut() {
                        session.advance(Phase::Receiving);
                    }
                    self.control(FrontDoorControl::Respond {
                        session: id,
                        accept: true,
                    })
                    .await;
                }
            }
            Command::Decline(id) => {
                if self.check_phase(id, Phase::AwaitingAccept) {
                    // Session stays until the front door reports Ended.
                    self.control(FrontDoorControl::Respond {
                        session: id,
                        accept: false,
                    })
                    .await;
                }
            }
            Command::Cancel(id) => {
                if self.active.as_ref().is_some_and(|s| s.id == id) {
                    self.control(FrontDoorControl::Cancel { session: id }).await;
                } else {
                    self.emit_error(Some(id), ErrorCode::UnknownSession, "no such session");
                }
            }
            Command::SetAutoOffMinutes(minutes) => {
                self.config.limits.auto_off_minutes = minutes;
                self.rearm_idle_timer();
            }
            Command::SetDiscovering(true) => {
                if !self.discovering {
                    self.discovering = true;
                    self.control(FrontDoorControl::StartDiscovery).await;
                    self.emit(Event::DiscoveringChanged(true));
                }
            }
            Command::SetDiscovering(false) => {
                if self.discovering {
                    self.discovering = false;
                    self.control(FrontDoorControl::StopDiscovery).await;
                    self.emit(Event::DiscoveringChanged(false));
                }
            }
            Command::SendFiles { endpoint, files } => {
                if files.is_empty() {
                    self.emit_error(None, ErrorCode::LimitsExceeded, "no files to send");
                    return;
                }
                let Some(session) = self.begin_outbound() else {
                    return;
                };
                self.control(FrontDoorControl::SendFiles {
                    session,
                    endpoint,
                    files,
                })
                .await;
            }
            Command::SendText {
                endpoint,
                kind,
                description,
                content,
            } => {
                if content.is_empty() {
                    self.emit_error(None, ErrorCode::LimitsExceeded, "no text to send");
                    return;
                }
                let Some(session) = self.begin_outbound() else {
                    return;
                };
                self.control(FrontDoorControl::SendText {
                    session,
                    endpoint,
                    kind,
                    description,
                    content,
                })
                .await;
            }
            Command::Shutdown => unreachable!("handled in run()"),
        }
    }

    /// Claim the single session slot for an outbound transfer, or report why
    /// it cannot be claimed. Outbound ids come from their own counter so they
    /// never collide with the inbound ones the front door allocates.
    fn begin_outbound(&mut self) -> Option<SessionId> {
        if self.active.is_some() {
            self.emit_error(None, ErrorCode::Busy, "another transfer is active");
            return None;
        }
        let session = SessionId(self.next_outbound);
        self.next_outbound += 1;
        self.active = Some(ActiveSession {
            id: session,
            phase: Phase::AwaitingPeerAccept,
            token: String::new(),
            total_bytes: 0,
        });
        Some(session)
    }

    async fn handle_signal(&mut self, signal: FrontDoorSignal) {
        match signal {
            FrontDoorSignal::Connected { session, token } => {
                if self.active.is_some() {
                    // Single-session policy (spec §10).
                    self.emit_error(Some(session), ErrorCode::Busy, "another transfer is active");
                    self.control(FrontDoorControl::Cancel { session }).await;
                    return;
                }
                self.active = Some(ActiveSession {
                    id: session,
                    phase: Phase::Connected,
                    token,
                    total_bytes: 0,
                });
                self.idle_deadline = None;
                self.emit(Event::SessionConnected { session });
            }
            FrontDoorSignal::Introduction {
                session,
                sender_name,
                files,
                total_bytes: declared_total,
                text_preview,
            } => {
                if !self.check_phase(session, Phase::Connected) {
                    return;
                }
                let total_bytes: u64 = if declared_total > 0 {
                    declared_total
                } else {
                    files.iter().map(|f| f.size).sum()
                };
                // Text and links occupy no disk and have no file count,
                // so the batch limits simply do not apply to them.
                let admissible = if text_preview.is_some() {
                    Ok(())
                } else {
                    let available =
                        dh_core::space::available_space(&self.config.settings.destination);
                    self.config.limits.check_introduction(
                        files.len() as u64,
                        total_bytes,
                        available,
                    )
                };
                if let Err(err) = admissible {
                    self.emit_error(Some(session), ErrorCode::LimitsExceeded, &err.to_string());
                    self.control(FrontDoorControl::Respond {
                        session,
                        accept: false,
                    })
                    .await;
                    return;
                }
                let token = match self.active.as_mut() {
                    Some(active) => {
                        active.advance(Phase::AwaitingAccept);
                        active.total_bytes = total_bytes;
                        active.token.clone()
                    }
                    None => return,
                };
                self.emit(Event::IntroductionReceived {
                    session,
                    sender_name,
                    files,
                    total_bytes,
                    token,
                    text_preview,
                });
            }
            FrontDoorSignal::Progress {
                session,
                bytes_received,
                current_file,
                files,
            } => {
                if let Some(active) = self.active.as_mut().filter(|s| s.id == session) {
                    // First outbound progress means the phone accepted.
                    if active.phase == Phase::AwaitingPeerAccept {
                        active.advance(Phase::Sending);
                    }
                    let total_bytes = active.total_bytes;
                    self.emit(Event::Progress {
                        session,
                        bytes_received,
                        total_bytes,
                        current_file,
                        files,
                    });
                }
            }
            FrontDoorSignal::FileStaged {
                session,
                staged_path,
                desired_name,
            } => {
                if !self.check_phase(session, Phase::Receiving) {
                    return;
                }
                let dest = self.config.settings.destination.clone();
                tracing::info!(
                    staged = %staged_path.display(), destination = %dest.display(),
                    name = %desired_name, "finalizing file"
                );
                let result = tokio::task::spawn_blocking(move || {
                    dh_core::finalize::finalize_file(&staged_path, &dest, &desired_name)
                })
                .await;
                match result {
                    Ok(Ok(path)) => {
                        tracing::info!(path = %path.display(), exists = path.exists(), "file finalized");
                        self.emit(Event::FileFinalized {
                            session,
                            path: path.to_string_lossy().into_owned(),
                        })
                    }
                    Ok(Err(err)) => {
                        self.emit_error(Some(session), ErrorCode::FinalizeFailed, &err.to_string());
                        self.control(FrontDoorControl::Cancel { session }).await;
                    }
                    Err(join_err) => {
                        self.emit_error(
                            Some(session),
                            ErrorCode::FinalizeFailed,
                            &join_err.to_string(),
                        );
                        self.control(FrontDoorControl::Cancel { session }).await;
                    }
                }
            }
            FrontDoorSignal::Ended { session, reason } => {
                if self.active.as_ref().is_some_and(|s| s.id == session) {
                    self.active = None;
                    self.emit(Event::SessionEnded {
                        session,
                        outcome: (&reason).into(),
                    });
                    // The clock starts again once nothing is in flight.
                    self.rearm_idle_timer();
                }
            }
            FrontDoorSignal::EndpointUpdated {
                endpoint,
                name,
                kind,
                present,
            } => {
                self.emit(Event::EndpointUpdated {
                    endpoint,
                    name,
                    kind,
                    present,
                });
            }
            FrontDoorSignal::TextReceived {
                session,
                kind,
                description,
                content,
            } => {
                if self.active.as_ref().is_some_and(|s| s.id == session) {
                    self.emit(Event::TextReceived {
                        session,
                        kind,
                        description,
                        content,
                    });
                }
            }
            FrontDoorSignal::SendAwaitingConsent {
                session,
                total_bytes,
                token,
            } => {
                if let Some(active) = self.active.as_mut().filter(|s| s.id == session) {
                    active.total_bytes = total_bytes;
                    active.token = token.clone();
                    self.emit(Event::SendAwaitingConsent {
                        session,
                        total_bytes,
                        token,
                    });
                }
            }
        }
    }

    /// Restart the idle countdown, or clear it when it does not apply.
    fn rearm_idle_timer(&mut self) {
        let minutes = self.config.limits.auto_off_minutes;
        self.idle_deadline = if self.receiving && self.active.is_none() && minutes > 0 {
            Some(Instant::now() + Duration::from_secs(minutes * 60))
        } else {
            None
        };
    }

    /// Validate that `id` is the active session in `expected` phase; emits an
    /// error event otherwise.
    fn check_phase(&mut self, id: SessionId, expected: Phase) -> bool {
        match self.active.as_ref() {
            Some(active) if active.id == id && active.phase == expected => true,
            Some(active) if active.id == id => {
                self.emit_error(
                    Some(id),
                    ErrorCode::InvalidPhase,
                    &format!("expected {expected:?}, session is {:?}", active.phase),
                );
                false
            }
            _ => {
                self.emit_error(Some(id), ErrorCode::UnknownSession, "no such session");
                false
            }
        }
    }

    async fn shutdown(&mut self) {
        if let Some(session) = self.active.take() {
            self.control(FrontDoorControl::Cancel {
                session: session.id,
            })
            .await;
            self.emit(Event::SessionEnded {
                session: session.id,
                outcome: SessionOutcome::Cancelled,
            });
        }
        if self.receiving {
            self.control(FrontDoorControl::StopAdvertising).await;
            self.emit(Event::AdvertisingChanged(false));
        }
        if self.discovering {
            self.control(FrontDoorControl::StopDiscovery).await;
            self.emit(Event::DiscoveringChanged(false));
        }
    }

    async fn control(&self, control: FrontDoorControl) {
        if self.control_tx.send(control).await.is_err() {
            tracing::warn!("front door control channel closed");
        }
    }

    fn emit(&self, event: Event) {
        // Err just means no UI is subscribed right now; that's fine.
        let _ = self.event_tx.send(event);
    }

    fn emit_error(&self, session: Option<SessionId>, code: ErrorCode, message: &str) {
        self.emit(Event::ErrorOccurred {
            session,
            code,
            message: message.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    // The workspace warns on unwrap because production code should say what
    // it expects instead of panicking. A test is the opposite case: an
    // unwrap that fails is the assertion doing its job.
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::event::Event;
    use std::path::PathBuf;

    fn spawn_engine() -> (
        DomainHandle,
        mpsc::Receiver<FrontDoorControl>,
        mpsc::Sender<FrontDoorSignal>,
    ) {
        let (channels, control_rx, signal_tx) = FrontDoorChannels::pair(8);
        let config = DomainConfig {
            settings: Settings::new("Mac", PathBuf::from("/tmp"), PathBuf::from("/tmp/staging")),
            limits: Limits::default(),
        };
        let (handle, _task) = spawn(config, channels);
        (handle, control_rx, signal_tx)
    }

    #[tokio::test]
    async fn send_text_reaches_the_front_door() {
        let (handle, mut control_rx, _signal_tx) = spawn_engine();

        handle
            .send(Command::SendText {
                endpoint: "endpoint-1".into(),
                kind: "link".into(),
                description: "example.com".into(),
                content: "https://example.com".into(),
            })
            .await
            .unwrap();

        let control = control_rx.recv().await.unwrap();
        let FrontDoorControl::SendText {
            session,
            endpoint,
            kind,
            description,
            content,
        } = control
        else {
            panic!("expected SendText, got {control:?}");
        };
        // Outbound ids live in the top half of the space, away from the
        // inbound ids the front door allocates.
        assert!(session.0 >= OUTBOUND_SESSION_BASE);
        assert_eq!(endpoint, "endpoint-1");
        assert_eq!(kind, "link");
        assert_eq!(description, "example.com");
        assert_eq!(content, "https://example.com");
    }

    #[tokio::test]
    async fn empty_text_is_refused_before_reaching_the_front_door() {
        let (handle, mut control_rx, _signal_tx) = spawn_engine();
        let mut events = handle.subscribe();

        handle
            .send(Command::SendText {
                endpoint: "endpoint-1".into(),
                kind: "text".into(),
                description: String::new(),
                content: String::new(),
            })
            .await
            .unwrap();

        let event = events.recv().await.unwrap();
        assert!(
            matches!(event, Event::ErrorOccurred { code, .. } if code == ErrorCode::LimitsExceeded),
            "expected a limits error, got {event:?}"
        );
        assert!(control_rx.try_recv().is_err(), "nothing should be sent");
    }

    #[tokio::test]
    async fn a_second_send_is_refused_while_one_is_active() {
        let (handle, mut control_rx, _signal_tx) = spawn_engine();
        let mut events = handle.subscribe();

        let text = |content: &str| Command::SendText {
            endpoint: "endpoint-1".into(),
            kind: "text".into(),
            description: "note".into(),
            content: content.into(),
        };
        handle.send(text("first")).await.unwrap();
        control_rx.recv().await.unwrap();

        handle.send(text("second")).await.unwrap();
        let event = events.recv().await.unwrap();
        assert!(
            matches!(event, Event::ErrorOccurred { code, .. } if code == ErrorCode::Busy),
            "expected a busy error, got {event:?}"
        );
    }
}
