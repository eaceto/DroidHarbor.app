import AppKit

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

/// Transparent overlay on the status-item button: forwards clicks (left and
/// right alike) and accepts drops, so files, links or text can be sent by
/// dropping them straight onto the menu-bar icon.
final class StatusDropView: NSView {
    var onClick: (() -> Void)?
    var onDrop: ((StatusDrop) -> Void)?
    /// Called with the drag's contents when it enters, and nil when it
    /// leaves or is dropped.
    var onDragChanged: ((StatusDrop?) -> Void)?

    private var highlighted = false {
        didSet { needsDisplay = true }
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        registerForDraggedTypes([.fileURL, .URL, .string])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("not used")
    }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        // The status button may not be laid out when the overlay is added.
        if let superview {
            frame = superview.bounds
        }
    }

    override func mouseDown(with event: NSEvent) {
        onClick?()
    }

    override func rightMouseDown(with event: NSEvent) {
        onClick?()
    }

    override func draw(_ dirtyRect: NSRect) {
        guard highlighted else { return }
        NSColor.controlAccentColor.withAlphaComponent(0.25).setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: 4, yRadius: 4).fill()
    }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        let dragged = contents(of: sender)
        highlighted = dragged != nil
        if let dragged { onDragChanged?(dragged) }
        return highlighted ? .copy : []
    }

    override func draggingExited(_ sender: NSDraggingInfo?) {
        highlighted = false
        onDragChanged?(nil)
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        highlighted = false
        onDragChanged?(nil)
        guard let dropped = contents(of: sender) else { return false }
        onDrop?(dropped)
        return true
    }

    /// URLs are read first: dragging a link out of a browser puts both the
    /// address and its title on the pasteboard, and the address is the one
    /// worth sending.
    private func contents(of sender: NSDraggingInfo) -> StatusDrop? {
        let pasteboard = sender.draggingPasteboard
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self]) as? [URL], !urls.isEmpty {
            return .files(urls)
        }
        if let text = pasteboard.string(forType: .string),
           !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        {
            return .text(text)
        }
        return nil
    }
}
