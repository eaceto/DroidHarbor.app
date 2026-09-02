import Foundation

// What is staged to send.
//
// Quick Share carries text as its own kind of attachment rather than as a
// file, and the phone offers different actions depending on what the text is
// (a browser for a link, a map for an address, the dialer for a number), so
// the kind travels with the content.

enum SendPayload: Equatable {
    case files([URL])
    case text(OutboundText)

    var files: [URL] {
        guard case .files(let urls) = self else { return [] }
        return urls
    }

    var text: OutboundText? {
        guard case .text(let text) = self else { return nil }
        return text
    }

    var isEmpty: Bool {
        switch self {
        case .files(let urls): return urls.isEmpty
        case .text(let text): return text.content.isEmpty
        }
    }

    /// Names for the staged-items list: one per file, or the text's title.
    var itemNames: [String] {
        switch self {
        case .files(let urls): return urls.map(\.lastPathComponent)
        case .text(let text): return [text.title]
        }
    }
}
