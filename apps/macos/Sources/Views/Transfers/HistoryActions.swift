import SwiftUI

/// What a history entry offers, built once and drawn twice: as icon buttons
/// on the hovered row, and as titled items in the list's context menu. A
/// link opens in the browser and can be copied, but has nothing to reveal
/// in Finder; a file on disk is the reverse.
struct HistoryActions: View {
    @EnvironmentObject private var state: AppState
    let entry: HistoryEntry
    let iconOnly: Bool

    private var isMissing: Bool { entry.isMissingFromDisk }

    var body: some View {
        // A file on disk can always be revealed and copied, whatever it
        // holds; the kind adds what is meaningful on top of that.
        switch entry.kind {
        case .files:
            fileActions

        case .contact:
            if let path = entry.paths.first, !isMissing {
                HistoryActionButton(symbol: "person.crop.circle.badge.plus",
                                    label: "Add to Contacts", iconOnly: iconOnly) {
                    state.open(path: path)
                }
                fileActions(includeOpen: false)
            } else if let content = entry.content {
                HistoryActionButton(symbol: "person.crop.circle.badge.plus",
                                    label: "Add to Contacts", iconOnly: iconOnly) {
                    state.openAsFile(content, extension: "vcf")
                }
                copyButton(content, label: "Copy contact")
            }

        case .calendar:
            if let path = entry.paths.first, !isMissing {
                HistoryActionButton(symbol: "calendar.badge.plus",
                                    label: "Add to Calendar", iconOnly: iconOnly) {
                    state.open(path: path)
                }
                fileActions(includeOpen: false)
            } else if let content = entry.content {
                HistoryActionButton(symbol: "calendar.badge.plus",
                                    label: "Add to Calendar", iconOnly: iconOnly) {
                    state.openAsFile(content, extension: "ics")
                }
                copyButton(content, label: "Copy event")
            }

        case .phone:
            if let content = entry.content {
                HistoryActionButton(symbol: "phone", label: "Call", iconOnly: iconOnly) {
                    state.call(content)
                }
                copyButton(content, label: "Copy number")
            }

        case .email:
            if let content = entry.content {
                HistoryActionButton(symbol: "envelope", label: "Write email", iconOnly: iconOnly) {
                    state.compose(to: content)
                }
                copyButton(content, label: "Copy address")
            }

        case .map:
            if let content = entry.content {
                HistoryActionButton(symbol: "map", label: "Open in Maps", iconOnly: iconOnly) {
                    state.showOnMap(content)
                }
                copyButton(content, label: "Copy place")
            }

        case .link:
            if let content = entry.content {
                // Anything can be copied; only web links can be opened, so
                // schemes that would launch another app are kept but offer
                // no Open action rather than one that always fails.
                if AppState.webURL(from: content) != nil {
                    // The browser's own symbol: "opens on the web" at a
                    // glance, where the generic open-in-app arrow said less.
                    HistoryActionButton(
                        symbol: "safari", label: "Open link", iconOnly: iconOnly
                    ) {
                        state.openLink(content)
                    }
                }
                copyButton(content, label: "Copy link")
            }

        case .text, .wifi:
            if let content = entry.content {
                copyButton(content, label: "Copy text")
            }
        }
    }

    /// What double-clicking the row does: open the thing, in whatever sense
    /// this entry has one. The same choice a Finder double-click makes.
    static func primary(for entry: HistoryEntry, in state: AppState) {
        switch entry.kind {
        case .files, .contact, .calendar:
            if let path = entry.paths.first, !entry.isMissingFromDisk {
                state.open(path: path)
            } else if let content = entry.content {
                let ext = entry.kind == .calendar ? "ics" : "vcf"
                if entry.kind == .files { state.copy(content) } else {
                    state.openAsFile(content, extension: ext)
                }
            }
        case .link:
            if let content = entry.content, AppState.webURL(from: content) != nil {
                state.openLink(content)
            } else if let content = entry.content {
                state.copy(content)
            }
        case .phone:
            if let content = entry.content { state.call(content) }
        case .email:
            if let content = entry.content { state.compose(to: content) }
        case .map:
            if let content = entry.content { state.showOnMap(content) }
        case .text, .wifi:
            if let content = entry.content { state.copy(content) }
        }
    }

    private var fileActions: some View { fileActions(includeOpen: true) }

    /// Nothing here for a missing file: Open and Show in Finder silently
    /// failed, and a path to nowhere is not worth copying. The row's caption
    /// says why the actions are gone.
    @ViewBuilder private func fileActions(includeOpen: Bool) -> some View {
        if entry.direction == .received, let path = entry.paths.first, !isMissing {
            if includeOpen {
                HistoryActionButton(symbol: "arrow.up.forward.app", label: "Open", iconOnly: iconOnly) {
                    state.open(path: path)
                }
            }
            HistoryActionButton(symbol: "magnifyingglass", label: "Show in Finder", iconOnly: iconOnly) {
                state.reveal(path: path)
            }
            HistoryActionButton(symbol: "doc.on.doc", label: "Copy path", iconOnly: iconOnly) {
                state.copyPath(path)
            }
        }
    }

    private func copyButton(_ content: String, label: LocalizedStringKey) -> some View {
        // doc.on.doc is the system's Copy glyph; doc.on.clipboard, used here
        // before, is Paste.
        HistoryActionButton(symbol: "doc.on.doc", label: label, iconOnly: iconOnly) {
            state.copy(content)
        }
    }
}
