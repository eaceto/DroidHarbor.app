//! The relm4 application component, arranged by responsibility: the model
//! and its helpers, the message set, the component's init/update wiring, and
//! the translation of domain events into state. Widgets live in `ui`.

mod component;
mod events;
mod messages;
mod model;

pub use messages::Msg;
pub use model::{App, AppInit, Staged, AUTO_OFF_CHOICES};
