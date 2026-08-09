import AppKit

/// A small panel that appears under the menu-bar icon while files are being
/// dragged onto it: the affordance Dropzone and Menu Drop use, so the target
/// is a visible area rather than a 22-point icon.
@MainActor
final class DropHintPanel {
    private var panel: NSPanel?

    private static func title(for drop: StatusDrop) -> String {
        switch drop {
        case .files(let urls):
            return urls.count == 1
                ? String(localized: "Release to send 1 file")
                : String(localized: "Release to send \(urls.count) files")
        case .text:
            return String(localized: "Release to send this text")
        }
    }

    func show(under button: NSStatusBarButton, for drop: StatusDrop) {
        guard panel == nil, let screenRect = button.window?.convertToScreen(button.frame) else {
            return
        }

        let label = NSTextField(labelWithString: Self.title(for: drop))
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.alignment = .center
        label.sizeToFit()

        let width = max(200, label.frame.width + 40)
        let height: CGFloat = 54
        let content = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: width, height: height))
        content.material = .popover
        content.state = .active
        content.wantsLayer = true
        content.layer?.cornerRadius = 12
        content.layer?.borderWidth = 2
        content.layer?.borderColor = NSColor.controlAccentColor.cgColor

        label.frame = NSRect(
            x: 0, y: (height - label.frame.height) / 2 - 1,
            width: width, height: label.frame.height)
        content.addSubview(label)

        let panel = NSPanel(
            contentRect: NSRect(
                x: screenRect.midX - width / 2,
                y: screenRect.minY - height - 6,
                width: width, height: height),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false)
        panel.contentView = content
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .statusBar
        // Never steal the drag: the panel is feedback, not a target.
        panel.ignoresMouseEvents = true
        panel.collectionBehavior = [.canJoinAllSpaces, .transient]
        panel.orderFrontRegardless()
        self.panel = panel
    }

    func hide() {
        panel?.orderOut(nil)
        panel = nil
    }
}
