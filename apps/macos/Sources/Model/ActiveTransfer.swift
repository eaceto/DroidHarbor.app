import Foundation

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
