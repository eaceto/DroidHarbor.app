import Foundation

/// A nearby Android device found during discovery.
struct Endpoint: Identifiable, Equatable {
    let id: String
    var name: String
    /// "phone" | "tablet" | "laptop" | "unknown"
    var kind: String
    /// False for devices seen before but not advertising right now.
    var present: Bool = true

    var symbolName: String {
        switch kind {
        case "tablet": return "ipad"
        case "laptop": return "laptopcomputer"
        case "phone": return "iphone"
        default: return "display"
        }
    }
}
