//! Domain guards: disk space and the idle auto-off timer.

use std::time::Duration;

use dh_core::limits::Limits;
use dh_domain::{
    spawn, Command, DomainConfig, ErrorCode, Event, FrontDoorChannels, FrontDoorControl,
    FrontDoorSignal, SessionId, Settings,
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

fn config(dest: &std::path::Path, staging: &std::path::Path, limits: Limits) -> DomainConfig {
    DomainConfig {
        settings: Settings::new("Test Mac", dest.to_path_buf(), staging.to_path_buf()),
        limits,
    }
}

#[tokio::test]
async fn introduction_larger_than_the_disk_is_rejected() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");

    let (channels, mut control_rx, signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(
        config(dest.path(), staging.path(), Limits::default()),
        channels,
    );
    let mut events = handle.subscribe();

    let session = SessionId(1);
    signal_tx
        .send(FrontDoorSignal::Connected {
            session,
            token: "1234".into(),
        })
        .await
        .expect("signal");
    let _ = next_event(&mut events).await; // SessionConnected

    // No real filesystem can hold this, so the check must fire before the
    // user is ever asked.
    signal_tx
        .send(FrontDoorSignal::Introduction {
            session,
            sender_name: "Pixel".into(),
            files: vec![dh_domain::FileOffer {
                name: "huge.bin".into(),
                size: 0,
                mime_type: None,
            }],
            total_bytes: u64::MAX / 2,
            text_preview: None,
        })
        .await
        .expect("signal");

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

#[tokio::test(start_paused = true)]
async fn receiving_turns_itself_off_when_idle() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");
    let limits = Limits {
        auto_off_minutes: 10,
        ..Limits::default()
    };

    let (channels, mut control_rx, _signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(config(dest.path(), staging.path(), limits), channels);
    let mut events = handle.subscribe();

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

    // Paused clock: await without a competing timeout, so auto-advance
    // jumps straight to the idle deadline instead of the timeout's.
    assert_eq!(
        events.recv().await.expect("event stream closed"),
        Event::AdvertisingChanged(false)
    );
    assert_eq!(
        control_rx.recv().await,
        Some(FrontDoorControl::StopAdvertising)
    );
    drop(handle);
}

#[tokio::test(start_paused = true)]
async fn auto_off_can_be_disabled() {
    let dest = tempfile::tempdir().expect("dest");
    let staging = tempfile::tempdir().expect("staging");
    let limits = Limits {
        auto_off_minutes: 0,
        ..Limits::default()
    };

    let (channels, mut control_rx, _signal_tx) = FrontDoorChannels::pair(16);
    let (handle, _task) = spawn(config(dest.path(), staging.path(), limits), channels);
    let mut events = handle.subscribe();

    handle
        .send(Command::SetReceiving(true))
        .await
        .expect("send");
    let _ = control_rx.recv().await;
    assert_eq!(
        next_event(&mut events).await,
        Event::AdvertisingChanged(true)
    );

    // Nothing further should arrive, however long the clock runs.
    assert!(
        timeout(Duration::from_secs(3600), events.recv())
            .await
            .is_err(),
        "receiving must stay on when the timer is disabled"
    );
    drop(handle);
}
