import SwiftUI

/// One page of the introduction: its symbol, heading, lead and steps.
struct OnboardingPage {
    let symbol: String
    let title: LocalizedStringKey
    let lead: LocalizedStringKey
    let steps: [OnboardingStep]
}
