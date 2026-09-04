import SwiftUI

// Everything configurable, plus the honest notes about visibility and the
// unofficial protocol that belong in front of the user rather than a README.

struct SettingsView: View {
    @EnvironmentObject private var state: AppState
    @State private var draftName = ""

    var body: some View {
        Form {
            Section("Receiving") {
                Toggle("Receive files", isOn: Binding(
                    get: { state.receiving },
                    set: { state.setReceiving($0) }
                ))
                if state.receiving {
                    LabeledContent("Visible as") {
                        Text(state.deviceName)
                            .foregroundStyle(.secondary)
                    }
                }
                LabeledContent("Save to") {
                    HStack(spacing: 8) {
                        Text(state.destination.path)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.head)
                        Button("Change…") { state.chooseDestination() }
                    }
                }
            }

            Section("This Mac") {
                LabeledContent("Device name") {
                    HStack(spacing: 8) {
                        TextField("Device name", text: $draftName)
                            .textFieldStyle(.roundedBorder)
                            .frame(maxWidth: 220)
                            .onSubmit { commitRename() }
                        Button("Save") { commitRename() }
                            .disabled(draftName.trimmingCharacters(in: .whitespaces).isEmpty
                                || draftName == state.deviceName)
                    }
                }
                Text("Nearby Android devices see this name in their share sheet. Changing it restarts the receiver.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Section("General") {
                Toggle("Open at login", isOn: Binding(
                    get: { state.launchAtLogin },
                    set: { state.setLaunchAtLogin($0) }
                ))
                if !state.notificationsAllowed {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Image(systemName: "bell.slash")
                            .foregroundStyle(.orange)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Notifications are turned off")
                                .font(.callout.weight(.medium))
                            Text("Transfers will still work, but nothing will be announced and incoming files cannot be accepted from a banner.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        Spacer()
                        Button("Open Settings") { state.openNotificationSettings() }
                            .controlSize(.small)
                    }
                }
                Toggle("Play sounds", isOn: Binding(
                    get: { state.playSounds },
                    set: { state.setPlaySounds($0) }
                ))
                Picker("Turn receiving off when idle", selection: Binding(
                    get: { state.autoOffMinutes },
                    set: { state.setAutoOffMinutes($0) }
                )) {
                    Text("Never").tag(0)
                    Text("After 10 minutes").tag(10)
                    Text("After 30 minutes").tag(30)
                    Text("After 1 hour").tag(60)
                }
                Button("Show the introduction again") { state.showOnboardingAgain() }
                Button("Check for Updates…") { state.checkForUpdates(userInitiated: true) }
            }

            Section("Trusted devices") {
                if state.trustedDevices.isEmpty {
                    Text("None yet. When you accept a transfer you can tick \u{201C}Always accept\u{201D} to skip the prompt next time.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    ForEach(state.trustedDevices, id: \.self) { name in
                        HStack {
                            Text(name)
                            Spacer()
                            Button("Revoke") { state.revokeTrust(name) }
                                .controlSize(.small)
                        }
                    }
                    Text("Trusted transfers are accepted without the confirmation code. Devices are matched by the name they announce, which a device chooses for itself.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            Section("Privacy") {
                Text("While receiving is on, this Mac is visible to any nearby Android device with Quick Share open. Every transfer still needs your explicit acceptance, and the code shown must match the phone.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                Text("DroidHarbor implements an unofficial, reverse-engineered protocol in order to interoperate with the sharing built into Android, and may stop working after an Android update. Files never leave your local network.")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .formStyle(.grouped)
        .onAppear { draftName = state.deviceName }
        .onChange(of: state.deviceName) { _, name in draftName = name }
    }

    private func commitRename() {
        state.rename(to: draftName)
    }
}
