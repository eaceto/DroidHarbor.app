import SwiftUI

/// A history-row action, rendered as an icon on hover and as a titled item
/// in the context menu: the same button in both places.
struct HistoryActionButton: View {
    let symbol: String
    let label: LocalizedStringKey
    let iconOnly: Bool
    let action: () -> Void

    var body: some View {
        Group {
            if iconOnly {
                Button(action: action) { Image(systemName: symbol) }
                    .buttonStyle(.borderless)
            } else {
                Button(action: action) { Label(label, systemImage: symbol) }
            }
        }
        .help(label)
        .accessibilityLabel(Text(label))
    }
}
