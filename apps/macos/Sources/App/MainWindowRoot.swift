import SwiftUI

/// Root of the main window scene. Its only extra job is handing the scene's
/// own window and open action to the delegate, which lives in AppKit and has
/// no SwiftUI environment of its own.
struct MainWindowRoot: View {
    @Environment(\.openWindow) private var openWindow
    /// Passed in rather than re-derived: the delegate adaptor belongs to the
    /// App type, and asking for another one here would not be the same
    /// instance.
    let onWindow: (NSWindow?) -> Void
    let onOpenAction: (@escaping () -> Void) -> Void

    var body: some View {
        MainWindow()
            .background(WindowAccessor(onResolve: onWindow))
            .onAppear {
                onOpenAction { openWindow(id: WindowID.main) }
            }
    }
}
