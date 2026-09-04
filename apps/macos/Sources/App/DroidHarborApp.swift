import SwiftUI

@main
struct DroidHarborApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        // A SwiftUI scene owns the main window, so SwiftUI knows the window's
        // real size. Hosting it in a hand-made NSWindow meant the split view
        // was laid out against an ideal size instead, nearly twice the
        // window's height, which pushed the sidebar off the top.
        Window("DroidHarbor", id: WindowID.main) {
            MainWindowRoot(
                onWindow: { [delegate] window in delegate.registerMainWindow(window) },
                onOpenAction: { [delegate] open in delegate.registerOpenAction(open) })
                .environmentObject(delegate.state)
        }
        .defaultSize(width: 900, height: 580)
        // Only the content's minimum should constrain the window. Under
        // .automatic the content's ideal size gets a say too, which is how a
        // tall-reporting first-run view could open the window past the screen
        // and take the size in defaultSize with it.
        .windowResizability(.contentMinSize)
        .commands {
            AppCommands(state: delegate.state) { delegate.showAbout() }
        }

        // No Settings scene. Declaring one is what puts Settings… in the app
        // menu, and this app keeps its settings in a section of the main
        // window, so the scene was an empty placeholder that the menu item
        // opened as an empty window. AppCommands puts the menu item back and
        // points it at the section.
    }
}
