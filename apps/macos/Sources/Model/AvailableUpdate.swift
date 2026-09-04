import Foundation

/// A release newer than the running version, as reported by UpdateChecker:
/// what it is and where to get it.
struct AvailableUpdate: Equatable {
    let version: String
    let url: URL
    let notes: String?
}
