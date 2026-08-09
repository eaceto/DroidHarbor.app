import Foundation

// Where the transfer history lives.
//
// The entries are the user's own record of what arrived, so they are kept as
// a readable JSON file under the home directory rather than buried in a
// binary preferences plist. Preferences hold only the path, so the file can
// be moved and still be found.

enum HistoryStore {
    /// Preference key holding the path of the history file.
    static let locationKey = "historyFile"
    /// Older builds kept the entries themselves under this key.
    private static let legacyEntriesKey = "history"

    /// `~/.droidharbor`, created on demand. A development build keeps its
    /// own folder, so testing never writes into the real history.
    static var defaultDirectory: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(AppInfo.stateDirectoryName, isDirectory: true)
    }

    /// The history file, recording its location in preferences the first
    /// time so the plist always points at the file in use.
    static func fileURL(for defaults: UserDefaults = .standard) -> URL {
        if let stored = defaults.string(forKey: locationKey), !stored.isEmpty {
            return URL(fileURLWithPath: (stored as NSString).expandingTildeInPath)
        }
        let url = defaultDirectory.appendingPathComponent("history.json")
        defaults.set(url.path, forKey: locationKey)
        return url
    }

    static func load(from defaults: UserDefaults = .standard) -> [HistoryEntry] {
        let url = fileURL(for: defaults)
        if let data = try? Data(contentsOf: url), let entries = decode(data) {
            return entries
        }
        // Nothing on disk yet: adopt whatever an older build left in
        // preferences, then write it out in the new place.
        guard let legacy = defaults.data(forKey: legacyEntriesKey),
              let entries = decode(legacy)
        else {
            return []
        }
        save(entries, to: defaults)
        defaults.removeObject(forKey: legacyEntriesKey)
        return entries
    }

    static func save(_ entries: [HistoryEntry], to defaults: UserDefaults = .standard) {
        let url = fileURL(for: defaults)
        let encoder = JSONEncoder()
        // A file someone might open deserves to be readable: indented, with
        // stable key order and dates as timestamps rather than a float.
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        encoder.dateEncodingStrategy = .iso8601

        // Deliberately uncapped: this is the user's own record of what
        // arrived, and truncating it silently loses history they may want.
        guard let data = try? encoder.encode(entries) else { return }
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: url, options: .atomic)
        } catch {
            NSLog("DroidHarbor: could not write history to \(url.path): \(error)")
        }
    }

    /// Dates were once encoded as a bare number; both forms must load.
    private static func decode(_ data: Data) -> [HistoryEntry]? {
        let iso = JSONDecoder()
        iso.dateDecodingStrategy = .iso8601
        if let entries = try? iso.decode([HistoryEntry].self, from: data) {
            return entries
        }
        return try? JSONDecoder().decode([HistoryEntry].self, from: data)
    }
}
