import SwiftUI

/// One past transfer, with the actions its kind supports.
struct HistoryRow: View {
    @EnvironmentObject private var state: AppState
    let entry: HistoryEntry
    @State private var hovered = false

    /// The prune pass removes these shortly, but a file can vanish while
    /// the row is on screen; until then it explains itself and offers no
    /// actions that would fail.
    private var isMissing: Bool { entry.isMissingFromDisk }

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: entry.symbolName)
                .foregroundStyle(entry.direction == .received ? Color.teal : Color.secondary)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 1) {
                Text(entry.summary)
                    .lineLimit(1)
                    .truncationMode(entry.isFile ? .middle : .tail)
                Text(entry.direction == .received
                    ? String(localized: "from \(entry.peer) · \(entry.date.formatted(date: .abbreviated, time: .shortened))")
                    : String(localized: "to \(entry.peer) · \(entry.date.formatted(date: .abbreviated, time: .shortened))"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                // The dimming alone read as a rendering glitch; say why.
                if isMissing {
                    Text("File moved or deleted")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
            }
            .opacity(isMissing ? 0.5 : 1)

            Spacer()

            if hovered {
                HStack(spacing: 2) { actions(iconOnly: true) }
            }
        }
        .padding(.vertical, 3)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        // Titles in the menu, icons on the row: the same actions either way.
        .contextMenu { actions(iconOnly: false) }
    }

    /// What a row offers depends on what it is: a link opens in the browser
    /// and can be copied, but has nothing to reveal in Finder.
    @ViewBuilder private func actions(iconOnly: Bool) -> some View {
        // A file on disk can always be revealed and copied, whatever it
        // holds; the kind adds what is meaningful on top of that.
        switch entry.kind {
        case .files:
            fileActions(iconOnly: iconOnly)

        case .contact:
            if let path = entry.paths.first, !isMissing {
                HistoryActionButton(symbol: "person.crop.circle.badge.plus",
                                    label: "Add to Contacts", iconOnly: iconOnly) {
                    state.open(path: path)
                }
                fileActions(iconOnly: iconOnly, includeOpen: false)
            } else if let content = entry.content {
                HistoryActionButton(symbol: "person.crop.circle.badge.plus",
                                    label: "Add to Contacts", iconOnly: iconOnly) {
                    state.openAsFile(content, extension: "vcf")
                }
                copyButton(content, label: "Copy contact", iconOnly: iconOnly)
            }

        case .calendar:
            if let path = entry.paths.first, !isMissing {
                HistoryActionButton(symbol: "calendar.badge.plus",
                                    label: "Add to Calendar", iconOnly: iconOnly) {
                    state.open(path: path)
                }
                fileActions(iconOnly: iconOnly, includeOpen: false)
            } else if let content = entry.content {
                HistoryActionButton(symbol: "calendar.badge.plus",
                                    label: "Add to Calendar", iconOnly: iconOnly) {
                    state.openAsFile(content, extension: "ics")
                }
                copyButton(content, label: "Copy event", iconOnly: iconOnly)
            }

        case .phone:
            if let content = entry.content {
                HistoryActionButton(symbol: "phone", label: "Call", iconOnly: iconOnly) {
                    state.call(content)
                }
                copyButton(content, label: "Copy number", iconOnly: iconOnly)
            }

        case .email:
            if let content = entry.content {
                HistoryActionButton(symbol: "envelope", label: "Write email", iconOnly: iconOnly) {
                    state.compose(to: content)
                }
                copyButton(content, label: "Copy address", iconOnly: iconOnly)
            }

        case .map:
            if let content = entry.content {
                HistoryActionButton(symbol: "map", label: "Open in Maps", iconOnly: iconOnly) {
                    state.showOnMap(content)
                }
                copyButton(content, label: "Copy place", iconOnly: iconOnly)
            }

        case .link:
            if let content = entry.content {
                // Anything can be copied; only web links can be opened, so
                // schemes that would launch another app are kept but offer
                // no Open action rather than one that always fails.
                if AppState.webURL(from: content) != nil {
                    HistoryActionButton(
                        symbol: "arrow.up.forward.app", label: "Open link", iconOnly: iconOnly
                    ) {
                        state.openLink(content)
                    }
                }
                copyButton(content, label: "Copy link", iconOnly: iconOnly)
            }

        case .text, .wifi:
            if let content = entry.content {
                copyButton(content, label: "Copy text", iconOnly: iconOnly)
            }
        }
    }

    /// Nothing here for a missing file: Open and Show in Finder silently
    /// failed, and a path to nowhere is not worth copying. The row's caption
    /// says why the actions are gone.
    @ViewBuilder private func fileActions(iconOnly: Bool, includeOpen: Bool = true) -> some View {
        if entry.direction == .received, let path = entry.paths.first, !isMissing {
            if includeOpen {
                HistoryActionButton(symbol: "arrow.up.forward.app", label: "Open", iconOnly: iconOnly) {
                    state.open(path: path)
                }
            }
            HistoryActionButton(symbol: "magnifyingglass", label: "Show in Finder", iconOnly: iconOnly) {
                state.reveal(path: path)
            }
            HistoryActionButton(symbol: "doc.on.clipboard", label: "Copy path", iconOnly: iconOnly) {
                state.copyPath(path)
            }
        }
    }

    private func copyButton(
        _ content: String, label: LocalizedStringKey, iconOnly: Bool
    ) -> some View {
        HistoryActionButton(symbol: "doc.on.clipboard", label: label, iconOnly: iconOnly) {
            state.copy(content)
        }
    }
}
