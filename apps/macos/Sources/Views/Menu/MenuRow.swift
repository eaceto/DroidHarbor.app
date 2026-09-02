import SwiftUI

/// A labelled popover row with an optional trailing value, highlighted on
/// hover the way a menu item would be.
struct MenuRow: View {
    let icon: String
    let label: LocalizedStringKey
    var value: String = ""
    var truncation: Text.TruncationMode = .middle
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(width: 18)
                Text(label)
                    .font(.callout)
                Spacer()
                if !value.isEmpty {
                    Text(value)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(truncation)
                        .frame(maxWidth: 190, alignment: .trailing)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .contentShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        }
        .buttonStyle(.plain)
        .background(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(hovered ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear))
        )
        .onHover { hovered = $0 }
        .padding(.horizontal, 6)
    }
}
