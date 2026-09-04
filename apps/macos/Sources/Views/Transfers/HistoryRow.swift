import SwiftUI

/// One past transfer: the row itself. Its actions live in `HistoryActions`,
/// shared with the list's context menu, and the list owns selection,
/// right-click and double-click.
struct HistoryRow: View {
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

            // Faded rather than inserted: a button that pops into existence
            // under an already-hovering pointer never arms its tooltip, and
            // the row's text used to shift to make room.
            HStack(spacing: 2) { HistoryActions(entry: entry, iconOnly: true) }
                .opacity(hovered ? 1 : 0)
                .allowsHitTesting(hovered)
        }
        .padding(.vertical, 3)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
    }
}
