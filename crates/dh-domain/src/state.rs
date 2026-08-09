//! Domain-visible session phases and their legal transitions.
//!
//! This is the *user-facing* state machine; the finer protocol states
//! (handshaking, keepalive, …) live inside the front door. Illegal
//! transitions are bugs in a front door or a UI and are rejected loudly.

/// Phase of the single active session as the UIs see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    // Inbound (phone → desktop)
    /// Sender connected, handshake done, no introduction yet.
    Connected,
    /// Introduction shown; waiting for the user.
    AwaitingAccept,
    /// User accepted; payloads are arriving.
    Receiving,

    // Outbound (desktop → phone)
    /// Introduction sent; waiting for the phone's user to accept.
    AwaitingPeerAccept,
    /// Phone accepted; payloads are leaving.
    Sending,
}

impl Phase {
    /// Whether a session may move from `self` to `next`.
    pub fn can_transition_to(self, next: Phase) -> bool {
        matches!(
            (self, next),
            (Phase::Connected, Phase::AwaitingAccept)
                | (Phase::AwaitingAccept, Phase::Receiving)
                | (Phase::AwaitingPeerAccept, Phase::Sending)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Phase::*;

    #[test]
    fn legal_transitions() {
        assert!(Connected.can_transition_to(AwaitingAccept));
        assert!(AwaitingAccept.can_transition_to(Receiving));
        assert!(AwaitingPeerAccept.can_transition_to(Sending));
    }

    #[test]
    fn illegal_transitions() {
        assert!(!Connected.can_transition_to(Receiving));
        assert!(!Receiving.can_transition_to(AwaitingAccept));
        assert!(!Receiving.can_transition_to(Connected));
        assert!(!AwaitingAccept.can_transition_to(Connected));
        assert!(!Connected.can_transition_to(Connected));
        assert!(!AwaitingPeerAccept.can_transition_to(Receiving));
        assert!(!Sending.can_transition_to(AwaitingPeerAccept));
        assert!(!Connected.can_transition_to(Sending));
    }
}
