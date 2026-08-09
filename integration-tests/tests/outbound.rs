//! Outbound (desktop → phone) domain flow with a fake front door.

use std::time::Duration;

use dh_core::limits::Limits;
use dh_domain::{
    spawn, Command, DomainConfig, EndReason, ErrorCode, Event, FrontDoorChannels, FrontDoorControl,
    FrontDoorSignal, SessionOutcome, Settings,
};
use tokio::sync::broadcast;
use tokio::time::timeout;

const STEP: Duration = Duration::from_secs(5);

async fn next_event(rx: &mut broadcast::Receiver<Event>) -> Event {
    timeout(STEP, rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("event stream closed")
}

fn test_config(dest: &std::path::Path, staging: &std::path::Path) -> DomainConfig {
    DomainConfig {
        settings: Settings::new("Test Mac", dest.to_path_buf(), staging.to_path_buf()),
        limits: Limits::default(),
    }
}

#[tokio::test]
async fn full_send_flow() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, mut control_rx, signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(test_config(dest.path(), staging.path()), channels);
    let mut events = handle.subscribe();

    // Discovery on → front door browses; UI learns about the phone.
    handle
        .send(Command::SetDiscovering(true))
        .await
        .expect("send");
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::StartDiscovery)
    );
    assert_eq!(
        next_event(&mut events).await,
        Event::DiscoveringChanged(true)
    );

    signal_tx
        .send(FrontDoorSignal::EndpointUpdated {
            endpoint: "ep-1".into(),
            name: "Pixel 8".into(),
            kind: "phone".into(),
            present: true,
        })
        .await
        .expect("signal");
    assert_eq!(
        next_event(&mut events).await,
        Event::EndpointUpdated {
            endpoint: "ep-1".into(),
            name: "Pixel 8".into(),
            kind: "phone".into(),
            present: true,
        }
    );

    // User sends two files.
    handle
        .send(Command::SendFiles {
            endpoint: "ep-1".into(),
            files: vec!["/tmp/a.jpg".into(), "/tmp/b.jpg".into()],
        })
        .await
        .expect("send");
    let session = match control_rx.recv().await {
        Some(FrontDoorControl::SendFiles {
            session,
            endpoint,
            files,
        }) => {
            assert_eq!(endpoint, "ep-1");
            assert_eq!(files.len(), 2);
            session
        }
        other => panic!("expected SendFiles control, got {other:?}"),
    };

    // Introduction delivered; phone user must accept.
    signal_tx
        .send(FrontDoorSignal::SendAwaitingConsent {
            session,
            total_bytes: 100,
            token: "4821".into(),
        })
        .await
        .expect("signal");
    assert_eq!(
        next_event(&mut events).await,
        Event::SendAwaitingConsent {
            session,
            total_bytes: 100,
            // The phone is showing this; the sending side has to be able to
            // show it too, or there is nothing to compare it against.
            token: "4821".into(),
        }
    );

    // Phone accepted: bytes start flowing; total comes from the consent step.
    signal_tx
        .send(FrontDoorSignal::Progress {
            session,
            bytes_received: 40,
            current_file: String::new(),
            files: vec![],
        })
        .await
        .expect("signal");
    assert_eq!(
        next_event(&mut events).await,
        Event::Progress {
            session,
            bytes_received: 40,
            total_bytes: 100,
            current_file: String::new(),
            files: vec![],
        }
    );

    signal_tx
        .send(FrontDoorSignal::Ended {
            session,
            reason: EndReason::Completed,
        })
        .await
        .expect("signal");
    assert_eq!(
        next_event(&mut events).await,
        Event::SessionEnded {
            session,
            outcome: SessionOutcome::Completed,
        }
    );
}

#[tokio::test]
async fn send_is_rejected_while_receiving() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, _control_rx, signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(test_config(dest.path(), staging.path()), channels);
    let mut events = handle.subscribe();

    // An inbound session is active.
    signal_tx
        .send(FrontDoorSignal::Connected {
            session: dh_domain::SessionId(1),
            token: "1234".into(),
        })
        .await
        .expect("signal");
    let _ = next_event(&mut events).await; // SessionConnected

    handle
        .send(Command::SendFiles {
            endpoint: "ep-1".into(),
            files: vec!["/tmp/a.jpg".into()],
        })
        .await
        .expect("send");
    match next_event(&mut events).await {
        Event::ErrorOccurred {
            code: ErrorCode::Busy,
            ..
        } => {}
        other => panic!("expected Busy error, got {other:?}"),
    }
}

#[tokio::test]
async fn empty_send_is_rejected() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, _control_rx, _signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(test_config(dest.path(), staging.path()), channels);
    let mut events = handle.subscribe();

    handle
        .send(Command::SendFiles {
            endpoint: "ep-1".into(),
            files: vec![],
        })
        .await
        .expect("send");
    match next_event(&mut events).await {
        Event::ErrorOccurred {
            code: ErrorCode::LimitsExceeded,
            ..
        } => {}
        other => panic!("expected LimitsExceeded, got {other:?}"),
    }
}
