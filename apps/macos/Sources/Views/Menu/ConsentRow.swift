import SwiftUI

/// A phone waiting for consent, answerable right here: the code and the two
/// buttons are the whole decision, and the popover is often the only part of
/// the app on screen. The "always accept" refinement stays in the window.
struct ConsentRow: View {
    @EnvironmentObject private var state: AppState
    let transfer: ActiveTransfer

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("\u{201C}\(transfer.senderName)\u{201D} wants to send files")
                .font(.callout.weight(.medium))
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 8) {
                CodeTicket(code: transfer.token, compact: true)
                Spacer()
                Button("Decline") { state.decline(transfer.session) }
                    .controlSize(.small)
                Button("Accept") { state.accept(transfer.session) }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .keyboardShortcut(.defaultAction)
            }
            Text("The code must match the one on the phone.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}
