import SwiftUI

/// What the popover's empty activity area says depends on the switch:
/// ready-and-waiting gets the phone-side steps, off gets the way back on.
/// The dashed ring borrows the drop zone's language: a place where
/// something will arrive.
struct ActivityEmptyState: View {
    @EnvironmentObject private var state: AppState

    var body: some View {
        VStack(spacing: 10) {
            ZStack {
                Circle()
                    .strokeBorder(
                        state.receiving ? Color.teal.opacity(0.5) : Theme.cardStroke,
                        style: StrokeStyle(lineWidth: 1.5, dash: [4, 4]))
                Image(systemName: "tray.and.arrow.down")
                    .font(.system(size: 22, weight: .light))
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(state.receiving ? AnyShapeStyle(Color.teal) : AnyShapeStyle(.tertiary))
            }
            .frame(width: 64, height: 64)
            .accessibilityHidden(true)

            Text(state.receiving
                ? String(localized: "Ready to receive")
                : String(localized: "Receiving is off"))
                .font(.headline)

            Text(state.receiving
                ? String(localized: "On the phone: pick files, then Share → Quick Share → \u{201C}\(state.deviceName)\u{201D}.")
                : String(localized: "Turn receiving on to accept files from nearby Android devices."))
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)

            if !state.receiving {
                Button("Turn On Receiving") { state.setReceiving(true) }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .padding(.top, 2)
            }
        }
        .padding(.vertical, 8)
    }
}
