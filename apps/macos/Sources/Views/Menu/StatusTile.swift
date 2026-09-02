import SwiftUI

/// The one deliberate flourish: the tile radiates while the Mac is listening.
struct StatusTile: View {
    let active: Bool

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(.teal.opacity(active ? 0.18 : 0.07))
                .frame(width: 34, height: 34)
            Image(systemName: "dot.radiowaves.left.and.right")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(active ? Color.teal : Color.secondary)
                .symbolEffect(.variableColor.iterative, isActive: active)
        }
        .animation(.easeInOut(duration: 0.2), value: active)
    }
}
