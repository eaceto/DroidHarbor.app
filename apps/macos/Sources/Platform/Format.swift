import Foundation

// Human-readable renderings of the numbers the domain reports.

enum Format {
    static func bytes(_ value: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(value), countStyle: .file)
    }

    static func rate(_ bytesPerSecond: Double) -> String {
        String(
            localized: "\(bytes(UInt64(max(0, bytesPerSecond))))/s",
            comment: "Transfer speed, e.g. 4.2 MB/s")
    }

    /// Coarse on purpose: a precise countdown that jitters reads as broken.
    static func remaining(_ seconds: Double) -> String {
        let total = Int(seconds.rounded())
        if total < 10 { return String(localized: "a few seconds left") }
        if total < 60 { return String(localized: "\(total) seconds left") }
        let minutes = (total + 30) / 60
        return minutes == 1
            ? String(localized: "about a minute left")
            : String(localized: "about \(minutes) minutes left")
    }
}
