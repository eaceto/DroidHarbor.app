import Foundation

// Keeps the Mac awake for as long as bytes are moving.

/// Holds a power assertion while a transfer is in flight.
///
/// Without this the display sleeping takes the system with it and a large
/// transfer dies part-way, the most likely "it just stopped" report. The
/// assertion only prevents *idle* sleep: closing the lid still sleeps, as the
/// user plainly intended.
@MainActor
final class SleepBlocker {
    private var activity: NSObjectProtocol?

    var isActive: Bool { activity != nil }

    func begin(reason: String) {
        guard activity == nil else { return }
        activity = ProcessInfo.processInfo.beginActivity(
            options: [.userInitiated, .idleSystemSleepDisabled],
            reason: reason)
    }

    func end() {
        guard let activity else { return }
        ProcessInfo.processInfo.endActivity(activity)
        self.activity = nil
    }
}
