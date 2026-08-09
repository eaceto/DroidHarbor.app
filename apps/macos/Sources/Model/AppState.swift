import AppKit
import Foundation
import UserNotifications

// The app's single source of truth: owns the DHService, turns domain
// events into published state, and exposes the intents the views call.
// Notifications live in AppState+Notifications, value types in Transfer.

@MainActor
final class AppState: ObservableObject {
    // Receiving
    @Published var receiving = false
    @Published var transfer: ActiveTransfer?
    /// Set while receiving is on a timer ("Receive for 10 minutes").
    @Published private(set) var receivingUntil: Date?

    // Sending
    @Published var endpoints: [Endpoint] = []
    /// What is staged to send: files or text, chosen but not yet sent.
    @Published var pendingSend: SendPayload?
    @Published var outbound: OutboundTransfer?

    // Shared
    @Published var history: [HistoryEntry] = []
    @Published var lastError: String?
    /// News rather than failure — "you are up to date" and the like. Kept
    /// apart from `lastError` because the views draw the two differently,
    /// and an informational line dressed as a warning reads as a fault.
    @Published var lastNotice: String?
    /// The most recent text or link, which lives on the clipboard rather
    /// than on disk.
    @Published var receivedText: ReceivedText?
    /// A published release newer than this build, if the check found one.
    @Published var availableUpdate: AvailableUpdate?
    @Published private(set) var destination: URL
    @Published private(set) var deviceName: String
    @Published private(set) var iconState: IconState = .off
    @Published var launchAtLogin: Bool = LaunchAtLogin.isEnabled
    /// False when the system will not show notifications, which is the
    /// usual reason a completed transfer seems to announce nothing.
    ///
    /// Starts false and is corrected by the first `refreshNotificationAuthorization`.
    /// The optimistic default claimed "Allowed" in the introduction's setup
    /// page for as long as that asynchronous answer took, which is exactly
    /// the moment the value is being acted on.
    @Published var notificationsAllowed = false
    /// True once the system has an answer on record. Asking is only
    /// meaningful before that: macOS shows the prompt once per install and
    /// answers every later request from the stored decision without showing
    /// anything, so a decided-and-refused state has to point at System
    /// Settings instead of offering a button that would do nothing.
    @Published var notificationsDecided = false
    @Published var playSounds: Bool
    /// Minutes of idle advertising before receiving switches itself off;
    /// 0 keeps it on until the user says otherwise.
    @Published var autoOffMinutes: Int
    /// False until the introduction has been shown once.
    @Published var hasOnboarded: Bool
    /// Sender names whose transfers are accepted without asking.
    @Published private(set) var trustedDevices: [String]
    /// Window tab to restore on open.
    @Published var selectedSection: String

    /// Set by the app delegate so events can bring the window forward.
    var onOpenWindow: (() -> Void)?

    /// Paths finalized during the current inbound session.
    private var sessionSaved: [String] = []
    /// Details of the tapped device while the send command is in flight.
    private var sendTargetName: String?
    private var sendPayload: SendPayload?
    private var autoOffTask: Task<Void, Never>?
    /// Set when the current session delivered a link or text, so its
    /// completion is not also announced as "0 files received".
    private var sessionDeliveredText = false
    private let sleepBlocker = SleepBlocker()
    /// The last outbound attempt, so a failure can be retried.
    @Published private(set) var lastSend: (endpoint: Endpoint, payload: SendPayload)?

    private var service: DHService?
    /// Storage is injected so tests can run against a scratch suite instead
    /// of the user's real preferences.
    private let defaults: UserDefaults

    private static let destinationKey = "destination"
    private static let deviceNameKey = "deviceName"
    private static let receivingKey = "receivingEnabled"
    private static let soundsKey = "playSounds"
    private static let autoOffKey = "autoOffMinutes"
    private static let onboardedKey = "hasOnboarded"
    private static let trustedKey = "trustedDevices"
    private static let sectionKey = "selectedSection"
    private static let knownDevicesKey = "knownDevices"

    /// - Parameter startService: false builds the state without starting the
    ///   transfer service, so event handling can be exercised without
    ///   sockets, mDNS or a live phone.
    init(defaults: UserDefaults = .standard, startService: Bool = true) {
        self.defaults = defaults
        if let path = defaults.string(forKey: Self.destinationKey) {
            destination = URL(fileURLWithPath: path)
        } else {
            destination = FileManager.default
                .urls(for: .downloadsDirectory, in: .userDomainMask)[0]
                .appendingPathComponent(
                    AppInfo.isDevelopmentBuild ? "droidharbor-dev" : "droidharbor",
                    isDirectory: true)
        }
        // A development build says so in the phone's share sheet: both can
        // be running, and picking the wrong one wastes a test.
        deviceName = defaults.string(forKey: Self.deviceNameKey)
            ?? ((Host.current().localizedName ?? "My Mac")
                + (AppInfo.isDevelopmentBuild ? " Dev" : ""))
        playSounds = defaults.object(forKey: Self.soundsKey) as? Bool ?? true
        // Default off: an always-ready receiver is the point of the app, and
        // every transfer still needs explicit acceptance.
        autoOffMinutes = defaults.object(forKey: Self.autoOffKey) as? Int ?? 0
        hasOnboarded = defaults.bool(forKey: Self.onboardedKey)
        trustedDevices = defaults.stringArray(forKey: Self.trustedKey) ?? []
        selectedSection = defaults.string(forKey: Self.sectionKey) ?? "transfers"
        // Devices seen before are listed straight away (dimmed) so the send
        // list is never empty while discovery warms up.
        endpoints = (defaults.array(forKey: Self.knownDevicesKey) as? [[String: String]] ?? [])
            .compactMap { entry in
                guard let name = entry["name"] else { return nil }
                return Endpoint(
                    id: entry["id"] ?? name, name: name,
                    kind: entry["kind"] ?? "phone", present: false)
            }
        history = HistoryStore.load(from: defaults)
        if startService {
            self.startService()
        }
    }

    // MARK: - Service lifecycle

    private var stagingDir: URL {
        // Same volume as the destination keeps finalization a pure rename.
        destination.appendingPathComponent(".dh-staging", isDirectory: true)
    }

    private func startService() {
        do {
            try FileManager.default.createDirectory(
                at: destination, withIntermediateDirectories: true)
            // Debug builds write ~/Library/Logs/DroidHarbor/droidharbor.log,
            // which names files and folders; shipped builds log nothing.
            var logging = false
            #if DEBUG
            logging = true
            #endif
            let service = try DHService.start(
                destination: destination.path,
                stagingDir: stagingDir.path,
                deviceName: deviceName,
                port: nil,
                enableLogging: logging)
            service.addListener(listener: EventForwarder(state: self))
            self.service = service
            try service.setAutoOffMinutes(minutes: UInt64(max(0, autoOffMinutes)))
            // Restore the last receiving state (also after settings-driven
            // service restarts).
            if defaults.bool(forKey: Self.receivingKey) {
                try service.setReceiving(on: true)
            }
        } catch {
            lastError = String(localized: "Could not start the receiver: \(error.localizedDescription)")
        }
    }

    /// Device name and staging location are fixed per service instance;
    /// changing either restarts the (idle) service.
    private func restartService() {
        try? service?.shutdown()
        service = nil
        receiving = false
        transfer = nil
        startService()
    }

    // MARK: - Receiving

    func setReceiving(_ on: Bool, persist: Bool = true) {
        if persist {
            defaults.set(on, forKey: Self.receivingKey)
        }
        if !on || persist {
            autoOffTask?.cancel()
            autoOffTask = nil
            receivingUntil = nil
        }
        do {
            try service?.setReceiving(on: on)
        } catch {
            lastError = String(localized: "Receiver is not running.")
        }
        refreshIcon()
    }

    /// Turn receiving on for a fixed window, then off again. Deliberately not
    /// persisted: a temporary session should not survive a restart.
    func receiveTemporarily(minutes: Int) {
        autoOffTask?.cancel()
        setReceiving(true, persist: false)
        let deadline = Date().addingTimeInterval(TimeInterval(minutes * 60))
        receivingUntil = deadline
        autoOffTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(minutes * 60))
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard let self else { return }
                self.receivingUntil = nil
                self.setReceiving(false, persist: false)
            }
        }
    }

    func accept(_ session: UInt64) {
        try? service?.accept(session: session)
        transfer?.receiving = true
        refreshIcon()
    }

    func decline(_ session: UInt64) {
        try? service?.decline(session: session)
    }

    func cancel(_ session: UInt64) {
        try? service?.cancel(session: session)
    }

    // MARK: - Settings

    func chooseDestination() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.prompt = String(localized: "Save Here")
        NSApp.activate(ignoringOtherApps: true)
        guard panel.runModal() == .OK, let url = panel.url else { return }
        destination = url
        defaults.set(url.path, forKey: Self.destinationKey)
        restartService()
    }

    func rename(to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, trimmed != deviceName else { return }
        deviceName = trimmed
        defaults.set(trimmed, forKey: Self.deviceNameKey)
        restartService()
    }

    func setLaunchAtLogin(_ enabled: Bool) {
        if let message = LaunchAtLogin.set(enabled) {
            lastError = message
        }
        launchAtLogin = LaunchAtLogin.isEnabled
    }

    /// Re-read the login item from the system. It can be switched off in
    /// System Settings → General → Login Items without the app hearing about
    /// it, which leaves the toggle showing the opposite of the truth.
    func refreshLaunchAtLogin() {
        launchAtLogin = LaunchAtLogin.isEnabled
    }

    func setPlaySounds(_ enabled: Bool) {
        playSounds = enabled
        defaults.set(enabled, forKey: Self.soundsKey)
    }

    func setAutoOffMinutes(_ minutes: Int) {
        autoOffMinutes = max(0, minutes)
        defaults.set(autoOffMinutes, forKey: Self.autoOffKey)
        try? service?.setAutoOffMinutes(minutes: UInt64(autoOffMinutes))
    }

    func selectSection(_ name: String) {
        selectedSection = name
        defaults.set(name, forKey: Self.sectionKey)
    }

    /// Trust is keyed by the name the phone announces, which it chooses
    /// itself, convenient but not proof of identity. Only ever set from an
    /// accept the user made deliberately.
    func trust(_ name: String) {
        guard !name.isEmpty, !trustedDevices.contains(name) else { return }
        trustedDevices.append(name)
        defaults.set(trustedDevices, forKey: Self.trustedKey)
    }

    func revokeTrust(_ name: String) {
        trustedDevices.removeAll { $0 == name }
        defaults.set(trustedDevices, forKey: Self.trustedKey)
    }

    func isTrusted(_ name: String) -> Bool {
        trustedDevices.contains(name)
    }

    private func rememberDevices() {
        let known = endpoints.map { ["id": $0.id, "name": $0.name, "kind": $0.kind] }
        defaults.set(known, forKey: Self.knownDevicesKey)
    }

    func forgetDevices() {
        endpoints.removeAll { !$0.present }
        rememberDevices()
    }

    func completeOnboarding() {
        hasOnboarded = true
        defaults.set(true, forKey: Self.onboardedKey)
    }

    func showOnboardingAgain() {
        hasOnboarded = false
        defaults.set(false, forKey: Self.onboardedKey)
        onOpenWindow?()
    }

    // MARK: - Files

    func reveal(path: String) {
        guard exists(path) else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    func open(path: String) {
        guard exists(path) else { return }
        NSWorkspace.shared.open(URL(fileURLWithPath: path))
    }

    /// History outlives the files it points at; say so instead of letting the
    /// click do nothing.
    private func exists(_ path: String) -> Bool {
        if FileManager.default.fileExists(atPath: path) {
            return true
        }
        lastError = String(localized: "\(URL(fileURLWithPath: path).lastPathComponent) is no longer at that location. It was moved or deleted.")
        return false
    }

    func copyPath(_ path: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(path, forType: .string)
    }

    func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    /// Open a received link in the default browser.
    func openLink(_ link: String) {
        guard let url = Self.webURL(from: link) else {
            lastError = String(localized: "Only web links can be opened.")
            return
        }
        NSWorkspace.shared.open(url)
    }

    /// Dial a number: the tel: URL is built here, from digits we extracted.
    func call(_ number: String) {
        open(scheme: "tel", value: PayloadClassifier.value(from: number, strippingScheme: "tel"))
    }

    /// Start an email to an address we extracted.
    func compose(to address: String) {
        open(
            scheme: "mailto",
            value: PayloadClassifier.value(from: address, strippingScheme: "mailto"))
    }

    /// Show a place in Maps. A maps link opens as-is when it is a web URL;
    /// anything else is treated as a search query.
    func showOnMap(_ place: String) {
        if let web = Self.webURL(from: place) {
            NSWorkspace.shared.open(web)
            return
        }
        let query = PayloadClassifier.value(from: place, strippingScheme: "geo")
        guard let encoded = query.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed),
              let url = URL(string: "https://maps.apple.com/?q=\(encoded)")
        else {
            lastError = String(localized: "That place could not be opened.")
            return
        }
        NSWorkspace.shared.open(url)
    }

    /// Open content the system understands as a file (a contact card or an
    /// event) by writing it somewhere temporary first.
    func openAsFile(_ content: String, extension ext: String) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("payloads/\(UUID().uuidString)", isDirectory: true)
        let file = dir.appendingPathComponent("shared.\(ext)")
        do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            try Data(content.utf8).write(to: file)
            NSWorkspace.shared.open(file)
        } catch {
            lastError = String(localized: "That item could not be opened.")
        }
    }

    /// Build a URL from a scheme this app chose and a value it extracted,
    /// so a sender can never pick the scheme.
    private func open(scheme: String, value: String) {
        let allowed = CharacterSet.urlPathAllowed.union(.urlQueryAllowed)
        guard !value.isEmpty,
              let encoded = value.addingPercentEncoding(withAllowedCharacters: allowed),
              let url = URL(string: "\(scheme):\(encoded)")
        else {
            lastError = String(localized: "That item could not be opened.")
            return
        }
        NSWorkspace.shared.open(url)
    }

    /// A link that is safe to hand to the system, or nil.
    ///
    /// The content comes from another device, so only http and https are
    /// accepted. Opening an arbitrary scheme would let a sender launch
    /// something else on this Mac with a single click: `file://` reveals or
    /// runs local items, and any installed app can claim a custom scheme.
    static func webURL(from link: String) -> URL? {
        guard let url = URL(string: link),
              let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              url.host?.isEmpty == false
        else {
            return nil
        }
        return url
    }

    func clearHistory() {
        history = []
        HistoryStore.save(history, to: defaults)
    }

    /// Forget one entry. This is a record of a transfer, not the transfer
    /// itself: a received file stays exactly where it was saved, and only
    /// the row goes away.
    func removeFromHistory(_ entry: HistoryEntry) {
        history.removeAll { $0.id == entry.id }
        HistoryStore.save(history, to: defaults)
    }

    // MARK: - Sending

    /// Pick files, then discover nearby devices to send them to.
    func chooseFilesToSend() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.prompt = String(localized: "Send")
        NSApp.activate(ignoringOtherApps: true)
        guard panel.runModal() == .OK, !panel.urls.isEmpty else { return }
        beginSend(files: panel.urls)
    }

    /// Stage files for sending and start looking for devices. Entry point for
    /// the picker, the window's drop zone, drops on the menu-bar icon and the
    /// share extension.
    ///
    /// A drag can carry web addresses as well as files (dragging a link out
    /// of a browser produces one), so those are sent as links rather than
    /// refused for not being files.
    func beginSend(files: [URL]) {
        let regular = files.filter { url in
            (try? url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true
        }
        guard regular.isEmpty else {
            stage(.files(regular))
            return
        }
        if let link = files.first(where: { $0.scheme == "http" || $0.scheme == "https" }) {
            beginSend(text: link.absoluteString)
            return
        }
        lastError = String(localized: "Only files can be sent. Folders are not supported yet.")
    }

    /// Stage text for sending. Entry point for the text sheet, dropped or
    /// pasted text, and the share extension.
    func beginSend(text: String) {
        let payload = OutboundText(content: text)
        guard !payload.content.isEmpty else {
            lastError = String(localized: "There is no text to send.")
            return
        }
        stage(.text(payload))
    }

    /// Send whatever text is on the clipboard, if any.
    func sendClipboardText() {
        guard let text = NSPasteboard.general.string(forType: .string),
              !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            lastError = String(localized: "The clipboard has no text to send.")
            return
        }
        beginSend(text: text)
    }

    private func stage(_ payload: SendPayload) {
        pendingSend = payload
        endpoints = []
        try? service?.setDiscovering(on: true)
        onOpenWindow?()
    }

    func cancelSendSelection() {
        pendingSend = nil
        endpoints = []
        try? service?.setDiscovering(on: false)
    }

    func send(to endpoint: Endpoint) {
        guard let pendingSend else { return }
        send(pendingSend, to: endpoint)
    }

    /// Retry the last send that did not complete.
    func retryLastSend() {
        guard let last = lastSend else { return }
        lastError = nil
        send(last.payload, to: last.endpoint)
    }

    func dismissRetry() {
        lastSend = nil
    }

    private func send(_ payload: SendPayload, to endpoint: Endpoint) {
        guard !payload.isEmpty else { return }
        sendTargetName = endpoint.name
        sendPayload = payload
        lastSend = (endpoint, payload)
        do {
            switch payload {
            case .files(let urls):
                try service?.sendFiles(endpoint: endpoint.id, files: urls.map(\.path))
            case .text(let text):
                try service?.sendText(
                    endpoint: endpoint.id,
                    kind: text.wireKind,
                    description: text.title,
                    content: text.content)
            }
            pendingSend = nil
        } catch {
            lastError = String(localized: "Could not start the send.")
        }
    }

    func dismissError() {
        lastError = nil
    }

    func dismissNotice() {
        lastNotice = nil
    }

    func dismissReceivedText() {
        receivedText = nil
    }

    /// Look for a newer release. Runs once at launch and on request; a
    /// user-initiated check reports "you are up to date" rather than saying
    /// nothing at all.
    func checkForUpdates(userInitiated: Bool = false) {
        let current = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
        guard let manifest = URL(string: Links.updateManifest) else { return }
        Task { [weak self] in
            let found = await UpdateChecker.check(current: current, manifest: manifest)
            await MainActor.run {
                guard let self else { return }
                self.availableUpdate = found
                if userInitiated, found == nil {
                    self.lastNotice = String(localized: "DroidHarbor is up to date.")
                }
            }
        }
    }

    func quit() {
        try? service?.shutdown()
        NSApp.terminate(nil)
    }

    // MARK: - Domain events

    func handle(_ event: DHEvent) {
        switch event {
        case .advertisingChanged(let on):
            receiving = on

        case .sessionConnected:
            break

        case .introductionReceived(
            let session, let senderName, let files, let totalBytes, let token, let textPreview):
            transfer = ActiveTransfer(
                session: session,
                senderName: senderName,
                files: files.enumerated().map { index, file in
                    OfferedFile(id: index, name: file.name, size: file.size)
                },
                totalBytes: totalBytes,
                token: token,
                textPreview: textPreview)
            sessionSaved = []
            lastError = nil
            if isTrusted(senderName) {
                accept(session)
                notify(
                    title: String(localized: "Receiving from \u{201C}\(senderName)\u{201D}"),
                    body: String(localized: "Accepted automatically. This device is trusted."),
                    attaching: nil)
            } else {
                requestConsent(
                    session: session, sender: senderName,
                    count: files.count, totalBytes: totalBytes, token: token)
                onOpenWindow?()
            }

        case .progress(let session, let bytesReceived, _, let currentFile, let files):
            if var active = transfer, active.session == session {
                active.bytesReceived = bytesReceived
                active.receiving = true
                if !currentFile.isEmpty {
                    active.currentFile = currentFile
                }
                for update in files {
                    guard let index = active.files.firstIndex(where: { $0.name == update.name })
                    else { continue }
                    active.files[index].bytesTransferred = update.bytesTransferred
                    active.files[index].completed = update.completed
                }
                active.rate.sample(bytes: bytesReceived, total: active.totalBytes)
                transfer = active
            } else if var sending = outbound, sending.session == session {
                sending.bytesSent = bytesReceived
                sending.awaitingConsent = false
                sending.rate.sample(bytes: bytesReceived, total: sending.totalBytes)
                outbound = sending
            }

        case .fileFinalized(_, let path):
            Quarantine.mark(path: path)
            sessionSaved.append(path)
            let name = URL(fileURLWithPath: path).lastPathComponent
            transfer?.completedNames.insert(name)
            if let index = transfer?.files.firstIndex(where: { $0.name == name }) {
                transfer?.files[index].completed = true
            }

        case .sessionEnded(let session, let outcome):
            if let sent = outbound, sent.session == session {
                finishOutbound(sent, outcome: outcome)
            } else {
                finishInbound(outcome: outcome)
            }

        case .textReceived(_, let kind, let description, let content):
            // Nothing lands on disk, so the clipboard is where it is useful.
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(content, forType: .string)
            receivedText = ReceivedText(kind: kind, description: description, content: content)
            sessionDeliveredText = true
            record(
                direction: .received,
                peer: transfer?.senderName ?? String(localized: "Android device"),
                kind: PayloadClassifier.kind(forText: content, declared: kind),
                content: content)
            notify(
                title: kind == "link"
                    ? String(localized: "Link copied")
                    : String(localized: "Text copied"),
                body: content.count > 120 ? String(content.prefix(120)) + "…" : content,
                attaching: nil)

        case .errorOccurred(_, _, let message):
            lastError = message

        case .discoveringChanged:
            break

        case .endpointUpdated(let endpoint, let name, let kind, let present):
            // Match on name too: the mDNS id changes between advertisements,
            // so a remembered device would otherwise appear twice.
            let index = endpoints.firstIndex { $0.id == endpoint || $0.name == name }
            if present {
                if let index {
                    endpoints[index] = Endpoint(id: endpoint, name: name, kind: kind, present: true)
                } else {
                    endpoints.append(Endpoint(id: endpoint, name: name, kind: kind))
                }
                rememberDevices()
            } else if let index {
                // Keep it listed, dimmed, rather than making the list jump.
                endpoints[index].present = false
            }

        case .sendAwaitingConsent(let session, let totalBytes, let token):
            outbound = OutboundTransfer(
                session: session,
                targetName: sendTargetName ?? String(localized: "Android device"),
                payload: sendPayload ?? .files([]),
                totalBytes: totalBytes,
                token: token)
            sendTargetName = nil
            sendPayload = nil
        }
        refreshIcon()
    }

    private func finishInbound(outcome: SessionOutcome) {
        let peer = transfer?.senderName ?? String(localized: "Android device")
        let saved = sessionSaved
        transfer = nil
        sessionSaved = []

        let deliveredText = sessionDeliveredText
        sessionDeliveredText = false

        switch outcome {
        case .completed:
            // A link or text was already recorded and announced when it
            // arrived; saying "0 files received" on top of that is noise.
            if saved.isEmpty && deliveredText { return }
            if !saved.isEmpty {
                // One entry per kind, so a contact and a photo arriving
                // together stay distinguishable in the list.
                let grouped = Dictionary(grouping: saved) { PayloadClassifier.kind(forFile: $0) }
                for (kind, paths) in grouped.sorted(by: { $0.key.rawValue < $1.key.rawValue }) {
                    record(direction: .received, peer: peer, kind: kind, paths: paths)
                }
            }
            notify(
                title: String(localized: "Files received"),
                body: saved.count == 1
                    ? String(localized: "Saved \(URL(fileURLWithPath: saved[0]).lastPathComponent)")
                    : String(localized: "Saved \(saved.count) files from \u{201C}\(peer)\u{201D}"),
                attaching: saved.first)
        case .rejected, .cancelled:
            break
        case .timedOut:
            lastError = String(localized: "\u{201C}\(peer)\u{201D} timed out waiting for an answer.")
        case .failed:
            lastError = String(localized: "The transfer from \u{201C}\(peer)\u{201D} did not complete.")
        }
    }

    private func finishOutbound(_ sent: OutboundTransfer, outcome: SessionOutcome) {
        outbound = nil
        try? service?.setDiscovering(on: false)

        switch outcome {
        case .completed:
            lastSend = nil
            if let text = sent.text {
                record(
                    direction: .sent, peer: sent.targetName, kind: text.kind,
                    content: text.content)
                notify(
                    title: text.kind == .link
                        ? String(localized: "Link sent")
                        : String(localized: "Text sent"),
                    body: String(localized: "Sent to \u{201C}\(sent.targetName)\u{201D}"),
                    attaching: nil)
            } else {
                record(
                    direction: .sent, peer: sent.targetName,
                    paths: sent.payload.files.map(\.path))
                notify(
                    title: String(localized: "Files sent"),
                    body: sent.itemNames.count == 1
                        ? String(localized: "Sent 1 file to \u{201C}\(sent.targetName)\u{201D}")
                        : String(localized: "Sent \(sent.itemNames.count) files to \u{201C}\(sent.targetName)\u{201D}"),
                    attaching: nil)
            }
        case .rejected:
            lastError = String(localized: "\u{201C}\(sent.targetName)\u{201D} declined the transfer.")
        case .cancelled:
            break
        case .timedOut, .failed:
            lastError = String(localized: "The send to \u{201C}\(sent.targetName)\u{201D} did not complete.")
        }
    }

    private func record(
        direction: HistoryEntry.Direction,
        peer: String,
        kind: HistoryEntry.Kind = .files,
        paths: [String] = [],
        content: String? = nil
    ) {
        history.insert(
            HistoryEntry(
                direction: direction, peer: peer, paths: paths,
                content: content, kind: kind),
            at: 0)
        HistoryStore.save(history, to: defaults)
    }

    private func refreshIcon() {
        let busy = outbound != nil || transfer?.receiving == true
        iconState = busy ? .busy : (receiving ? .on : .off)

        // A transfer in progress is reason enough to keep the Mac awake.
        if busy {
            sleepBlocker.begin(reason: "Transferring files")
        } else {
            sleepBlocker.end()
        }
    }

}

/// Bridges Rust-thread callbacks onto the main actor.
private final class EventForwarder: EventListener {
    // Weak reference is only read to hop onto the main actor; safe despite
    // Sendable's mutability rule (weak requires `var`).
    private nonisolated(unsafe) weak var state: AppState?

    init(state: AppState) {
        self.state = state
    }

    func onEvent(event: DHEvent) {
        Task { @MainActor [weak state] in
            state?.handle(event)
        }
    }
}
