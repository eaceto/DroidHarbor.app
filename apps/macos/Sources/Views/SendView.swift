import SwiftUI
import UniformTypeIdentifiers

/// Drop zone + discovered devices + outbound progress.
struct SendView: View {
    @EnvironmentObject private var state: AppState
    @State private var targeted = false
    @State private var composingText = false

    private var staged: SendPayload? { state.pendingSend }

    // A ScrollView, not a bare stack: whatever the content asks for, the
    // column stays the size of the window. A plain stack reports its ideal
    // height to the split view, and anything taller than the window pushes
    // the whole thing, sidebar included, off the top.
    var body: some View {
        ScrollView {
            content
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .sheet(isPresented: $composingText) {
            SendTextSheet { text in state.beginSend(text: text) }
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 14) {
            AppMessages()

            if state.outbound == nil, let last = state.lastSend {
                HStack(spacing: 10) {
                    Text("Last send to \u{201C}\(last.endpoint.name)\u{201D} did not finish.")
                        .font(.callout)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer()
                    Button("Try Again") { state.retryLastSend() }
                    Button("Dismiss") { state.dismissRetry() }
                        .buttonStyle(.link)
                }
                .padding(10)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(.quaternary.opacity(0.5))
                )
            }

            if let outbound = state.outbound {
                OutboundCard(outbound: outbound)
            } else {
                dropZone

                if state.pendingSend != nil {
                    deviceList
                }
            }
        }
    }

    private var dropZone: some View {
        VStack(spacing: 10) {
            Image(systemName: staged?.text?.symbolName ?? "arrow.up.doc")
                .font(.system(size: 30, weight: .light))
                .foregroundStyle(targeted ? AnyShapeStyle(Color.teal) : AnyShapeStyle(.tertiary))
            switch staged {
            case nil:
                Text("Drop files, links or text here to send")
                    .font(.headline)
                Text("or use the buttons below. You can also drop things on the menu-bar icon, or share them from Finder and other apps.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 380)
                HStack(spacing: 10) {
                    Button("Choose Files…") { state.chooseFilesToSend() }
                    Button("Send Text…") { composingText = true }
                }
            case .files(let urls):
                Text(urls.count == 1
                    ? String(localized: "1 file ready")
                    : String(localized: "\(urls.count) files ready"))
                    .font(.headline)
                Text(urls.map(\.lastPathComponent).joined(separator: ", "))
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 380)
                Button("Clear") { state.cancelSendSelection() }
            case .text(let text):
                Text(text.kind == .link
                    ? String(localized: "Link ready")
                    : String(localized: "Text ready"))
                    .font(.headline)
                Text(text.content)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(3)
                    .multilineTextAlignment(.center)
                    .textSelection(.enabled)
                    .frame(maxWidth: 380)
                Button("Clear") { state.cancelSendSelection() }
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 26)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(targeted ? Color.teal.opacity(0.08) : Color.clear)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(
                    targeted ? Color.teal : Color.secondary.opacity(0.35),
                    style: StrokeStyle(lineWidth: 1.5, dash: [6, 4]))
        )
        // URLs win when a drag carries both: dragging a link out of a browser
        // offers its address as text too, and the URL is the better reading.
        .dropDestination(for: DroppedItem.self) { items, _ in
            let urls = items.compactMap(\.url)
            if !urls.isEmpty {
                state.beginSend(files: urls)
            } else if let text = items.compactMap(\.text).first {
                state.beginSend(text: text)
            }
            return true
        } isTargeted: { targeted = $0 }
    }

    private var deviceList: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Nearby devices")
                .font(.headline)

            if state.endpoints.isEmpty {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Open Quick Share on the phone (Settings → Connected devices, or the Files app) so it appears here.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.vertical, 6)
            } else {
                ForEach(state.endpoints) { endpoint in
                    DeviceRow(endpoint: endpoint) { state.send(to: endpoint) }
                }
                if state.endpoints.contains(where: { !$0.present }) {
                    HStack(spacing: 6) {
                        Text("Dimmed devices were seen before but are not visible right now.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                        Button("Forget") { state.forgetDevices() }
                            .controlSize(.small)
                            .buttonStyle(.link)
                    }
                    .padding(.top, 2)
                }
            }
        }
    }
}

private struct DeviceRow: View {
    let endpoint: Endpoint
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: endpoint.symbolName)
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .frame(width: 22)
                    .accessibilityHidden(true)
                Text(endpoint.name)
                    .lineLimit(1)
                Spacer()
                Text(endpoint.present ? String(localized: "Send") : String(localized: "Not nearby"))
                    .font(.callout.weight(.medium))
                    .foregroundStyle(hovered && endpoint.present ? Color.teal : .secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
        .buttonStyle(.plain)
        .disabled(!endpoint.present)
        .opacity(endpoint.present ? 1 : 0.45)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(hovered && endpoint.present ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear))
        )
        .onHover { hovered = $0 }
        .accessibilityLabel(Text("Send to \(endpoint.name)"))
    }
}

private struct OutboundCard: View {
    @EnvironmentObject private var state: AppState
    let outbound: OutboundTransfer

    private var fileSummary: String {
        if let text = outbound.text {
            return text.kind == .link
                ? String(localized: "Link")
                : String(localized: "Text · \(Format.bytes(outbound.totalBytes))")
        }
        let count = outbound.itemNames.count
        guard outbound.totalBytes > 0 else {
            return count == 1
                ? String(localized: "1 file")
                : String(localized: "\(count) files")
        }
        let size = Format.bytes(outbound.totalBytes)
        return count == 1
            ? String(localized: "1 file · \(size)")
            : String(localized: "\(count) files · \(size)")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(outbound.awaitingConsent
                        ? "Waiting for \u{201C}\(outbound.targetName)\u{201D} to accept…"
                        : "Sending to \u{201C}\(outbound.targetName)\u{201D}")
                        .font(.headline)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(fileSummary)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Cancel", role: .destructive) { state.cancel(outbound.session) }
            }

            if outbound.awaitingConsent {
                VStack(alignment: .leading, spacing: 10) {
                    // The phone shows this and asks its user to check it
                    // against the sending device. Without it here there was
                    // nothing on this screen to check it against.
                    if !outbound.token.isEmpty {
                        HStack(spacing: 8) {
                            Text("Code")
                                .font(.callout)
                                .foregroundStyle(.secondary)
                            Text(outbound.token)
                                .font(.system(.title, design: .rounded).weight(.semibold).monospacedDigit())
                                .accessibilityLabel(Text("Confirmation code \(outbound.token)"))
                            Text("must match the code shown on the phone")
                                .font(.callout)
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }

                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Accept the transfer on the phone to start.")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                }
            } else if outbound.totalBytes > 0 {
                ProgressView(
                    value: Double(outbound.bytesSent),
                    total: Double(outbound.totalBytes))
                HStack {
                    Text("\(Format.bytes(outbound.bytesSent)) of \(Format.bytes(outbound.totalBytes))")
                    Spacer()
                    if outbound.rate.bytesPerSecond > 0 {
                        Text(Format.rate(outbound.rate.bytesPerSecond))
                        if let left = outbound.rate.secondsRemaining {
                            Text("·")
                            Text(Format.remaining(left))
                        }
                    }
                }
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
            } else {
                ProgressView().controlSize(.small)
            }

            VStack(alignment: .leading, spacing: 3) {
                ForEach(outbound.itemNames, id: \.self) { name in
                    HStack(spacing: 8) {
                        Image(systemName: outbound.symbolName)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .frame(width: 14)
                        Text(name)
                            .font(.callout)
                            .lineLimit(1)
                            .truncationMode(.middle)
                        Spacer()
                    }
                }
            }
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(.quaternary.opacity(0.5))
        )
    }
}
