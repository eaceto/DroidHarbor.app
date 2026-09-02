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
                .padding(Theme.pagePadding)
                .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .sheet(isPresented: $composingText) {
            SendTextSheet { text in state.beginSend(text: text) }
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: Theme.cardGap) {
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
                .cardSurface(padding: 12)
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
        VStack(spacing: 12) {
            // Teal the moment the zone has something: a drag hovering over
            // it, or a payload already staged and waiting for a device.
            Image(systemName: staged?.text?.symbolName ?? "arrow.up.doc")
                .font(.system(size: 34, weight: .light))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(targeted || staged != nil
                    ? AnyShapeStyle(Color.teal)
                    : AnyShapeStyle(.tertiary))
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
                        .buttonStyle(.borderedProminent)
                    Button("Send Text…") { composingText = true }
                }
                .padding(.top, 4)
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
        .padding(.vertical, 36)
        .padding(.horizontal, Theme.cardPadding)
        // Three states, one shape: a calm dashed invitation when empty, a
        // solid teal edge under a drag, and a settled card once a payload
        // is staged and the question becomes which device.
        .background(
            RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous)
                .fill(targeted ? Color.teal.opacity(0.1) : Theme.cardFill)
        )
        .overlay(
            RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous)
                .strokeBorder(
                    targeted ? Color.teal : Theme.cardStroke,
                    style: targeted || staged != nil
                        ? StrokeStyle(lineWidth: 1.5)
                        : StrokeStyle(lineWidth: 1, dash: [6, 4]))
        )
        .animation(.easeInOut(duration: 0.15), value: targeted)
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
        VStack(alignment: .leading, spacing: 8) {
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
                VStack(spacing: 2) {
                    ForEach(state.endpoints) { endpoint in
                        DeviceRow(endpoint: endpoint) { state.send(to: endpoint) }
                    }
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
        .frame(maxWidth: .infinity, alignment: .leading)
        .cardSurface()
    }
}
