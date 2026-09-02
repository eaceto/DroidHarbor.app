import Foundation

/// What a drag onto the menu-bar icon is carrying.
enum StatusDrop {
    case files([URL])
    case text(String)

    /// How many items the drop hint should describe.
    var count: Int {
        switch self {
        case .files(let urls): return urls.count
        case .text: return 1
        }
    }
}
