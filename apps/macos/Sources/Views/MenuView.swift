import SwiftUI

/// Menu-bar popover: glanceable state and the actions that don't need a full
/// window. Anything richer lives in the main window.
struct MenuView: View {
    @EnvironmentObject private var state: AppState

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HeaderSection()

            if let transfer = state.transfer {
                CompactTransferRow(
                    title: transfer.receiving
                        ? String(localized: "Receiving from \u{201C}\(transfer.senderName)\u{201D}")
                        : String(localized: "\u{201C}\(transfer.senderName)\u{201D} is waiting, code \(transfer.token)"),
                    fraction: transfer.totalBytes > 0
                        ? Double(transfer.bytesReceived) / Double(transfer.totalBytes)
                        : nil,
                    indeterminate: !transfer.receiving)
                    .padding(.horizontal, 14)
                    .padding(.bottom, 10)
            }

            if let outbound = state.outbound {
                CompactTransferRow(
                    title: outbound.awaitingConsent
                        ? (outbound.token.isEmpty
                            ? String(localized: "Waiting for \u{201C}\(outbound.targetName)\u{201D} to accept")
                            : String(localized: "Waiting for \u{201C}\(outbound.targetName)\u{201D} to accept, code \(outbound.token)"))
                        : String(localized: "Sending to \u{201C}\(outbound.targetName)\u{201D}"),
                    fraction: outbound.totalBytes > 0
                        ? Double(outbound.bytesSent) / Double(outbound.totalBytes)
                        : nil,
                    indeterminate: outbound.awaitingConsent)
                    .padding(.horizontal, 14)
                    .padding(.bottom, 10)
            }

            Divider().padding(.horizontal, 12)

            VStack(alignment: .leading, spacing: 2) {
                MenuRow(icon: "clock.arrow.circlepath", label: "Receive for 10 minutes") {
                    state.receiveTemporarily(minutes: 10)
                }
                MenuRow(icon: "paperplane", label: "Send files…") {
                    state.chooseFilesToSend()
                }
                MenuRow(icon: "text.quote", label: "Send clipboard text") {
                    state.sendClipboardText()
                }
                MenuRow(icon: "macwindow", label: "Open DroidHarbor…") {
                    state.onOpenWindow?()
                }
            }
            .padding(.vertical, 6)

            Divider().padding(.horizontal, 12)

            VStack(alignment: .leading, spacing: 2) {
                MenuRow(
                    icon: "folder", label: "Save to",
                    value: state.destination.path, truncation: .head
                ) {
                    state.chooseDestination()
                }
                MenuRow(icon: "pencil", label: "Device name", value: state.deviceName) {
                    state.onOpenWindow?()
                }
            }
            .padding(.vertical, 6)

            Divider().padding(.horizontal, 12)

            HStack {
                Button("Quit DroidHarbor") { state.quit() }
                    .buttonStyle(.plain)
                    .font(.callout)
                Spacer()
                Text(version)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
        }
        .frame(width: 320)
        .tint(.teal)
        .background(EmptyView().allowsHitTesting(false))
    }

    private var version: String {
        let short = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String
        return short.map { "v\($0)" } ?? ""
    }
}

// MARK: - Header

private struct HeaderSection: View {
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
        }
        .padding(.horizontal, 12)
        .padding(.top, 12)
        .padding(.bottom, 10)
    }

    private var subtitle: String {
        state.receiving
            ? String(localized: "Visible as \u{201C}\(state.deviceName)\u{201D}")
            : String(localized: "Turn on to receive files")
    }
}

/// The one deliberate flourish: the tile radiates while the Mac is listening.
private struct StatusTile: View {
    let active: Bool

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(.teal.opacity(active ? 0.18 : 0.07))
                .frame(width: 34, height: 34)
            Image(systemName: "dot.radiowaves.left.and.right")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(active ? Color.teal : Color.secondary)
                .symbolEffect(.variableColor.iterative, isActive: active)
        }
        .animation(.easeInOut(duration: 0.2), value: active)
    }
}

// MARK: - Rows

private struct CompactTransferRow: View {
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

private struct MenuRow: View {
    let icon: String
    let label: LocalizedStringKey
    var value: String = ""
    var truncation: Text.TruncationMode = .middle
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(width: 18)
                Text(label)
                    .font(.callout)
                Spacer()
                if !value.isEmpty {
                    Text(value)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(truncation)
                        .frame(maxWidth: 150, alignment: .trailing)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .contentShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        }
        .buttonStyle(.plain)
        .background(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(hovered ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear))
        )
        .onHover { hovered = $0 }
        .padding(.horizontal, 6)
    }
}

