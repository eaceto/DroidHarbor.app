//! Everything the component can be told — by widgets, the tray, the platform
//! layer, and the domain's event stream.

use std::path::PathBuf;

use dh_domain::{Event, SessionId};

use crate::{history, platform, update};

#[derive(Debug)]
pub enum Msg {
    /// Does nothing but reach `update_view`. relm4 does not render after
    /// `init`, so without this the loaded history and the initial switch
    /// states would not appear until some other event happened to arrive.
    Refresh,
    Domain(Event),
    SetReceiving(bool),
    SetDiscovering(bool),
    Consent {
        session: SessionId,
        decision: platform::Decision,
    },
    Cancel(SessionId),
    HideToBackground,
    ChooseDestination,
    DestinationChosen(PathBuf),
    RenameDevice(String),
    SetAutoOff(u64),
    SetLaunchAtLogin(bool),
    SetPlaySounds(bool),
    /// Linux only: an AppImage is run, not installed.
    #[cfg(target_os = "linux")]
    InstallDesktopEntry,
    Revoke(String),
    SelectEndpoint(String),
    StageFiles,
    Staged(Vec<PathBuf>),
    StageText(String),
    ClearStaged,
    Send,
    RetrySend,
    DismissRetry,
    ShowOnboarding,
    FinishOnboarding,
    CheckForUpdates,
    UpdateAvailable(Option<update::Available>),
    Search(String),
    Filter(history::Category),
    RemoveEntry(uuid::Uuid),
    /// The Clear button: asks first, since unlike a removed row there is no
    /// putting the whole record back.
    ClearHistory,
    /// The dialog's answers.
    ClearHistoryShown,
    ClearHistoryAll,
    /// Drop entries whose received files no longer exist on disk.
    PruneHistory,
    Reveal(PathBuf),
    CopyText(String),
    DismissText,
    DismissNotice,
    /// Fired by the timer started when a notice appeared.
    ExpireNotice(u64),
    /// The tray registered (or did not).
    TrayReady(Option<platform::Tray>),
    /// Bring the window back from the tray or a second launch.
    Present,
    ShowAbout,
    /// Turn receiving on and let it lapse after `minutes`.
    ReceiveFor(u64),
    SendClipboardText,
    Quit,
}
