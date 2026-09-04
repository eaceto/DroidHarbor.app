import SwiftUI

/// The popover's identity band: the beacon tile, what the Mac currently is
/// to nearby phones, the receiving switch, and the overflow menu.
struct MenuHeader: View {
    @EnvironmentObject private var state: AppState

    var body: some View {
        HStack(spacing: 10) {
            StatusTile(active: state.receiving)

            VStack(alignment: .leading, spacing: 1) {
                Text("DroidHarbor")
                    .font(.headline)
                if let until = state.receivingUntil {
                    HStack(spacing: 3) {
                        Text("Visible for")
                        Text(timerInterval: Date()...until, countsDown: true)
                            .monospacedDigit()
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                } else {
                    Text(subtitle)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            Spacer()

            // Titled even though the title is hidden: the header beside it
            // says what the switch is for, but with an empty label VoiceOver
            // announced an unnamed switch.
            Toggle("Receiving", isOn: Binding(
                get: { state.receiving },
                set: { state.setReceiving($0) }
            ))
            .toggleStyle(.switch)
            .controlSize(.small)
            .labelsHidden()

            // Everything that used to be its own row: navigation, the
            // temporary window, quitting, the version. Settings… goes to the
            // section itself rather than merely opening the window, which is
            // where the old "Device name" row sent people to hunt.
            Menu {
                Button("Settings…") {
                    state.selectSection(MainWindow.Section.settings.rawValue)
                    state.onOpenWindow?()
                }
                Button("Receive for 10 minutes") {
                    state.receiveTemporarily(minutes: 10)
                }
                Divider()
                Button("Quit DroidHarbor") { state.quit() }
                if !version.isEmpty {
                    Divider()
                    Text(version)
                }
            } label: {
                Image(systemName: "gearshape")
                    .foregroundStyle(.secondary)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .accessibilityLabel(Text("More options"))
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 12)
    }

    /// The name is always here: while receiving it is what phones can see,
    /// and while off it is what they would see, so renames and the switch
    /// both read against the same line.
    private var subtitle: String {
        state.receiving
            ? String(localized: "Visible as \u{201C}\(state.deviceName)\u{201D}")
            : String(localized: "Off · will appear as \u{201C}\(state.deviceName)\u{201D}")
    }

    private var version: String {
        let short = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String
        return short.map { "v\($0)" } ?? ""
    }
}
