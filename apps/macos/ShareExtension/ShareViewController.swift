import AppKit
import UniformTypeIdentifiers

// "Share… → DroidHarbor" in Finder, Photos, Safari and anywhere else macOS
// offers the share sheet.
//
// The extension does no work of its own: it collects what was shared, hands
// it to the app through a droidharbor:// URL, and closes. Transfers belong to
// the app, which owns the connection to the phone and the UI for choosing a
// device. Doing any of that here, in a sandboxed process with no window,
// would be a second implementation to keep in step with the first.

final class ShareViewController: NSViewController {
    override func loadView() {
        // No UI of its own: the app takes over as soon as it opens.
        view = NSView(frame: .zero)
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        Task { await handOver() }
    }

    private func handOver() async {
        let attachments = (extensionContext?.inputItems as? [NSExtensionItem] ?? [])
            .flatMap { $0.attachments ?? [] }

        var files: [URL] = []
        var text: String?

        for provider in attachments {
            if let url = await provider.loadURL() {
                if url.isFileURL {
                    files.append(stabilized(url))
                } else {
                    // A web address shared from a browser.
                    text = text ?? url.absoluteString
                }
            } else if let shared = await provider.loadText() {
                text = text ?? shared
            }
        }

        let shared: ShareRequest.Shared? = files.isEmpty
            ? text.map(ShareRequest.Shared.text)
            : .files(files)

        guard let shared, let destination = ShareRequest.url(for: shared) else {
            extensionContext?.cancelRequest(withError: NSError(
                domain: Bundle.main.bundleIdentifier ?? "droidharbor.share", code: 1,
                userInfo: [NSLocalizedDescriptionKey:
                    String(localized: "There is nothing here that can be sent.")]))
            return
        }

        deliver(destination)
        extensionContext?.completeRequest(returningItems: nil)
    }

    /// A path the file will still be at once this process is gone.
    ///
    /// Finder shares hand over the user's real file, which outlives the
    /// extension and passes through untouched. Photos, Mail and friends vend
    /// a temporary copy inside this extension's own sandbox, and macOS may
    /// reap it the moment the request completes — before the app has read
    /// it. Those are copied into the container's persistent outbox, which
    /// the app sweeps of old batches at launch.
    private lazy var outboxBatch = AppInfo.shareOutbox?
        .appendingPathComponent(UUID().uuidString, isDirectory: true)

    private func stabilized(_ url: URL) -> URL {
        let path = url.path
        let ephemeral = path.hasPrefix(NSTemporaryDirectory())
            || path.hasPrefix(NSHomeDirectory())
            || path.hasPrefix("/private/var/folders")
            || path.hasPrefix("/var/folders")
        guard ephemeral, let batch = outboxBatch else { return url }

        do {
            try FileManager.default.createDirectory(at: batch, withIntermediateDirectories: true)
            let stable = batch.appendingPathComponent(url.lastPathComponent)
            try FileManager.default.copyItem(at: url, to: stable)
            return stable
        } catch {
            // Better to hand over the original and race the teardown than to
            // hand over nothing.
            NSLog("DroidHarbor: could not stabilize \(url.lastPathComponent): \(error)")
            return url
        }
    }

    /// Get the request to the app.
    ///
    /// `NSExtensionContext.open` is the documented route and it does not work
    /// for share extensions on macOS: it returns immediately having done
    /// nothing. So the workspace is asked directly, which both launches the
    /// app and delivers the URL, and a notification is posted as well for the
    /// case where the app is already running. The app ignores a repeat of the
    /// same URL, so arriving twice is harmless.
    private func deliver(_ url: URL) {
        DistributedNotificationCenter.default().postNotificationName(
            AppInfo.forwardedURLNotification,
            object: url.absoluteString,
            userInfo: nil,
            deliverImmediately: true)

        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        NSWorkspace.shared.open(url, configuration: configuration) { _, error in
            if let error {
                NSLog("DroidHarbor: could not open \(url.scheme ?? "?"): \(error)")
            }
        }
    }
}

// MARK: - Item providers, awaited

private extension NSItemProvider {
    /// A file or web URL, whichever this provider carries.
    func loadURL() async -> URL? {
        await load(UTType.url) { item in
            switch item {
            case let url as URL: return url
            case let data as Data: return URL(dataRepresentation: data, relativeTo: nil)
            default: return nil
            }
        }
    }

    func loadText() async -> String? {
        await load(UTType.plainText) { item in
            switch item {
            case let text as String: return text
            case let data as Data: return String(data: data, encoding: .utf8)
            default: return nil
            }
        }
    }

    private func load<T>(
        _ type: UTType, convert: @escaping (NSSecureCoding?) -> T?
    ) async -> T? {
        guard hasItemConformingToTypeIdentifier(type.identifier) else { return nil }
        return await withCheckedContinuation { continuation in
            loadItem(forTypeIdentifier: type.identifier) { item, _ in
                continuation.resume(returning: convert(item))
            }
        }
    }
}
