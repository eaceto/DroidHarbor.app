import Foundation

/// One file offered in an incoming transfer, with live progress.
///
/// Mirrors the domain's plain-data events; no behaviour beyond derived
/// values such as the progress fraction.
struct OfferedFile: Identifiable, Equatable {
    let id: Int
    let name: String
    let size: UInt64
    var bytesTransferred: UInt64 = 0
    var completed = false

    var fraction: Double? {
        guard size > 0 else { return nil }
        return min(1, Double(bytesTransferred) / Double(size))
    }
}
