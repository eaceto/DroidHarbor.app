//! Commands: UI → domain (spec §5.2).

use crate::types::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Toggle receiving: starts/stops mDNS advertising via the front door.
    SetReceiving(bool),
    /// Rename the device as shown in the phone's share sheet.
    SetDeviceName(String),
    /// Change the destination folder (absolute path as a string, per the
    /// plain-data FFI rule).
    SetDestination(String),
    /// User accepted the pending introduction.
    Accept(SessionId),
    /// User declined the pending introduction.
    Decline(SessionId),
    /// User cancelled an in-flight transfer.
    Cancel(SessionId),
    /// Turn receiving off automatically after this many idle minutes;
    /// `0` disables the timer. Idle means advertising with no live transfer.
    SetAutoOffMinutes(u64),
    /// Turn receiving on for a fixed window, then off again. The engine owns
    /// the deadline and reports it via `ReceivingUntilChanged`, so every UI
    /// shows the same clock instead of racing its own timer against the idle
    /// one. A manual `SetReceiving` cancels the window; a transfer running
    /// when it lapses finishes first, and receiving stops after.
    ReceiveTemporarily { minutes: u64 },
    /// Toggle discovery of nearby Android devices (for sending). The phone
    /// must have its Quick Share screen open to be discoverable.
    SetDiscovering(bool),
    /// Send files (absolute paths) to a discovered endpoint.
    SendFiles {
        endpoint: String,
        files: Vec<String>,
    },
    /// Send a text payload to a discovered endpoint. `kind` is
    /// "text" | "link" | "address" | "phone" and decides what the phone
    /// offers to do with it; anything else is treated as plain text.
    /// `description` is the title shown while the phone asks to accept.
    SendText {
        endpoint: String,
        kind: String,
        description: String,
        content: String,
    },
    /// Stop advertising, abort any session, end the engine task.
    Shutdown,
}
