import Foundation

// Value types describing what a transfer looks like to the UI. They
// mirror the domain's plain-data events; no behaviour beyond derived
// values such as progress fractions and rates.

/// One file offered in an incoming transfer, with live progress.
struct OfferedFile: Identifiable, Equatable {
    let id: Int
    let name: String
    let size: UInt64
    var bytesTransferred: UInt64 = 0
    var completed = false

    var fraction: Double? {
        guard size > 0 else { return nil }
        return min(1, Double(bytesTransferred) / Double(size))
    }
}

/// A text, link or Wi-Fi payload: received, copied to the clipboard, and
/// shown until dismissed since there is no file to point at afterwards.
struct ReceivedText: Identifiable, Equatable {
    let id = UUID()
    /// "text" | "link" | "wifi"
    let kind: String
    let description: String
    let content: String
}

/// Transfer rate and remaining time, derived from progress samples.
struct TransferRate {
    var bytesPerSecond: Double = 0
    var secondsRemaining: Double?

    private var lastBytes: UInt64 = 0
    private var lastAt: Date?

    /// Feed a progress sample; smoothed so the figure does not jump.
    ///
    /// Samples closer together than the window are skipped, but the baseline
    /// is deliberately *not* moved when that happens: a fast transfer reports
    /// every few milliseconds, and resetting the clock each time meant the
    /// window never elapsed and the rate stayed at zero for the whole
    /// transfer.
    mutating func sample(bytes: UInt64, total: UInt64) {
        let now = Date()
        guard let lastAt, bytes >= lastBytes else {
            // First sample, or a transfer that restarted: rebase quietly
            // rather than report a rate from a bogus delta.
            lastBytes = bytes
            self.lastAt = now
            return
        }
        let elapsed = now.timeIntervalSince(lastAt)
        guard elapsed > 0.15 else { return }
        defer { lastBytes = bytes; self.lastAt = now }
        let instant = Double(bytes - lastBytes) / elapsed
        bytesPerSecond = bytesPerSecond == 0 ? instant : bytesPerSecond * 0.7 + instant * 0.3
        if total > bytes, bytesPerSecond > 1 {
            secondsRemaining = Double(total - bytes) / bytesPerSecond
        } else {
            secondsRemaining = nil
        }
    }
}

/// An incoming transfer as the UI sees it.
struct ActiveTransfer {
    let session: UInt64
    let senderName: String
    var files: [OfferedFile]
    let totalBytes: UInt64
    let token: String
    /// Set when the payload is text, a link or Wi-Fi credentials rather
    /// than files.
    var textPreview: String?
    var bytesReceived: UInt64 = 0
    var currentFile: String = ""
    var completedNames: Set<String> = []
    var rate = TransferRate()
    /// False while the user has not answered the consent prompt.
    var receiving = false
}

/// A nearby Android device found during discovery.
struct Endpoint: Identifiable, Equatable {
    let id: String
    var name: String
    /// "phone" | "tablet" | "laptop" | "unknown"
    var kind: String
    /// False for devices seen before but not advertising right now.
    var present: Bool = true

    var symbolName: String {
        switch kind {
        case "tablet": return "ipad"
        case "laptop": return "laptopcomputer"
        case "phone": return "iphone"
        default: return "display"
        }
    }
}

/// An outbound transfer (Mac → phone) as the UI sees it.
struct OutboundTransfer {
    let session: UInt64
    let targetName: String
    let payload: SendPayload
    var totalBytes: UInt64 = 0
    var bytesSent: UInt64 = 0
    var rate = TransferRate()
    var awaitingConsent = true
    /// The 4-digit code the phone is showing while it waits for its user to
    /// accept. Empty when the protocol layer reported none.
    var token: String = ""

    /// Filenames, or the title of the text being sent.
    var itemNames: [String] { payload.itemNames }
    /// Set when text is being sent rather than files, so the transfer can be
    /// shown and recorded as the kind it is.
    var text: OutboundText? { payload.text }
    var symbolName: String { payload.text?.symbolName ?? "doc" }
}

/// What the menu-bar icon should show.
enum IconState {
    case off
    case on
    case busy
}
