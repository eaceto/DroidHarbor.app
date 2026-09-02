import SwiftUI

/// One discovered device: click to send whatever is staged.
struct DeviceRow: View {
    let endpoint: Endpoint
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: endpoint.symbolName)
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .frame(width: 22)
                    .accessibilityHidden(true)
                Text(endpoint.name)
                    .font(.body.weight(.medium))
                    .lineLimit(1)
                Spacer()
                Text(endpoint.present ? String(localized: "Send") : String(localized: "Not nearby"))
                    .font(.callout.weight(.medium))
                    .foregroundStyle(hovered && endpoint.present ? Color.teal : .secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
        .buttonStyle(.plain)
        .disabled(!endpoint.present)
        .opacity(endpoint.present ? 1 : 0.45)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(hovered && endpoint.present ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear))
        )
        .onHover { hovered = $0 }
        .accessibilityLabel(Text("Send to \(endpoint.name)"))
    }
}
