import SwiftUI

/// An outbound transfer: waiting for the phone's consent, then progress.
struct OutboundCard: View {
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
                        .font(.subheadline)
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
                        HStack(spacing: 10) {
                            CodeTicket(code: outbound.token)
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
        // Tinted while the phone's user is deciding, matching the incoming
        // consent card; neutral once bytes are moving.
        .cardSurface(tinted: outbound.awaitingConsent)
    }
}
