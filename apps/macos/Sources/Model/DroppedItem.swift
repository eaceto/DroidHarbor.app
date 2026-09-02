import CoreTransferable
import Foundation

/// One item from a drag onto a drop target: a file, a web address, or text.
/// Reading both kinds means a link dragged out of a browser and a selection
/// dragged out of a document are equally droppable.
struct DroppedItem: Transferable {
    var url: URL?
    var text: String?

    static var transferRepresentation: some TransferRepresentation {
        ProxyRepresentation(importing: { (url: URL) in DroppedItem(url: url) })
        ProxyRepresentation(importing: { (text: String) in DroppedItem(text: text) })
    }
}
