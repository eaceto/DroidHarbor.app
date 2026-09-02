import SwiftUI

/// One of the popover's three everyday actions: an icon over its label,
/// equal widths, the whole cell clickable.
struct FooterAction: View {
    let symbol: String
    let label: LocalizedStringKey
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            VStack(spacing: 5) {
                Image(systemName: symbol)
                    .font(.system(size: 16, weight: .medium))
                Text(label)
                    .font(.caption)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
            .contentShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        }
        .buttonStyle(.plain)
        .background(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(hovered ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear))
        )
        .onHover { hovered = $0 }
    }
}
