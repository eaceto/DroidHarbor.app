import SwiftUI

// The menu bar, declared rather than assembled by hand.
//
// SwiftUI contributes the standard App, Edit, View, Window and Help menus for
// free (which is where Cut/Copy/Paste and ⌘Q come from), and these commands
// add the app's own entries on top. Shortcuts belong here rather than on
// hidden buttons: the menu is where macOS users look for them.

struct AppCommands: Commands {
    @ObservedObject var state: AppState
    let showAbout: () -> Void

    var body: some Commands {
        CommandGroup(replacing: .appInfo) {
            Button("About DroidHarbor") { showAbout() }
        }

        // Settings live in a section of the main window, not a window of
        // their own, so the app menu's usual item selects that section and
        // brings the window forward. Without this the item would be gone
        // entirely: it exists only because a Settings scene or this group
        // claims the placement, and ⌘, is where macOS users reach for it.
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") {
                state.selectSection(MainWindow.Section.settings.rawValue)
                state.onOpenWindow?()
            }
            .keyboardShortcut(",", modifiers: .command)
        }

        CommandGroup(after: .newItem) {
            Button(state.receiving ? "Stop Receiving" : "Start Receiving") {
                state.setReceiving(!state.receiving)
            }
            .keyboardShortcut("r", modifiers: .command)

            Button("Send Files…") { state.chooseFilesToSend() }
                .keyboardShortcut("o", modifiers: .command)

            Button("Send Clipboard Text") { state.sendClipboardText() }
                .keyboardShortcut("v", modifiers: [.command, .shift])

            Divider()

            Button("Receive for 10 Minutes") { state.receiveTemporarily(minutes: 10) }
        }

        // Section switching sits in View, beside the sidebar toggle macOS
        // puts there itself.
        CommandGroup(after: .sidebar) {
            Divider()
            ForEach(Array(MainWindow.Section.allCases.enumerated()), id: \.offset) { index, item in
                Button(item.title) { state.selectSection(item.rawValue) }
                    .keyboardShortcut(
                        KeyEquivalent(Character("\(index + 1)")), modifiers: .command)
            }
        }

        CommandGroup(replacing: .help) {
            Button("DroidHarbor Help") { state.showOnboardingAgain() }
            Divider()
            Link("Source Code", destination: URL(string: Links.source)!)
            Button("Check for Updates…") { state.checkForUpdates(userInitiated: true) }
        }
    }
}
