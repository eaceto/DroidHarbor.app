import SwiftUI

/// One instruction on an introduction page: a symbol and what to do.
struct OnboardingStep: Identifiable {
    let symbol: String
    let text: LocalizedStringKey
    var id: String { symbol + "\(text)" }
}
