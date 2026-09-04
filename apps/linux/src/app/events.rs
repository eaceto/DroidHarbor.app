//! Translation of domain events into model state — the receiving half of the
//! conversation `component.rs` starts with its commands.

use std::time::{Duration, Instant};

use gtk4::prelude::*;
use relm4::ComponentSender;

use dh_domain::{Command, Event};

use super::model::{App, Endpoint, ReceivedText};
use super::Msg;
use crate::{format, history, platform, transfer};

const CONSENT_TIMEOUT: Duration = Duration::from_secs(60);

impl App {
    pub fn on_domain_event(&mut self, event: Event, sender: &ComponentSender<Self>) {
        match event {
            Event::AdvertisingChanged(on) => {
                // Every command is acknowledged, so this fires for redundant
                // ones too; only a real transition is worth a toast.
                let changed = self.receiving != on;
                self.receiving = on;
                self.sync_tray();
                if changed && !on {
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

            Event::FileFinalized { session, path } => {
                // The real landed path. The offered name is not it: the
                // finalizer sanitizes and appends ` (n)` on collision, and a
                // history row rebuilt from the offer then reveals the wrong
                // file — or gets pruned for pointing at nothing.
                if let Some(active) = self.active.as_mut().filter(|a| a.session == session) {
                    active.finalized_paths.push(path);
                }
            }

            Event::ReceivingUntilChanged { until_epoch_secs } => {
                self.receiving_until = until_epoch_secs;
            }

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
                        // Prefer the paths FileFinalized reported — those are
                        // where the files actually are, collision suffixes
                        // and all. The join is only the fallback for a
                        // session that ended before anything finalized.
                        let paths: Vec<String> = if active.finalized_paths.is_empty() {
                            active
                                .files
                                .iter()
                                .map(|file| self.destination.join(&file.name).display().to_string())
                                .collect()
                        } else {
                            active.finalized_paths.clone()
                        };
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
                        // The finalized path, for the same reason as the
                        // history record above.
                        active
                            .finalized_paths
                            .first()
                            .map(std::path::PathBuf::from)
                            .or_else(|| {
                                active
                                    .files
                                    .first()
                                    .map(|file| self.destination.join(&file.name))
                            })
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
