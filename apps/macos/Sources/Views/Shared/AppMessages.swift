import SwiftUI

/// Whatever the app currently has to say, in the order it should be read.
/// Every section shows this in the same place rather than picking its own
/// subset, which is how the introduction ended up with no way to report a
/// failure on three of its four pages.
struct AppMessages: View {
    @EnvironmentObject private var state: AppState

    var body: some View {
        // An explicit EmptyView when there is nothing to say. A stack skips
        // EmptyView entirely, where a group that merely renders nothing still
        // takes its share of the surrounding spacing and left a gap above
        // the first card.
        if state.lastNotice == nil, state.lastError == nil {
            EmptyView()
        } else {
            VStack(spacing: 8) {
                if let notice = state.lastNotice {
                    MessageStrip(kind: .notice, message: notice) { state.dismissNotice() }
                }
                if let error = state.lastError {
                    MessageStrip(kind: .error, message: error) { state.dismissError() }
                }
            }
        }
    }
}
