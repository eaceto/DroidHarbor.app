//! The seam between the domain layer and a transfer front door.
//!
//! A front door (Quick Share today, QR+HTTP later) owns protocol, crypto and
//! sockets. It reports upward with [`FrontDoorSignal`]s and obeys
//! [`FrontDoorControl`]s. The domain layer is the only party that talks to
//! both the UI and the front door; the two never meet.
//!
//! Channels (not a trait object) keep the seam runtime-friendly, trivially
//! fakeable in tests, and identical in shape to a future IPC transport.

use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::types::{EndReason, FileOffer, FileProgress, SessionId};

/// Domain → front door.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontDoorControl {
    StartAdvertising {
        device_name: String,
    },
    StopAdvertising,
    /// Answer a pending introduction.
    Respond {
        session: SessionId,
        accept: bool,
    },
    /// Abort a session (user cancel, busy rejection, shutdown).
    Cancel {
        session: SessionId,
    },
    /// Start browsing for nearby receive-ready Android devices.
    StartDiscovery,
    StopDiscovery,
    /// Send files to a discovered endpoint under a pre-allocated session id.
    SendFiles {
        session: SessionId,
        endpoint: String,
        files: Vec<String>,
    },
    /// Send a text payload to a discovered endpoint. `kind` is
    /// "text" | "link" | "address" | "phone".
    SendText {
        session: SessionId,
        endpoint: String,
        kind: String,
        description: String,
        content: String,
    },
}

/// Front door → domain.
#[derive(Debug, Clone, PartialEq)]
pub enum FrontDoorSignal {
    /// Handshake completed; `token` is the 4-digit confirmation code.
    Connected { session: SessionId, token: String },
    /// Sender introduced a batch and awaits accept/decline.
    Introduction {
        session: SessionId,
        sender_name: String,
        files: Vec<FileOffer>,
        /// Title of a text, link or Wi-Fi payload; `None` for files. Such a
        /// transfer carries no files, so the size and count limits do not
        /// apply to it.
        text_preview: Option<String>,
        /// Declared batch total. Authoritative when the front door cannot
        /// report per-file sizes (Quick Share only exposes the batch total);
        /// `0` means "derive from the file list".
        total_bytes: u64,
    },
    /// Bytes landed in staging.
    Progress {
        session: SessionId,
        bytes_received: u64,
        current_file: String,
        /// Per-file detail when the front door reports it; may be empty.
        files: Vec<FileProgress>,
    },
    /// One payload is fully staged and verified; domain finalizes it.
    FileStaged {
        session: SessionId,
        staged_path: PathBuf,
        desired_name: String,
    },
    /// The session reached a terminal state.
    Ended {
        session: SessionId,
        reason: EndReason,
    },
    /// A nearby endpoint appeared or vanished during discovery.
    EndpointUpdated {
        endpoint: String,
        name: String,
        /// "phone" | "tablet" | "laptop" | "unknown", plain data so the
        /// contract survives FFI and IPC unchanged.
        kind: String,
        present: bool,
    },
    /// A text, link or Wi-Fi payload finished; there is nothing on disk.
    TextReceived {
        session: SessionId,
        /// "text" | "link" | "wifi"
        kind: String,
        description: String,
        content: String,
    },
    /// Outbound: the introduction was delivered; the phone's user must
    /// accept. Carries the batch total reported by the protocol layer, and
    /// the 4-digit confirmation token the phone is showing.
    SendAwaitingConsent {
        session: SessionId,
        total_bytes: u64,
        token: String,
    },
}

/// The channel pair a front door hands to [`crate::engine::spawn`].
pub struct FrontDoorChannels {
    pub control_tx: mpsc::Sender<FrontDoorControl>,
    pub signal_rx: mpsc::Receiver<FrontDoorSignal>,
}

impl FrontDoorChannels {
    /// Create a connected pair: the domain-side handles and the front-door
    /// side handles. Also used by tests to fake a front door.
    pub fn pair(
        buffer: usize,
    ) -> (
        Self,
        mpsc::Receiver<FrontDoorControl>,
        mpsc::Sender<FrontDoorSignal>,
    ) {
        let (control_tx, control_rx) = mpsc::channel(buffer);
        let (signal_tx, signal_rx) = mpsc::channel(buffer);
        (
            Self {
                control_tx,
                signal_rx,
            },
            control_rx,
            signal_tx,
        )
    }
}
