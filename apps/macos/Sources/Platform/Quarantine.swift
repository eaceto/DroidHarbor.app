import Foundation

/// Marks received files with `com.apple.quarantine` so Gatekeeper treats
/// them like any other download (spec §8: never let received content run
/// without the usual checks).
enum Quarantine {
    /// `QTN_FLAG_DOWNLOAD` (0x0001) plus the standard high bit browsers set.
    ///
    /// Deliberately *without* `QTN_FLAG_SANDBOX` (0x0002): that bit tells the
    /// system the file belongs to a sandboxed app's container, and every
    /// later attempt to reveal or open it fails with "sandbox extension
    /// creation failed".
    private static let flags = "0081"

    static func mark(path: String) {
        // flags;unix-time-hex;agent-name;  (no event UUID for third parties)
        let stamp = String(UInt64(Date().timeIntervalSince1970), radix: 16)
        let value = "\(flags);\(stamp);DroidHarbor;"
        _ = value.withCString { valuePtr in
            setxattr(path, "com.apple.quarantine", valuePtr, strlen(valuePtr), 0, 0)
        }
    }
}
