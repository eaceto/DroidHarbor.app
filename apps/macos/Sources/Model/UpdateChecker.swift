import Foundation

// Update checking against a small JSON manifest published beside each
// release. Deliberately not an auto-updater: it notices a newer version and
// points at the download, leaving the install to the user.

struct AvailableUpdate: Equatable {
    let version: String
    let url: URL
    let notes: String?
}

/// The shape `release.sh` writes to updates.json.
private struct Manifest: Decodable {
    let version: String
    let url: String
    let notes: String?
}

enum UpdateChecker {
    /// Fetch the manifest and report a release newer than `current`.
    static func check(current: String, manifest: URL) async -> AvailableUpdate? {
        var request = URLRequest(url: manifest)
        // A stale CDN copy would keep reporting the previous version.
        request.cachePolicy = .reloadIgnoringLocalCacheData
        request.timeoutInterval = 15

        guard let (data, response) = try? await URLSession.shared.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200,
              let latest = try? JSONDecoder().decode(Manifest.self, from: data),
              let url = URL(string: latest.url),
              isNewer(latest.version, than: current)
        else {
            return nil
        }
        return AvailableUpdate(version: latest.version, url: url, notes: latest.notes)
    }

    /// Compare dotted numeric versions ("1.10.0" is newer than "1.9.3").
    static func isNewer(_ candidate: String, than current: String) -> Bool {
        let left = parts(candidate)
        let right = parts(current)
        for index in 0..<max(left.count, right.count) {
            let a = index < left.count ? left[index] : 0
            let b = index < right.count ? right[index] : 0
            if a != b { return a > b }
        }
        return false
    }

    private static func parts(_ version: String) -> [Int] {
        version.split(separator: ".").map { Int($0.filter(\.isNumber)) ?? 0 }
    }
}
