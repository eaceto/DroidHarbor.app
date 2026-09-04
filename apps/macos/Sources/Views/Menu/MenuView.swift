import SwiftUI

/// Menu-bar popover, laid out as four bands: an identity header with the
/// receiving switch and an overflow menu, a roomy activity area that carries
/// whatever is happening right now (or says that nothing is), the save
/// destination, and a bar of the three everyday actions. Anything richer
/// lives in the main window.
struct MenuView: View {
    @EnvironmentObject private var state: AppState
    @State private var dropTargeted = false

    private var hasActivity: Bool {
        state.transfer != nil || state.outbound != nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            MenuHeader()
                .background(Rectangle().fill(.quinary))

            Divider()

            activity

            Divider()

            // The destination matters exactly when files are about to land,
            // which is what this popover is open for; one row, clickable.
            MenuRow(
                icon: "folder", label: "Save to",
                value: (state.destination.path as NSString).abbreviatingWithTildeInPath,
                truncation: .head
            ) {
                state.chooseDestination()
            }
            .padding(.vertical, 6)

            Divider()

            HStack(spacing: 4) {
                FooterAction(symbol: "paperplane", label: "Send files") {
                    state.chooseFilesToSend()
                }
                FooterAction(symbol: "doc.on.clipboard", label: "Send clipboard") {
                    state.sendClipboardText()
                }
                FooterAction(symbol: "macwindow", label: "Open app") {
                    state.onOpenWindow?()
                }
            }
            .padding(8)
            .background(Rectangle().fill(.quinary))
        }
        .frame(width: 340)
        .tint(.teal)
        .overlay { if dropTargeted { dropOverlay } }
        // The whole popover accepts what the icon accepts, so a drag aimed
        // at the icon that lands on the open popover still works. Dropping
        // stages the payload; staging opens the window on Send, and the
        // transient popover steps aside on its own.
        .dropDestination(for: DroppedItem.self) { items, _ in
            // URLs win when a drag carries both: dragging a link out of a
            // browser offers its address as text too, and the URL is the
            // better reading.
            let urls = items.compactMap(\.url)
            if !urls.isEmpty {
                state.beginSend(files: urls)
            } else if let text = items.compactMap(\.text).first {
                state.beginSend(text: text)
            } else {
                return false
            }
            return true
        } isTargeted: { dropTargeted = $0 }
    }

    /// Consent first, then whatever is moving; an empty area explains itself
    /// and offers the switch state's next step instead of dead space.
    private var activity: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Errors must be visible here, not only in the window: "Send
            // clipboard" with an empty clipboard otherwise failed in
            // silence, since the popover is often the only thing open.
            if let error = state.lastError {
                MessageStrip(kind: .error, message: error) { state.dismissError() }
            }

            if let transfer = state.transfer {
                if transfer.receiving {
                    CompactTransferRow(
                        title: String(localized: "Receiving from \u{201C}\(transfer.senderName)\u{201D}"),
                        fraction: transfer.totalBytes > 0
                            ? Double(transfer.bytesReceived) / Double(transfer.totalBytes)
                            : nil,
                        indeterminate: false)
                } else {
                    ConsentRow(transfer: transfer)
                }
            }

            if let outbound = state.outbound {
                CompactTransferRow(
                    title: outbound.awaitingConsent
                        ? (outbound.token.isEmpty
                            ? String(localized: "Waiting for \u{201C}\(outbound.targetName)\u{201D} to accept")
                            : String(localized: "Waiting for \u{201C}\(outbound.targetName)\u{201D} to accept, code \(outbound.token)"))
                        : String(localized: "Sending to \u{201C}\(outbound.targetName)\u{201D}"),
                    fraction: outbound.totalBytes > 0
                        ? Double(outbound.bytesSent) / Double(outbound.totalBytes)
                        : nil,
                    indeterminate: outbound.awaitingConsent)
            }

            if !hasActivity {
                ActivityEmptyState()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(16)
        .frame(minHeight: 200, alignment: .top)
    }

    /// Full-popover feedback while a drag hovers: the same dashed teal
    /// language as the Send pane's drop zone.
    private var dropOverlay: some View {
        ZStack {
            Rectangle().fill(.regularMaterial)
            RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous)
                .strokeBorder(Color.teal, style: StrokeStyle(lineWidth: 1.5, dash: [6, 4]))
                .padding(8)
            VStack(spacing: 8) {
                Image(systemName: "arrow.up.doc")
                    .font(.system(size: 30, weight: .light))
                    .foregroundStyle(Color.teal)
                Text("Release to send")
                    .font(.headline)
            }
        }
        .allowsHitTesting(false)
    }
}
