import SwiftUI

/// A history-row action, rendered as an icon on hover and as a titled item
/// in the context menu: the same button in both places.
///
/// The icon form uses `.accessoryBar` — the system style for icon actions
/// living inside rows (Mail, Notes): monochrome glyphs with the standard
/// rounded hover backdrop, instead of accent-tinted borderless buttons that
/// read as foreign. `.help` sits directly on the button; hung off a wrapper
/// it never armed and the icons gave no hint of what they do.
struct HistoryActionButton: View {
    let symbol: String
    let label: LocalizedStringKey
    let iconOnly: Bool
    let action: () -> Void

    var body: some View {
        if iconOnly {
            Button(action: action) { Image(systemName: symbol) }
                .buttonStyle(.accessoryBar)
                .help(label)
                .accessibilityLabel(Text(label))
        } else {
            Button(action: action) { Label(label, systemImage: symbol) }
        }
    }
}
