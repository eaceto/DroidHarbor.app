import SwiftUI

/// One line of transfer state for the popover: what is happening, and a bar
/// when there is progress to show.
struct CompactTransferRow: View {
    let title: String
    let fraction: Double?
    let indeterminate: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.callout)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
            if indeterminate {
                ProgressView().controlSize(.small)
            } else if let fraction {
                ProgressView(value: fraction)
            }
        }
    }
}
