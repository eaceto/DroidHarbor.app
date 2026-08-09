//! Domain layer (spec §5.2): session orchestration and policy.
//!
//! The public surface is deliberately narrow: [`Command`]s in, [`Event`]s out,
//! and every payload is plain data, so the same contract works in-process
//! (Linux UI, tests), across UniFFI (Swift), and later across IPC when the
//! daemon split happens.
//!
//! Front doors (Quick Share today, QR+HTTP later) plug in underneath via
//! [`frontdoor`] channels; UIs never see protocol types.

pub mod command;
pub mod engine;
pub mod event;
pub mod frontdoor;
pub mod settings;
pub mod state;
pub mod types;

pub use command::Command;
pub use engine::{spawn, DomainConfig, DomainHandle};
pub use event::Event;
pub use frontdoor::{FrontDoorChannels, FrontDoorControl, FrontDoorSignal};
pub use settings::Settings;
pub use types::{EndReason, ErrorCode, FileOffer, FileProgress, SessionId, SessionOutcome};
