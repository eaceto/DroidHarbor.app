import SwiftUI

/// The closing page's rows. None of these can be settled well anywhere else:
/// receiving is what makes the Mac exist as far as the phone is concerned,
/// the notification prompt is a one-shot that should follow an explanation,
/// and a menu-bar app that is not running is invisible, so the login item is
/// worth raising before the first transfer rather than leaving to be
/// discovered in Settings. All three stay changeable in Settings afterwards.
struct SetupRows: View {
    @EnvironmentObject private var state: AppState

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(spacing: 8) {
                // First, because the other two are refinements and this one
                // is the difference between a working app and an app that
                // never appears on the phone.
                OnboardingRow(
                    symbol: "dot.radiowaves.left.and.right",
                    title: "Receiving",
                    text: "This Mac is only visible to nearby phones while this is on. Every transfer still needs your acceptance."
                ) {
                    Toggle("Receiving", isOn: Binding(
                        get: { state.receiving },
                        set: { state.setReceiving($0) }
                    ))
                    .labelsHidden()
                    .toggleStyle(.switch)
                }

                OnboardingRow(
                    symbol: "bell.badge",
                    title: "Notifications",
                    text: notificationDetail
                ) {
                    notificationControl
                }

                OnboardingRow(
                    symbol: "power",
                    title: "Open at login",
                    text: "Your Mac only appears in the phone's share sheet while DroidHarbor is running."
                ) {
                    Toggle("Open at login", isOn: Binding(
                        get: { state.launchAtLogin },
                        set: { state.setLaunchAtLogin($0) }
                    ))
                    .labelsHidden()
                    .toggleStyle(.switch)
                }
            }

            // Both prompts arrive from the system at the first transfer, with
            // no explanation of their own, and refusing either leaves an app
            // that finds nothing and looks broken.
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: "info.circle")
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                Text("The first time you use it, macOS asks for Local Network and Bluetooth access. DroidHarbor needs both to find your phone.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 4)
        }
        // Catches the answer given in System Settings by someone who left and
        // came back, so the row is never stale.
        .onAppear { state.refreshNotificationAuthorization() }
    }

    private var notificationDetail: LocalizedStringKey {
        if state.notificationsAllowed {
            return "Transfers are announced, and an incoming one can be accepted straight from the banner."
        }
        if state.notificationsDecided {
            return "Turned off. Transfers still work, but nothing is announced and you cannot accept from a banner."
        }
        return "Accept an incoming transfer straight from the banner, without opening the app."
    }

    @ViewBuilder
    private var notificationControl: some View {
        if state.notificationsAllowed {
            // Assembled rather than a Label: green text on the row's own
            // background sits near 2:1, and a Label cannot be given one
            // colour for the symbol and another for the word. The check mark
            // carries the green, the word carries the contrast.
            HStack(spacing: 4) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(.green)
                Text("Allowed")
            }
            .font(.callout)
        } else if state.notificationsDecided {
            // The prompt is spent. Only System Settings can change it now.
            Button("Open Settings") { state.openNotificationSettings() }
        } else {
            Button("Allow") { state.requestNotificationAuthorization() }
        }
    }
}
