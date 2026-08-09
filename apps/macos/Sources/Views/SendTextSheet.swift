import AppKit
import SwiftUI

/// Compose a piece of text (a note, a link, an address) to send to a phone.
///
/// Opens holding whatever text is on the clipboard, since "copy on the Mac,
/// paste on the phone" is the errand this exists for.
struct SendTextSheet: View {
    let onSend: (String) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var text: String = NSPasteboard.general.string(forType: .string) ?? ""
    @FocusState private var editing: Bool

    private var trimmed: String {
        text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// What the phone will make of it, shown so the choice is never a
    /// surprise: a link opens a browser, a number offers to dial.
    private var classified: OutboundText? {
        trimmed.isEmpty ? nil : OutboundText(content: trimmed)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Send Text")
                .font(.headline)

            TextEditor(text: $text)
                .font(.body)
                .focused($editing)
                .frame(minWidth: 380, minHeight: 120)
                .padding(6)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(.quaternary.opacity(0.4))
                )
                .accessibilityLabel(Text("Text to send"))

            HStack(spacing: 8) {
                if let classified {
                    Image(systemName: classified.symbolName)
                        .foregroundStyle(.secondary)
                        .accessibilityHidden(true)
                    Text(hint(for: classified.kind))
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Cancel", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button("Send") {
                    onSend(trimmed)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(trimmed.isEmpty)
            }
        }
        .padding(16)
        .onAppear { editing = true }
    }

    private func hint(for kind: HistoryEntry.Kind) -> String {
        switch kind {
        case .link: return String(localized: "The phone can open this in a browser.")
        case .phone: return String(localized: "The phone can dial this number.")
        case .map: return String(localized: "The phone can open this in Maps.")
        case .email: return String(localized: "Sent as text. The phone can copy it.")
        default: return String(localized: "Sent as plain text.")
        }
    }
}
