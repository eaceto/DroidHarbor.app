import SwiftUI

/// Inline message strip used by the window sections and the popover.
struct MessageStrip: View {
    /// A failure and a piece of news look nothing alike to a reader, and
    /// dressing "DroidHarbor is up to date." as an orange warning was
    /// telling the user something had gone wrong.
    enum Kind {
        case error
        case notice

        var symbol: String {
            switch self {
            case .error: return "exclamationmark.triangle.fill"
            case .notice: return "info.circle.fill"
            }
        }

        var color: Color {
            switch self {
            case .error: return .orange
            case .notice: return .teal
            }
        }
    }

    var kind: Kind = .error
    let message: String
    let onDismiss: () -> Void

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: kind.symbol)
                .foregroundStyle(kind.color)
            Text(message)
                .fixedSize(horizontal: false, vertical: true)
            Spacer()
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text("Dismiss"))
        }
        .font(.callout)
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: Theme.stripRadius, style: .continuous)
                .fill(kind.color.opacity(0.12))
        )
    }
}
