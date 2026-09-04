import SwiftUI

/// One row: a symbol, what it says, and optionally the control for it. The
/// explanation steps and the setup settings are the same object drawn twice,
/// which is what makes the four pages read as one design. It also retired
/// the numbered teal circles, whose white digits sat at about 2.2:1 against
/// their own background.
struct OnboardingRow<Control: View>: View {
    let symbol: String
    var title: LocalizedStringKey?
    let text: LocalizedStringKey
    @ViewBuilder var control: () -> Control

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 14, weight: .semibold))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(Color.teal)
                .frame(width: 22, height: 22)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                if let title {
                    Text(title)
                        .font(.callout.weight(.medium))
                }
                Text(text)
                    .font(.callout)
                    .foregroundStyle(title == nil ? .primary : .secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            control()
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(.quaternary)
        )
    }
}

extension OnboardingRow where Control == EmptyView {
    init(symbol: String, text: LocalizedStringKey) {
        self.init(symbol: symbol, title: nil, text: text) { EmptyView() }
    }
}
