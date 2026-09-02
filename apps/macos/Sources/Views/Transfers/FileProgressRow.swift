import SwiftUI

/// One file of an incoming batch: state symbol, name, size, and a bar while
/// it is the one being written.
struct FileProgressRow: View {
    let file: OfferedFile
    let current: Bool
    /// False for a lone file: the card's own bar already tracks it, and two
    /// bars counting the same bytes tell you nothing twice.
    var showsProgress = true

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 8) {
                Image(systemName: symbol)
                    .font(.caption)
                    .foregroundStyle(file.completed ? Color.teal : .secondary)
                    .frame(width: 14)
                    .accessibilityHidden(true)
                Text(file.name)
                    .font(.callout)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                if file.size > 0 {
                    Text(Format.bytes(file.size))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
            }
            if showsProgress, !file.completed, current, let fraction = file.fraction {
                ProgressView(value: fraction)
                    .controlSize(.small)
                    .padding(.leading, 22)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private var symbol: String {
        if file.completed { return "checkmark.circle.fill" }
        return current ? "arrow.down.circle" : "circle"
    }
}
