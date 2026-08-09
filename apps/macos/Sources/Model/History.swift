import SwiftUI

/// One completed transfer, persisted across launches. Files, links and text
/// all land here; what differs is what can be done with them afterwards.
struct HistoryEntry: Codable, Identifiable, Equatable {
    enum Direction: String, Codable {
        case received
        case sent
    }

    enum Kind: String, Codable {
        case files
        case link
        case text
        case wifi
        case contact
        case calendar
        case phone
        case email
        case map
    }

    let id: UUID
    let direction: Direction
    /// Peer device name as it presented itself.
    let peer: String
    let date: Date
    /// Absolute paths for file transfers; empty for links and text.
    let paths: [String]
    /// The link or text itself, which never touches the disk.
    let content: String?
    let kind: Kind

    init(
        id: UUID = UUID(),
        direction: Direction,
        peer: String,
        date: Date = Date(),
        paths: [String] = [],
        content: String? = nil,
        kind: Kind = .files
    ) {
        self.id = id
        self.direction = direction
        self.peer = peer
        self.date = date
        self.paths = paths
        self.content = content
        self.kind = kind
    }

    /// Entries written before links were recorded have neither field, so
    /// both decode with defaults rather than failing the whole history.
    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(UUID.self, forKey: .id)
        direction = try values.decode(Direction.self, forKey: .direction)
        peer = try values.decode(String.self, forKey: .peer)
        date = try values.decode(Date.self, forKey: .date)
        paths = try values.decodeIfPresent([String].self, forKey: .paths) ?? []
        content = try values.decodeIfPresent(String.self, forKey: .content)
        kind = (try? values.decodeIfPresent(Kind.self, forKey: .kind)).flatMap { $0 } ?? .files
    }

    /// Whether the row is named by a filename rather than by its content.
    var isFile: Bool { kind == .files || (paths.isEmpty == false && content == nil) }

    var primaryName: String {
        if let content, !isFile { return content }
        guard let first = paths.first else { return String(localized: "Transfer") }
        return URL(fileURLWithPath: first).lastPathComponent
    }

    var summary: String {
        guard isFile, paths.count > 1 else { return primaryName }
        return String(localized: "\(primaryName) and \(paths.count - 1) more")
    }

    /// What the filter groups entries by. Files are classified by their
    /// extension, since "an image" is what the user is looking for, not
    /// "a file transfer".
    enum Category: String, CaseIterable, Identifiable {
        case all
        case image
        case video
        case audio
        case document
        case link
        case contact
        case calendar
        case phone
        case email
        case map
        case other

        var id: String { rawValue }

        var title: LocalizedStringKey {
            switch self {
            case .all: return "All"
            case .image: return "Images"
            case .video: return "Video"
            case .audio: return "Audio"
            case .document: return "Docs"
            case .link: return "Links"
            case .contact: return "Contacts"
            case .calendar: return "Events"
            case .phone: return "Phones"
            case .email: return "Emails"
            case .map: return "Places"
            case .other: return "Other"
            }
        }

        static func forExtension(_ ext: String) -> Category {
            switch ext.lowercased() {
            case "jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "bmp", "tiff", "tif",
                 "svg", "avif", "raw", "dng":
                return .image
            case "mp4", "mov", "m4v", "avi", "mkv", "webm", "3gp", "mpg", "mpeg", "wmv":
                return .video
            case "mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "aiff", "wma":
                return .audio
            case "pdf", "doc", "docx", "pages", "txt", "rtf", "md", "csv", "xls", "xlsx",
                 "numbers", "ppt", "pptx", "key", "epub":
                return .document
            default:
                return .other
            }
        }
    }

    /// A multi-file transfer is classified by its first file.
    var category: Category {
        switch kind {
        case .link: return .link
        case .contact: return .contact
        case .calendar: return .calendar
        case .phone: return .phone
        case .email: return .email
        case .map: return .map
        case .text, .wifi: return .other
        case .files:
            guard let first = paths.first else { return .other }
            return Category.forExtension((first as NSString).pathExtension)
        }
    }

    /// True when the payload is a file on disk rather than content we hold.
    var hasFile: Bool { !paths.isEmpty }

    /// Free-text match over the things a person would actually type: part of
    /// a name, an extension, a domain, a scheme, or who sent it.
    func matches(_ query: String) -> Bool {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !needle.isEmpty else { return true }
        // A leading dot reads as "the extension", not part of a name.
        let asExtension = needle.hasPrefix(".") ? String(needle.dropFirst()) : needle

        if peer.lowercased().contains(needle) { return true }

        if let content = content?.lowercased() {
            if content.contains(needle) { return true }
            if let url = URL(string: content) {
                if url.scheme?.lowercased().hasPrefix(needle) == true { return true }
                if url.host?.lowercased().contains(needle) == true { return true }
            }
        }

        return paths.contains { path in
            let name = (path as NSString).lastPathComponent.lowercased()
            if name.contains(needle) { return true }
            return (path as NSString).pathExtension.lowercased() == asExtension
        }
    }

    var symbolName: String {
        switch kind {
        case .files: return direction == .received
            ? "arrow.down.circle.fill" : "arrow.up.circle.fill"
        case .link: return "link.circle.fill"
        case .text: return "text.quote"
        case .wifi: return "wifi.circle.fill"
        case .contact: return "person.crop.circle.fill"
        case .calendar: return "calendar.circle.fill"
        case .phone: return "phone.circle.fill"
        case .email: return "envelope.circle.fill"
        case .map: return "mappin.circle.fill"
        }
    }
}
