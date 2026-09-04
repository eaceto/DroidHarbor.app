import SwiftUI

/// Shared empty-state block, with room for the action it suggests.
struct EmptyStateView<Action: View>: View {
    let symbol: String
    let title: String
    let message: String
    @ViewBuilder var action: () -> Action

    var body: some View {
        VStack(spacing: 12) {
            // Hierarchical teal rather than gray: an empty pane still says
            // which app it belongs to, without shouting.
            Image(systemName: symbol)
                .font(.system(size: 36, weight: .light))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(Color.teal)
                .accessibilityHidden(true)
            Text(title)
                .font(.title3.weight(.semibold))
            Text(message)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                // No fixedSize here. Inside a view that fills its container,
                // asking the text for its ideal height makes it size against
                // an unconstrained width proposal and report a hugely tall
                // result, which propagated up and pushed the split view (and
                // the sidebar with it) far past the window. A bounded width is
                // enough for the text to wrap.
                .frame(maxWidth: 340)
            action()
                .padding(.top, 2)
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

extension EmptyStateView where Action == EmptyView {
    init(symbol: String, title: String, message: String) {
        self.init(symbol: symbol, title: title, message: message) { EmptyView() }
    }
}
