//! Events: domain → UI (spec §5.2). One ordered stream; plain data only.

use crate::types::{ErrorCode, FileOffer, FileProgress, SessionId, SessionOutcome};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Advertising started or stopped (mirrors `SetReceiving` and auto-off).
    AdvertisingChanged(bool),
    /// A sender connected and the handshake completed.
    SessionConnected { session: SessionId },
    /// The sender introduced a batch; UI must show the accept dialog.
    IntroductionReceived {
        session: SessionId,
        sender_name: String,
        files: Vec<FileOffer>,
        total_bytes: u64,
        /// 4-digit confirmation token; must match the sender's screen.
        token: String,
        /// Set when the transfer is a text, link or Wi-Fi payload rather
        /// than files.
        text_preview: Option<String>,
    },
    /// Transfer progress for the active session.
    Progress {
        session: SessionId,
        bytes_received: u64,
        total_bytes: u64,
        current_file: String,
        /// Per-file detail when available; may be empty.
        files: Vec<FileProgress>,
    },
    /// One file was atomically placed in the destination.
    FileFinalized { session: SessionId, path: String },
    /// The session reached a terminal state.
    SessionEnded {
        session: SessionId,
        outcome: SessionOutcome,
    },
    /// A recoverable or session-fatal error the UI should surface.
    ErrorOccurred {
        session: Option<SessionId>,
        code: ErrorCode,
        message: String,
    },
    /// Endpoint discovery started or stopped (mirrors `SetDiscovering`).
    DiscoveringChanged(bool),
    /// A nearby Android device appeared (`present: true`) or vanished.
    EndpointUpdated {
        endpoint: String,
        name: String,
        /// "phone" | "tablet" | "laptop" | "unknown".
        kind: String,
        present: bool,
    },
    /// A text, link or Wi-Fi payload arrived. Nothing was written to disk;
    /// the UI decides what to do with the content.
    TextReceived {
        session: SessionId,
        /// "text" | "link" | "wifi"
        kind: String,
        description: String,
        content: String,
    },
    /// An outbound transfer was introduced to the phone; its user must
    /// accept there. `total_bytes` covers the whole batch.
    SendAwaitingConsent {
        session: SessionId,
        total_bytes: u64,
        /// 4-digit confirmation token, the same one the phone is showing.
        /// Both ends derive it from the key exchange and the phone asks its
        /// user to compare the two, so the sending side has to be able to
        /// show it. Empty when the protocol layer reported none, which the
        /// UI draws as no code rather than as one that could never match.
        token: String,
    },
}
