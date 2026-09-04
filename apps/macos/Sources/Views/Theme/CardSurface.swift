import SwiftUI

/// Neutral elevated card, or the teal-tinted variant reserved for the one
/// urgent surface in the app: a transfer waiting for someone's consent.
struct CardSurface: ViewModifier {
    var tinted = false
    var padding: CGFloat = Theme.cardPadding

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(
                RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous)
                    .fill(tinted
                        ? AnyShapeStyle(Color.teal.opacity(0.1))
                        : AnyShapeStyle(Theme.cardFill))
            )
            // Content such as a list must not poke past the rounded corners.
            .clipShape(RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous)
                    .strokeBorder(tinted ? Color.teal.opacity(0.35) : Theme.cardStroke)
            )
    }
}

extension View {
    /// `padding: 0` is for cards that manage their own interior, such as a
    /// header-plus-list panel whose list runs to the card's edges.
    func cardSurface(tinted: Bool = false, padding: CGFloat = Theme.cardPadding) -> some View {
        modifier(CardSurface(tinted: tinted, padding: padding))
    }
}
