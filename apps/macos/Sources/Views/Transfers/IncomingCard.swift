import SwiftUI

/// An incoming transfer: the consent question while it is one, then live
/// progress once accepted.
struct IncomingCard: View {
    @EnvironmentObject private var state: AppState
    let transfer: ActiveTransfer
    @State private var alwaysAccept = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    // No fixedSize here: inside the split view it reports
                    // an ideal height for the whole column that is taller
                    // than the window, and everything gets pushed off the
                    // top, sidebar included.
                    Text(title)
                        .font(.headline)
                        .lineLimit(2)
                    Text(fileSummary)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if transfer.receiving {
                    Button("Cancel", role: .destructive) { state.cancel(transfer.session) }
                } else {
                    HStack(spacing: 8) {
                        Button("Decline") { state.decline(transfer.session) }
                        Button("Accept") {
                            if alwaysAccept { state.trust(transfer.senderName) }
                            state.accept(transfer.session)
                        }
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                    }
                }
            }

            if !transfer.receiving {
                HStack(spacing: 10) {
                    CodeTicket(code: transfer.token)
                    Text("must match the code shown on the phone")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Toggle(isOn: $alwaysAccept) {
                    Text("Always accept from \u{201C}\(transfer.senderName)\u{201D}")
                        .font(.callout)
                }
                .toggleStyle(.checkbox)
            } else if transfer.totalBytes > 0 {
                ProgressView(
                    value: Double(transfer.bytesReceived),
                    total: Double(transfer.totalBytes))
                    .accessibilityLabel(Text("Transfer progress"))
                HStack {
                    Text("\(Format.bytes(transfer.bytesReceived)) of \(Format.bytes(transfer.totalBytes))")
                    Spacer()
                    if transfer.rate.bytesPerSecond > 0 {
                        Text(Format.rate(transfer.rate.bytesPerSecond))
                        if let left = transfer.rate.secondsRemaining {
                            Text("·")
                            Text(Format.remaining(left))
                        }
                    }
                }
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
            }

            // Per-file rows: only the window has room for these, and only
            // so many of them. A long batch scrolls instead of growing the
            // card past the height of the window.
            if transfer.files.count > 4 {
                ScrollView { fileRows }
                    .frame(maxHeight: 132)
            } else {
                fileRows
            }
        }
        // Tinted only while the decision is pending: the app's one urgent
        // surface. Once accepted it becomes an ordinary working card.
        .cardSurface(tinted: !transfer.receiving)
    }

    private var fileRows: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(transfer.files) { file in
                FileProgressRow(
                    file: file,
                    current: transfer.currentFile == file.name,
                    showsProgress: transfer.files.count > 1)
            }
        }
    }

    private var fileSummary: String {
        if let preview = transfer.textPreview {
            return preview.isEmpty ? String(localized: "A link or text") : preview
        }
        let size = Format.bytes(transfer.totalBytes)
        return transfer.files.count == 1
            ? String(localized: "1 file · \(size)")
            : String(localized: "\(transfer.files.count) files · \(size)")
    }

    private var title: String {
        transfer.receiving
            ? String(localized: "Receiving from \u{201C}\(transfer.senderName)\u{201D}")
            : String(localized: "\u{201C}\(transfer.senderName)\u{201D} wants to send files")
    }
}
