import Foundation

/// A text, link or Wi-Fi payload: received, copied to the clipboard, and
/// shown until dismissed since there is no file to point at afterwards.
struct ReceivedText: Identifiable, Equatable {
    let id = UUID()
    /// "text" | "link" | "wifi"
    let kind: String
    let description: String
    let content: String
}
