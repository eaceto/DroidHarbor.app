import Foundation

// Works out what a transfer actually is.
//
// Quick Share only distinguishes files, text and Wi-Fi credentials, and
// rqs_lib flattens the protocol's ADDRESS and PHONE_NUMBER hints into plain
// text before we see them. A contact, an event, a phone number or a map
// pin therefore arrives as an ordinary .vcf, .ics, or line of text, so the
// app recovers the meaning here and treats each as its own kind.

enum PayloadClassifier {
    /// Classify a received file by extension, falling back to a quick look
    /// at its first line for the two formats that matter and are trivially
    /// recognisable.
    static func kind(forFile path: String) -> HistoryEntry.Kind {
        switch (path as NSString).pathExtension.lowercased() {
        case "vcf", "vcard":
            return .contact
        case "ics", "ical", "ifb", "icalendar":
            return .calendar
        default:
            return sniff(fileAt: path) ?? .files
        }
    }

    /// Classify received text. `declared` is what the protocol layer said
    /// ("link", "text", "wifi"); the content itself is more reliable, since
    /// a phone number and an address both arrive labelled as plain text.
    static func kind(forText content: String, declared: String) -> HistoryEntry.Kind {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        let lower = trimmed.lowercased()

        if declared == "wifi" { return .wifi }
        if lower.hasPrefix("begin:vcard") { return .contact }
        if lower.hasPrefix("begin:vcalendar") || lower.contains("begin:vevent") {
            return .calendar
        }
        if lower.hasPrefix("tel:") || isPhoneNumber(trimmed) { return .phone }
        if lower.hasPrefix("mailto:") || isEmailAddress(trimmed) { return .email }
        if lower.hasPrefix("geo:") || isMapLink(trimmed) { return .map }
        if lower.hasPrefix("http://") || lower.hasPrefix("https://") { return .link }
        return declared == "link" ? .link : .text
    }

    // MARK: - Recognisers

    /// Deliberately strict: a bare run of digits is far more likely to be a
    /// code or an ID than a number worth offering to dial.
    static func isPhoneNumber(_ text: String) -> Bool {
        let stripped = text.replacingOccurrences(
            of: "[ ()\\-./]", with: "", options: .regularExpression)
        let body = stripped.hasPrefix("+") ? String(stripped.dropFirst()) : stripped
        guard body.count >= 7, body.count <= 15, body.allSatisfy(\.isNumber) else {
            return false
        }
        // A plain local number without punctuation or a country code is too
        // ambiguous to claim; require one of those signals.
        return stripped.hasPrefix("+") || text.contains(where: { " ()-.".contains($0) })
    }

    static func isEmailAddress(_ text: String) -> Bool {
        guard !text.contains(" "), text.count <= 254 else { return false }
        return text.range(
            of: "^[A-Z0-9._%+-]+@[A-Z0-9.-]+\\.[A-Z]{2,}$",
            options: [.regularExpression, .caseInsensitive]) != nil
    }

    static func isMapLink(_ text: String) -> Bool {
        guard let host = URL(string: text)?.host?.lowercased() else { return false }
        return host == "maps.apple.com" || host == "maps.google.com"
            || (host.contains("google.") && text.lowercased().contains("/maps"))
    }

    /// The payload without its scheme: "tel:+441234" → "+441234".
    static func value(from content: String, strippingScheme scheme: String) -> String {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.lowercased().hasPrefix("\(scheme):") else { return trimmed }
        let value = String(trimmed.dropFirst(scheme.count + 1))
        return value.removingPercentEncoding ?? value
    }

    private static func sniff(fileAt path: String) -> HistoryEntry.Kind? {
        guard let handle = FileHandle(forReadingAtPath: path) else { return nil }
        defer { try? handle.close() }
        guard let head = try? handle.read(upToCount: 64),
              let text = String(data: head, encoding: .utf8)?.lowercased()
        else {
            return nil
        }
        if text.hasPrefix("begin:vcard") { return .contact }
        if text.hasPrefix("begin:vcalendar") { return .calendar }
        return nil
    }
}
