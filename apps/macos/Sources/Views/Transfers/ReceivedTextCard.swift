import SwiftUI

/// Text and links never touch the disk, so they are shown here until
/// dismissed: the clipboard already has the content.
struct ReceivedTextCard: View {
    let text: ReceivedText
    let onDismiss: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: text.kind == "link" ? "link" : "doc.on.clipboard")
                .font(.title3)
                .foregroundStyle(Color.teal)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 3) {
                Text(text.kind == "link"
                    ? String(localized: "Link copied to the clipboard")
                    : String(localized: "Text copied to the clipboard"))
                    .font(.body.weight(.medium))
                Text(text.content)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(3)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                // Same restriction as the history row: only web links are
                // offered, since the content came from another device.
                if text.kind == "link", let url = AppState.webURL(from: text.content) {
                    Link("Open link", destination: url)
                        .font(.callout)
                }
            }
            Spacer()
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text("Dismiss"))
        }
        .cardSurface()
    }
}
