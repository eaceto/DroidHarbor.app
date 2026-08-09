//! Plain-data types shared across the command/event boundary.
//!
//! FFI rule (spec §5.2): only strings, integers, booleans and lists; these
//! types must survive UniFFI and, later, IPC serialization unchanged.

/// Opaque identifier for one transfer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

/// One offered file as presented to the user in the accept dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOffer {
    pub name: String,
    /// Declared size in bytes; `0` when the front door only reports batch
    /// totals (Quick Share exposes per-batch, not per-file, sizes).
    pub size: u64,
    pub mime_type: Option<String>,
}

/// Live progress for one file inside a transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileProgress {
    pub name: String,
    pub bytes_transferred: u64,
    /// Declared size; `0` when the front door does not report one.
    pub size: u64,
    pub completed: bool,
}

/// Why a session ended, as reported by the front door.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndReason {
    Completed,
    DeclinedByUser,
    CancelledBySender,
    CancelledByUser,
    TimedOut,
    Failed(String),
}

/// User-facing outcome of a finished session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOutcome {
    Completed,
    Rejected,
    Cancelled,
    TimedOut,
    Failed,
}

impl From<&EndReason> for SessionOutcome {
    fn from(reason: &EndReason) -> Self {
        match reason {
            EndReason::Completed => SessionOutcome::Completed,
            EndReason::DeclinedByUser => SessionOutcome::Rejected,
            EndReason::CancelledBySender | EndReason::CancelledByUser => SessionOutcome::Cancelled,
            EndReason::TimedOut => SessionOutcome::TimedOut,
            EndReason::Failed(_) => SessionOutcome::Failed,
        }
    }
}

/// Stable error codes for UI mapping (UIs localize; codes never change).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Introduction violated limits (count/size/space).
    LimitsExceeded,
    /// A finalized file could not be placed in the destination.
    FinalizeFailed,
    /// Command referenced a session that is not active.
    UnknownSession,
    /// Command is not valid in the session's current phase.
    InvalidPhase,
    /// A second sender connected while a session was active.
    Busy,
}
