import Foundation

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
