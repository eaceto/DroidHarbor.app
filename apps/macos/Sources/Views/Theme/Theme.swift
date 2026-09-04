import SwiftUI

/// The app's visual vocabulary: one spacing grid, one card treatment, and
/// teal reserved for state and action. The panels share these values instead
/// of each inventing its own, which is what made the old surfaces read as
/// unrelated grays.
enum Theme {
    // A 4pt grid. Page and card values are the only two paddings a pane
    // should need; anything finer belongs inside a component.
    /// Outer padding of every detail pane.
    static let pagePadding: CGFloat = 20
    /// Vertical space between the cards of a pane.
    static let cardGap: CGFloat = 16
    /// Default padding inside a card.
    static let cardPadding: CGFloat = 16
    static let cardRadius: CGFloat = 12
    /// Message strips and hover highlights, which sit inside cards or
    /// between them and should read as lighter than either.
    static let stripRadius: CGFloat = 8

    /// Card surface: white over the window gray in light mode, a lifted
    /// gray in dark — the same relationship the grouped Settings form has
    /// with its window, so the panes and Settings read as one app.
    static let cardFill = Color(nsColor: NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            ? NSColor.white.withAlphaComponent(0.07)
            : NSColor.white
    })
    static let cardStroke = Color(nsColor: .separatorColor)
}
