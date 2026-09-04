import SwiftUI

/// The 4-digit confirmation code, set as the ticket it is: matching it
/// against the phone is the app's whole security ritual, so it is the one
/// typographic flourish the panels keep.
struct CodeTicket: View {
    let code: String
    /// Smaller for the menu-bar popover, which has no room for display type.
    var compact = false

    var body: some View {
        Text(code)
            .font(.system(compact ? .title3 : .title2, design: .rounded)
                .weight(.semibold).monospacedDigit())
            .kerning(1.5)
            .padding(.horizontal, compact ? 8 : 10)
            .padding(.vertical, compact ? 2 : 4)
            .background(
                RoundedRectangle(cornerRadius: Theme.stripRadius, style: .continuous)
                    .fill(Color.teal.opacity(0.12))
            )
            .accessibilityLabel(Text("Confirmation code \(code)"))
    }
}
