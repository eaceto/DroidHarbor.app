# dh-domain

Domain layer: session orchestration and policy. This crate's command/event
surface is **the only API a UI ever sees**: no protocol, crypto, or
filesystem types leak through it. Every payload is plain data (strings, ints,
bools, lists) so the same contract works in-process (Linux UI, tests), across
UniFFI (Swift), and later across IPC after the daemon split.

## API

- `spawn(DomainConfig, FrontDoorChannels) -> (DomainHandle, JoinHandle)`
  starts the engine: a single Tokio task owning all session state (no locks,
  total event order).
- `DomainHandle` is the cloneable UI-side handle:
  - `send(Command)`: `SetReceiving`, `SetDeviceName`, `SetDestination`,
    `Accept`, `Decline`, `Cancel`, `SetDiscovering`, `SendFiles`, `SendText`,
    `Shutdown`.
  - `subscribe() -> broadcast::Receiver<Event>`: `AdvertisingChanged`,
    `SessionConnected`, `IntroductionReceived` (files, total, 4-digit token),
    `Progress`, `FileFinalized`, `SessionEnded`, `ErrorOccurred`,
    `DiscoveringChanged`, `EndpointUpdated`, `SendAwaitingConsent`.
- `frontdoor` is the seam a transfer implementation plugs into:
  `FrontDoorChannels::pair()` gives typed channels; the front door receives
  `FrontDoorControl` (start/stop advertising, respond, cancel) and reports
  `FrontDoorSignal` (connected, introduction, progress, file staged, ended).
- `types`: `SessionId`, `FileOffer`, `SessionOutcome`, `EndReason`,
  stable `ErrorCode`s for UI localization.
- `state::Phase`: the user-visible session phases and legal transitions.
- `Settings`: device name, destination folder, staging directory.

Policy enforced here: single active session (`Busy` + cancel for a second
sender), limits auto-rejection at introduction time, phase validation on
every command/signal, atomic finalization via `dh-core`.
