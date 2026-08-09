import AppKit
import SwiftUI

// Hands the hosting NSWindow back to non-SwiftUI code.
//
// The menu-bar item lives in AppKit and has to raise and focus the window
// SwiftUI created. Searching NSApp.windows for it means guessing at
// SwiftUI's private window identifiers; this reports the real one.

struct WindowAccessor: NSViewRepresentable {
    let onResolve: (NSWindow?) -> Void

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        // The window is not attached during makeNSView, so ask once the view
        // has joined the hierarchy.
        DispatchQueue.main.async { onResolve(view.window) }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async { onResolve(view.window) }
    }
}
