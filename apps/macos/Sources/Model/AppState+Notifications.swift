import AppKit
import Foundation
import UserNotifications

// Everything the app says through Notification Center: the consent request
// with its Accept/Decline actions, and the banners announcing that something
// arrived or was sent, each carrying the actions that suit it.

@MainActor
extension AppState {
    /// Ask for consent through a notification with Accept/Decline actions, so
    /// no modal window steals focus. The window's transfer card carries the
    /// same choice for anyone who dismissed the banner.
    func requestConsent(
        session: UInt64, sender: String, count: Int, totalBytes: UInt64, token: String
    ) {
        let content = UNMutableNotificationContent()
        content.title = count == 1
            ? String(localized: "\u{201C}\(sender)\u{201D} wants to send 1 file")
            : String(localized: "\u{201C}\(sender)\u{201D} wants to send \(count) files")
        content.body = String(
            localized: "\(Format.bytes(totalBytes)) · Code \(token) (must match the phone)")
        content.categoryIdentifier = Notifications.consentCategory
        content.userInfo = [Notifications.sessionKey: String(session)]
        if playSounds {
            content.sound = .default
        }
        post(content, identifier: "consent-\(session)")
    }

    /// Announce a finished transfer. `path` gets the banner its Open and
    /// Show in Finder actions; without one, clicking it opens the app.
    func notify(title: String, body: String, attaching path: String?) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        if playSounds {
            content.sound = .default
        }

        if let path {
            content.categoryIdentifier = Notifications.receivedFileCategory
            content.userInfo = [Notifications.pathKey: path]
            // Preview received images right in the banner.
            //
            // UNNotificationAttachment takes ownership of the file it is
            // handed and MOVES it into the notification store, so it must
            // never be given the file the user just received. Attach a
            // throwaway copy.
            if ["jpg", "jpeg", "png", "gif", "heic", "webp"]
                .contains(URL(fileURLWithPath: path).pathExtension.lowercased()),
                let preview = temporaryCopy(of: path),
                let attachment = try? UNNotificationAttachment(
                    identifier: UUID().uuidString, url: preview, options: nil)
            {
                content.attachments = [attachment]
            }
        } else {
            content.categoryIdentifier = Notifications.transferDoneCategory
        }

        post(content, identifier: UUID().uuidString)
    }

    /// Deliver it, and say so when the system refuses: a silent failure
    /// here looks exactly like "the app never noticed the transfer".
    private func post(_ content: UNNotificationContent, identifier: String) {
        let request = UNNotificationRequest(
            identifier: identifier, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request) { error in
            guard let error else { return }
            NSLog("DroidHarbor: notification refused: \(error.localizedDescription)")
        }
    }

    /// Notifications that are switched off are the usual reason nothing
    /// appears, so the state is shown in Settings rather than left a mystery.
    func refreshNotificationAuthorization() {
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            let status = settings.authorizationStatus
            let granted = status == .authorized || status == .provisional
            // Permission on its own is not enough for this app. With the
            // alert style set to None the system still reports .authorized,
            // yet no banner is ever drawn, which means the Accept and Decline
            // buttons on a consent notification cannot be reached. Reporting
            // that as working sends the user looking in the wrong place.
            let allowed = granted && settings.alertSetting != .disabled
            let decided = status != .notDetermined
            Task { @MainActor [weak self] in
                self?.notificationsAllowed = allowed
                self?.notificationsDecided = decided
            }
        }
    }

    /// Ask the system for permission, which is what puts the prompt on
    /// screen. Driven from the onboarding's last page rather than from
    /// launch: the prompt is a one-shot, and spending it on someone who has
    /// not yet been told what notifications are for here is how an install
    /// ends up permanently silent.
    func requestNotificationAuthorization() {
        UNUserNotificationCenter.current()
            .requestAuthorization(options: [.alert, .sound]) { _, error in
                if let error {
                    NSLog("DroidHarbor: notifications not authorised: \(error.localizedDescription)")
                }
                Task { @MainActor [weak self] in
                    self?.refreshNotificationAuthorization()
                }
            }
    }

    func openNotificationSettings() {
        let url = URL(
            string: "x-apple.systempreferences:com.apple.preference.notifications")
        guard let url else { return }
        NSWorkspace.shared.open(url)
    }

    /// Copy a file into a unique temporary directory. The copy is what gets
    /// handed to APIs that consume the file they are given.
    func temporaryCopy(of path: String) -> URL? {
        let source = URL(fileURLWithPath: path)
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("previews/\(UUID().uuidString)", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let copy = dir.appendingPathComponent(source.lastPathComponent)
            try FileManager.default.copyItem(at: source, to: copy)
            return copy
        } catch {
            return nil
        }
    }
}

enum Notifications {
    static let consentCategory = "TRANSFER_CONSENT"
    static let receivedFileCategory = "TRANSFER_RECEIVED_FILE"
    static let transferDoneCategory = "TRANSFER_DONE"

    static let acceptAction = "ACCEPT_TRANSFER"
    static let declineAction = "DECLINE_TRANSFER"
    static let revealAction = "REVEAL_ITEM"
    static let openAction = "OPEN_ITEM"

    static let sessionKey = "session"
    static let pathKey = "path"
}
