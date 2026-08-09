import Combine
import SwiftUI
import UserNotifications

@main
struct DroidHarborApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        // A SwiftUI scene owns the main window, so SwiftUI knows the window's
        // real size. Hosting it in a hand-made NSWindow meant the split view
        // was laid out against an ideal size instead, nearly twice the
        // window's height, which pushed the sidebar off the top.
        Window("DroidHarbor", id: WindowID.main) {
            MainWindowRoot(
                onWindow: { [delegate] window in delegate.registerMainWindow(window) },
                onOpenAction: { [delegate] open in delegate.registerOpenAction(open) })
                .environmentObject(delegate.state)
        }
        .defaultSize(width: 900, height: 580)
        // Only the content's minimum should constrain the window. Under
        // .automatic the content's ideal size gets a say too, which is how a
        // tall-reporting first-run view could open the window past the screen
        // and take the size in defaultSize with it.
        .windowResizability(.contentMinSize)
        .commands {
            AppCommands(state: delegate.state) { delegate.showAbout() }
        }

        // No Settings scene. Declaring one is what puts Settings… in the app
        // menu, and this app keeps its settings in a section of the main
        // window, so the scene was an empty placeholder that the menu item
        // opened as an empty window. AppCommands puts the menu item back and
        // points it at the section.
    }
}

/// Owns every AppKit surface: the status item (with its popover and drop
/// target), the main window and notification handling.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Unit tests are hosted by this app. It must then behave as a quiet
    /// shell: no receiver on the network, no windows, and no activation.
    /// Activating or switching activation policy mid-run breaks XCTest's
    /// connection to its host.
    static let isRunningTests = NSClassFromString("XCTestCase") != nil

    let state = AppState(startService: !AppDelegate.isRunningTests)

    private var statusItem: NSStatusItem?
    private let popover = NSPopover()
    private var iconSubscription: AnyCancellable?
    private var pulseTimer: Timer?
    private var pulsePhase = false
    private let dropHint = DropHintPanel()
    private var aboutWindow: NSWindow?
    /// The main scene's window, reported by the scene itself.
    private weak var mainWindow: NSWindow?
    /// SwiftUI's own "open this scene" action, handed over by the scene when
    /// it appears. It must be kept apart from `AppState.onOpenWindow`: that
    /// one belongs to this delegate, and pointing both at each other made
    /// `showWindow()` call itself until the stack ran out.
    private var openMainWindow: (() -> Void)?
    /// Set while waiting for SwiftUI to create that window, so it can be
    /// focused the moment it exists rather than on a guessed delay.
    private var raiseWhenReady = false
    private var badgeSubscription: AnyCancellable?
    /// Transient popovers close on the mouse-down of the very click that is
    /// about to toggle them; ignore toggles right after a close so clicking
    /// the icon while the menu is open closes it instead of reopening.
    private var lastClose = Date.distantPast
    /// True when another copy is already running and this one is standing
    /// down: it forwards its work instead of doing it.
    private var isDuplicate = false
    private var lastHandledURL: URL?
    static let forwardedURL = AppInfo.forwardedURLNotification

    func applicationDidFinishLaunching(_ notification: Notification) {
        if Self.isRunningTests { return }

        // Registered before the single-instance check: a copy that is about
        // to stand down still has to catch the URL it was launched with.
        NSAppleEventManager.shared().setEventHandler(
            self,
            andSelector: #selector(handleGetURLEvent(_:withReply:)),
            forEventClass: AEEventClass(kInternetEventClass),
            andEventID: AEEventID(kAEGetURL))

        guard ensureSingleInstance() else { return }
        observeForwardedURLs()
        configureNotifications()
        configureStatusItem()

        // The one owner of this hook. The scene must never assign it as well:
        // whichever ran last would win, and if that was this line the app
        // would ask itself to open the window, forever.
        state.onOpenWindow = { [weak self] in self?.showWindow() }
        state.checkForUpdates()

        iconSubscription = state.$iconState
            .receive(on: DispatchQueue.main)
            .sink { [weak self] iconState in self?.apply(iconState) }

        // Show "done/total" beside the icon during multi-file transfers.
        badgeSubscription = state.$transfer
            .receive(on: DispatchQueue.main)
            .sink { [weak self] transfer in
                guard let button = self?.statusItem?.button else { return }
                guard let transfer, transfer.receiving, transfer.files.count > 1 else {
                    button.attributedTitle = NSAttributedString(string: "")
                    return
                }
                let done = transfer.files.filter(\.completed).count
                button.attributedTitle = NSAttributedString(
                    string: " \(done)/\(transfer.files.count)",
                    attributes: [
                        .font: NSFont.monospacedDigitSystemFont(ofSize: 10, weight: .medium),
                        .foregroundColor: NSColor.secondaryLabelColor,
                    ])
            }

        // A menu-bar app opens no window on its own, so the introduction
        // would never be seen otherwise.
        if !state.hasOnboarded {
            showWindow()
        }
    }

    /// Two copies would share one staging directory and fight over the same
    /// mDNS advertisement: the second one wins the port and the first one's
    /// in-flight files can vanish. Hand over to the copy already running.
    ///
    /// Quitting on the spot would throw away whatever this copy was launched
    /// to do. macOS sends `droidharbor://` to whichever copy it considers
    /// canonical, which is often not the one already running, so the share
    /// extension's request lands here and has to be passed along.
    private func ensureSingleInstance() -> Bool {
        let others = NSRunningApplication.runningApplications(
            withBundleIdentifier: Bundle.main.bundleIdentifier ?? "")
            .filter { $0.processIdentifier != ProcessInfo.processInfo.processIdentifier }
        guard let existing = others.first else { return true }

        isDuplicate = true
        existing.activate()
        Task { @MainActor in
            // Long enough for a URL event to arrive, short enough that a
            // stray second copy does not linger.
            try? await Task.sleep(for: .milliseconds(1500))
            NSApp.terminate(nil)
        }
        return false
    }

    /// `droidharbor://send?...`, opened by the share extension. Nothing else
    /// uses the scheme, and anything that is not a send request is ignored
    /// rather than acted on: a URL can be opened by anyone.
    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls { handle(url) }
    }

    /// The Apple Event behind the same thing. AppKit's delegate call does not
    /// always arrive under the SwiftUI app lifecycle, and this one predates
    /// it and always does, so both are wired to the same funnel.
    @objc private func handleGetURLEvent(
        _ event: NSAppleEventDescriptor, withReply reply: NSAppleEventDescriptor
    ) {
        guard let raw = event.paramDescriptor(forKeyword: keyDirectObject)?.stringValue,
              let url = URL(string: raw)
        else {
            return
        }
        handle(url)
    }

    private func handle(_ url: URL) {
        // Both routes can deliver the same URL; act on it once.
        guard let shared = ShareRequest.parse(url), url != lastHandledURL else { return }
        lastHandledURL = url

        guard !isDuplicate else {
            // This copy is on its way out. Pass the request to the one that
            // is staying, which is the one holding the connection.
            DistributedNotificationCenter.default().postNotificationName(
                Self.forwardedURL,
                object: url.absoluteString,
                userInfo: nil,
                deliverImmediately: true)
            return
        }

        switch shared {
        case .files(let files): state.beginSend(files: files)
        case .text(let text): state.beginSend(text: text)
        }
    }

    /// Requests forwarded by a second copy that macOS launched and that then
    /// stood down.
    private func observeForwardedURLs() {
        DistributedNotificationCenter.default().addObserver(
            forName: Self.forwardedURL, object: nil, queue: .main
        ) { [weak self] notification in
            guard let raw = notification.object as? String, let url = URL(string: raw) else {
                return
            }
            MainActor.assumeIsolated { self?.handle(url) }
        }
    }

    func applicationShouldHandleReopen(
        _ sender: NSApplication, hasVisibleWindows: Bool
    ) -> Bool {
        guard !Self.isRunningTests else { return false }
        showWindow()
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        // The menu-bar item is the app's home; closing the window is not
        // quitting.
        false
    }

    /// Both of these live in System Settings, where the user can change them
    /// behind the app's back — and Settings sends them there itself, with its
    /// own "Open Settings" button. Coming back is the moment to re-read them.
    /// Without this the window keeps whatever it learned when it opened, so
    /// the warning about notifications being off survived turning them on.
    func applicationDidBecomeActive(_ notification: Notification) {
        guard !Self.isRunningTests else { return }
        state.refreshNotificationAuthorization()
        state.refreshLaunchAtLogin()
    }

    // MARK: - Status item

    private func configureStatusItem() {
        let hosting = NSHostingController(rootView: MenuView().environmentObject(state))
        hosting.sizingOptions = .preferredContentSize
        popover.contentViewController = hosting
        popover.behavior = .transient
        popover.animates = false
        popover.delegate = self

        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = item.button {
            button.image = Self.icon(for: .off)
            // A transparent overlay handles clicks and file drops; dropping
            // files on the icon starts a send.
            let drop = StatusDropView(frame: button.bounds)
            drop.autoresizingMask = [.width, .height]
            drop.onClick = { [weak self] in self?.togglePopover() }
            drop.onDrop = { [weak self] dropped in
                self?.dropHint.hide()
                switch dropped {
                case .files(let urls): self?.state.beginSend(files: urls)
                case .text(let text): self?.state.beginSend(text: text)
                }
            }
            drop.onDragChanged = { [weak self] dragged in
                guard let self, let button = self.statusItem?.button else { return }
                if let dragged {
                    self.dropHint.show(under: button, for: dragged)
                } else {
                    self.dropHint.hide()
                }
            }
            button.addSubview(drop)
        }
        statusItem = item
    }

    private func togglePopover() {
        guard let button = statusItem?.button else { return }
        if popover.isShown {
            popover.performClose(nil)
        } else if Date().timeIntervalSince(lastClose) > 0.25 {
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        }
    }

    private func apply(_ iconState: IconState) {
        statusItem?.button?.image = Self.icon(for: iconState)
        pulseTimer?.invalidate()
        pulseTimer = nil
        statusItem?.button?.alphaValue = 1

        guard iconState == .busy else { return }
        // Gentle pulse so an in-flight transfer is visible without opening
        // anything.
        let timer = Timer(timeInterval: 0.6, repeats: true) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                self.pulsePhase.toggle()
                self.statusItem?.button?.animator().alphaValue = self.pulsePhase ? 0.45 : 1
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        pulseTimer = timer
    }

    private static func icon(for iconState: IconState) -> NSImage? {
        let name: String
        switch iconState {
        case .off: name = "tray.and.arrow.down"
        case .on: name = "dot.radiowaves.left.and.right"
        case .busy: name = "arrow.up.arrow.down.circle.fill"
        }
        return NSImage(systemSymbolName: name, accessibilityDescription: "DroidHarbor")
    }

    // MARK: - Window

    func showWindow() {
        state.refreshNotificationAuthorization()
        // Become a regular app first: an accessory app cannot take focus, so
        // activating before this leaves the new window behind whatever was
        // in front (Xcode, most visibly).
        NSApp.setActivationPolicy(.regular)
        popover.performClose(nil)

        if let window = mainWindow, window.isVisible {
            raise(window)
            return
        }

        raiseWhenReady = true
        // Ask SwiftUI to create the window. If its scene has not appeared yet
        // there is no action to call, so raise whatever window exists instead
        // of doing nothing.
        if let openMainWindow {
            openMainWindow()
        } else {
            raise(mainWindow)
        }
    }

    func registerMainWindow(_ window: NSWindow?) {
        guard let window else { return }
        mainWindow = window
        if raiseWhenReady {
            raiseWhenReady = false
            raise(window)
        }
    }

    /// Called by the main scene once SwiftUI can open it on demand.
    func registerOpenAction(_ open: @escaping () -> Void) {
        openMainWindow = open
    }

    private func raise(_ window: NSWindow?) {
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
        window?.orderFrontRegardless()
    }

    /// A panel rather than a scene: a second SwiftUI Window would open
    /// itself at launch, which a menu-bar app should not do. The content is
    /// SwiftUI; only the container is AppKit.
    func showAbout() {
        if aboutWindow == nil {
            let window = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 380, height: 440),
                styleMask: [.titled, .closable],
                backing: .buffered, defer: false)
            window.title = String(localized: "About DroidHarbor")
            window.isReleasedWhenClosed = false
            // Deliberately not `sizingOptions = .preferredContentSize`. That
            // has AppKit ask SwiftUI for its size from inside
            // updateViewConstraints, and answering dirties the view graph,
            // which asks the window to schedule another constraint pass while
            // one is already running. AppKit raises on that re-entrancy, and
            // an uncaught raise here killed the app. The panel is measured in
            // sizeAboutWindowToFit() instead, outside any layout pass.
            window.contentViewController = NSHostingController(
                rootView: AboutView().environmentObject(state))
            aboutWindow = window
        }
        sizeAboutWindowToFit()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        aboutWindow?.makeKeyAndOrderFront(nil)
    }

    /// The panel has no one height: an available update adds a row to it. It
    /// is measured each time it is shown, which is a plain menu action rather
    /// than part of a display cycle, so asking SwiftUI for a size here cannot
    /// land in the middle of a constraint update.
    private func sizeAboutWindowToFit() {
        guard let window = aboutWindow,
              let content = window.contentViewController?.view
        else { return }

        content.layoutSubtreeIfNeeded()
        let fitting = content.fittingSize
        // A zero here means SwiftUI had no size to give; the window keeps the
        // size it was created with rather than collapsing.
        guard fitting.width > 0, fitting.height > 0 else { return }

        guard !window.isVisible else { return }
        window.setContentSize(fitting)
        // setContentSize pins the bottom-left corner, so a panel that grew
        // would drift off centre without this.
        window.center()
    }

    // MARK: - Notifications

    private func configureNotifications() {
        let center = UNUserNotificationCenter.current()
        center.delegate = self

        let accept = UNNotificationAction(
            identifier: Notifications.acceptAction,
            title: String(localized: "Accept"),
            options: [])
        let decline = UNNotificationAction(
            identifier: Notifications.declineAction,
            title: String(localized: "Decline"),
            options: [.destructive])
        // A finished transfer offers the two things worth doing with it
        // straight away; clicking the banner itself opens the app.
        let reveal = UNNotificationAction(
            identifier: Notifications.revealAction,
            title: String(localized: "Show in Finder"),
            options: [])
        let openItem = UNNotificationAction(
            identifier: Notifications.openAction,
            title: String(localized: "Open"),
            options: [.foreground])

        center.setNotificationCategories([
            UNNotificationCategory(
                identifier: Notifications.consentCategory,
                actions: [accept, decline],
                intentIdentifiers: [],
                options: []),
            UNNotificationCategory(
                identifier: Notifications.receivedFileCategory,
                actions: [openItem, reveal],
                intentIdentifiers: [],
                options: []),
            UNNotificationCategory(
                identifier: Notifications.transferDoneCategory,
                actions: [],
                intentIdentifiers: [],
                options: []),
        ])
        // Registering the categories prompts for nothing; asking for
        // permission does, and doing that here put a system dialog in front
        // of a first-run user before the app had explained itself. The
        // onboarding's last page asks instead. Installs that predate that
        // page never pass through it, so they are still asked here.
        if state.hasOnboarded {
            state.requestNotificationAuthorization()
        } else {
            state.refreshNotificationAuthorization()
        }
    }
}

extension AppDelegate: NSPopoverDelegate {
    func popoverDidClose(_ notification: Notification) {
        lastClose = Date()
    }
}

extension AppDelegate: UNUserNotificationCenterDelegate {
    // Show notifications even while the app is frontmost.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let action = response.actionIdentifier
        let info = response.notification.request.content.userInfo
        let session = (info[Notifications.sessionKey] as? String).flatMap(UInt64.init)
        let path = info[Notifications.pathKey] as? String

        Task { @MainActor in
            switch action {
            case Notifications.acceptAction:
                if let session { self.state.accept(session) }
            case Notifications.declineAction:
                if let session { self.state.decline(session) }
            case Notifications.revealAction:
                if let path { self.state.reveal(path: path) }
            case Notifications.openAction:
                if let path { self.state.open(path: path) }
            default:
                // Clicking the banner body opens the app, which is where the
                // transfer and its history are.
                self.showWindow()
            }
            completionHandler()
        }
    }
}

enum WindowID {
    static let main = "main"
}

/// Root of the main window scene. Its only extra job is handing the scene's
/// own window and open action to the delegate, which lives in AppKit and has
/// no SwiftUI environment of its own.
private struct MainWindowRoot: View {
    @Environment(\.openWindow) private var openWindow
    /// Passed in rather than re-derived: the delegate adaptor belongs to the
    /// App type, and asking for another one here would not be the same
    /// instance.
    let onWindow: (NSWindow?) -> Void
    let onOpenAction: (@escaping () -> Void) -> Void

    var body: some View {
        MainWindow()
            .background(WindowAccessor(onResolve: onWindow))
            .onAppear {
                onOpenAction { openWindow(id: WindowID.main) }
            }
    }
}
