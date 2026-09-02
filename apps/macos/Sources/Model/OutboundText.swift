import Foundation

/// Text staged for sending, classified so it arrives on the phone as the
/// kind it is rather than as anonymous text.
struct OutboundText: Equatable {
    /// Shares the history kinds, so a link sent from this Mac is recorded
    /// exactly like a link received from the phone.
    let kind: HistoryEntry.Kind
    let content: String

    init(content: String) {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        self.content = trimmed
        self.kind = PayloadClassifier.kind(forText: trimmed, declared: "text")
    }

    /// What the protocol layer calls this kind. Only four are distinguished
    /// on the wire; an email address or a contact travels as plain text.
    var wireKind: String {
        switch kind {
        case .link: return "link"
        case .map: return "address"
        case .phone: return "phone"
        default: return "text"
        }
    }

    /// The title the phone shows while asking whether to accept. A link is
    /// best recognised by its host; anything else by its opening words.
    var title: String {
        if kind == .link, let host = URL(string: content)?.host {
            return host
        }
        let firstLine = content.split(
            separator: "\n", maxSplits: 1, omittingEmptySubsequences: true
        ).first.map(String.init) ?? content
        return firstLine.count <= 60 ? firstLine : String(firstLine.prefix(59)) + "\u{2026}"
    }

    var symbolName: String {
        switch kind {
        case .link: return "link"
        case .phone: return "phone"
        case .email: return "envelope"
        case .map: return "mappin"
        case .contact: return "person.crop.circle"
        case .calendar: return "calendar"
        default: return "text.quote"
        }
    }
}
