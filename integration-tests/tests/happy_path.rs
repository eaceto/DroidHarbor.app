//! End-to-end domain-layer test with a fake front door: the exact interaction
//! a real UI + real Quick Share front door will have, minus sockets.

use std::fs;
use std::time::Duration;

use dh_core::limits::Limits;
use dh_domain::{
    spawn, Command, DomainConfig, EndReason, ErrorCode, Event, FileOffer, FrontDoorChannels,
    FrontDoorControl, FrontDoorSignal, SessionId, SessionOutcome, Settings,
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
async fn full_receive_flow() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, mut control_rx, signal_tx) = FrontDoorChannels::pair(16);
    let (handle, task) = spawn(test_config(dest.path(), staging.path()), channels);
    let mut events = handle.subscribe();

    // UI turns receiving on → front door is told to advertise.
    handle
        .send(Command::SetReceiving(true))
        .await
        .expect("send");
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::StartAdvertising {
            device_name: "Test Mac".into()
        })
    );
    assert_eq!(
        next_event(&mut events).await,
        Event::AdvertisingChanged(true)
    );

    // Phone connects and introduces two files.
    let session = SessionId(1);
    signal_tx
        .send(FrontDoorSignal::Connected {
            session,
            token: "4821".into(),
        })
        .await
        .expect("signal");
    assert_eq!(
        next_event(&mut events).await,
        Event::SessionConnected { session }
    );

    let files = vec![
        FileOffer {
            name: "photo.jpg".into(),
            size: 5,
            mime_type: Some("image/jpeg".into()),
        },
        FileOffer {
            name: "notes.txt".into(),
            size: 3,
            mime_type: None,
        },
    ];
    signal_tx
        .send(FrontDoorSignal::Introduction {
            session,
            sender_name: "Pixel 9".into(),
            files: files.clone(),
            total_bytes: 0,
            text_preview: None,
        })
        .await
        .expect("signal");

    match next_event(&mut events).await {
        Event::IntroductionReceived {
            session: s,
            sender_name,
            files: offered,
            total_bytes,
            token,
            ..
        } => {
            assert_eq!(s, session);
            assert_eq!(sender_name, "Pixel 9");
            assert_eq!(offered, files);
            assert_eq!(total_bytes, 8);
            assert_eq!(token, "4821");
        }
        other => panic!("expected IntroductionReceived, got {other:?}"),
    }

    // UI accepts → front door told to respond.
    handle.send(Command::Accept(session)).await.expect("send");
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::Respond {
            session,
            accept: true
        })
    );

    // First file arrives: progress, then staged, then finalized by the domain.
    signal_tx
        .send(FrontDoorSignal::Progress {
            session,
            bytes_received: 5,
            current_file: "photo.jpg".into(),
            files: vec![],
        })
        .await
        .expect("signal");
    assert_eq!(
        next_event(&mut events).await,
        Event::Progress {
            session,
            bytes_received: 5,
            total_bytes: 8,
            current_file: "photo.jpg".into(),
            files: vec![],
        }
    );

    let staged = staging.path().join("payload-1");
    fs::write(&staged, b"jpegs").expect("stage");
    signal_tx
        .send(FrontDoorSignal::FileStaged {
            session,
            staged_path: staged,
            desired_name: "photo.jpg".into(),
        })
        .await
        .expect("signal");

    match next_event(&mut events).await {
        Event::FileFinalized { session: s, path } => {
            assert_eq!(s, session);
            assert_eq!(path, dest.path().join("photo.jpg").to_string_lossy());
            assert_eq!(fs::read(&path).expect("read"), b"jpegs");
        }
        other => panic!("expected FileFinalized, got {other:?}"),
    }

    // Sender finishes.
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
            outcome: SessionOutcome::Completed
        }
    );

    handle.send(Command::Shutdown).await.expect("send");
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::StopAdvertising)
    );
    timeout(STEP, task)
        .await
        .expect("engine exit")
        .expect("join");
}

#[tokio::test]
async fn decline_is_relayed_and_session_ends() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, mut control_rx, signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(test_config(dest.path(), staging.path()), channels);
    let mut events = handle.subscribe();

    let session = SessionId(7);
    signal_tx
        .send(FrontDoorSignal::Connected {
            session,
            token: "0000".into(),
        })
        .await
        .expect("signal");
    signal_tx
        .send(FrontDoorSignal::Introduction {
            session,
            sender_name: "Pixel".into(),
            files: vec![FileOffer {
                name: "a".into(),
                size: 1,
                mime_type: None,
            }],
            total_bytes: 0,
            text_preview: None,
        })
        .await
        .expect("signal");
    let _ = next_event(&mut events).await; // SessionConnected
    let _ = next_event(&mut events).await; // IntroductionReceived

    handle.send(Command::Decline(session)).await.expect("send");
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::Respond {
            session,
            accept: false
        })
    );

    signal_tx
        .send(FrontDoorSignal::Ended {
            session,
            reason: EndReason::DeclinedByUser,
        })
        .await
        .expect("signal");
    assert_eq!(
        next_event(&mut events).await,
        Event::SessionEnded {
            session,
            outcome: SessionOutcome::Rejected
        }
    );
}

#[tokio::test]
async fn second_sender_is_rejected_while_busy() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, mut control_rx, signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(test_config(dest.path(), staging.path()), channels);
    let mut events = handle.subscribe();

    let first = SessionId(1);
    let second = SessionId(2);
    signal_tx
        .send(FrontDoorSignal::Connected {
            session: first,
            token: "1111".into(),
        })
        .await
        .expect("signal");
    signal_tx
        .send(FrontDoorSignal::Connected {
            session: second,
            token: "2222".into(),
        })
        .await
        .expect("signal");

    assert_eq!(
        next_event(&mut events).await,
        Event::SessionConnected { session: first }
    );
    match next_event(&mut events).await {
        Event::ErrorOccurred {
            session,
            code: ErrorCode::Busy,
            ..
        } => assert_eq!(session, Some(second)),
        other => panic!("expected Busy error, got {other:?}"),
    }
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::Cancel { session: second })
    );
    drop(handle);
}

#[tokio::test]
async fn oversized_introduction_is_auto_rejected() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, mut control_rx, signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(test_config(dest.path(), staging.path()), channels);
    let mut events = handle.subscribe();

    let session = SessionId(3);
    signal_tx
        .send(FrontDoorSignal::Connected {
            session,
            token: "9999".into(),
        })
        .await
        .expect("signal");
    // 501 files exceeds the default limit of 500.
    let files: Vec<FileOffer> = (0..501)
        .map(|i| FileOffer {
            name: format!("f{i}"),
            size: 1,
            mime_type: None,
        })
        .collect();
    signal_tx
        .send(FrontDoorSignal::Introduction {
            session,
            sender_name: "Pixel".into(),
            files,
            total_bytes: 0,
            text_preview: None,
        })
        .await
        .expect("signal");

    let _ = next_event(&mut events).await; // SessionConnected
    match next_event(&mut events).await {
        Event::ErrorOccurred {
            code: ErrorCode::LimitsExceeded,
            ..
        } => {}
        other => panic!("expected LimitsExceeded, got {other:?}"),
    }
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::Respond {
            session,
            accept: false
        })
    );
    drop(handle);
}
