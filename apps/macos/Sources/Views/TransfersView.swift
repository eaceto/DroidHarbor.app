import SwiftUI

/// Live transfer plus persistent history.
struct TransfersView: View {
    @EnvironmentObject var state: AppState
    @State var category: HistoryEntry.Category = .all
    @State var query = ""

    /// Only offer categories that something in the history actually falls
    /// into, so the control does not advertise empty filters.
    private var availableCategories: [HistoryEntry.Category] {
        let present = Set(state.history.map(\.category))
        return HistoryEntry.Category.allCases.filter { $0 == .all || present.contains($0) }
    }

    private var visibleHistory: [HistoryEntry] {
        state.history
            .filter { category == .all || $0.category == category }
            .filter { $0.matches(query) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            AppMessages()

            if let transfer = state.transfer {
                IncomingCard(transfer: transfer)
            }

            if let text = state.receivedText {
                ReceivedTextCard(text: text) { state.dismissReceivedText() }
            }

            if state.history.isEmpty && state.transfer == nil {
                EmptyStateView(
                    symbol: "tray",
                    title: state.receiving
                        ? String(localized: "Ready to receive")
                        : String(localized: "Receiving is off"),
                    message: state.receiving
                        ? String(localized: "On the phone: pick files, then Share → Quick Share → \u{201C}\(state.deviceName)\u{201D}.")
                        : String(localized: "Turn receiving on to accept files from nearby Android devices.")
                ) {
                    // An empty screen should offer the next step, not just
                    // describe it.
                    if !state.receiving {
                        Button("Turn On Receiving") { state.setReceiving(true) }
                            .buttonStyle(.borderedProminent)
                            .keyboardShortcut(.defaultAction)
                    }
                }
            } else if !state.history.isEmpty {
                historySection
            } else {
                // Only when nothing above fills: exactly one greedy element
                // in the stack, so nothing has to fight for the leftovers.
                Spacer(minLength: 0)
            }
        }
        .padding(16)
        .searchable(
            text: $query,
            placement: .toolbar,
            prompt: Text("Search name, extension, link or sender"))
        // A category with nothing left in it would strand the user on an
        // empty list.
        .onChange(of: state.history.count) { _, _ in
            if !availableCategories.contains(category) { category = .all }
        }
    }
}

/// The history list with its controls.
///
/// The list and the "nothing matches" message live inside one region that
/// always fills the remaining space. Switching filters then changes only
/// what is inside that region, never its geometry. Previously the two were
/// siblings of different types, so each change tore one down and built the
/// other, and their competing height demands pushed the rows up behind the
/// filter.
extension TransfersView {
    var categoryPicker: some View {
        Picker("Show", selection: $category) {
            ForEach(availableCategories) { item in
                Text(item.title).tag(item)
            }
        }
        .labelsHidden()
    }

    var historySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("History")
                    .font(.headline)
                Spacer()
                Button("Clear", role: .destructive) { state.clearHistory() }
                    .controlSize(.small)
            }

            // Segments stay readable up to a handful; beyond that they
            // squeeze into unreadable slivers, so switch to a menu.
            if availableCategories.count > 6 {
                categoryPicker
                    .pickerStyle(.menu)
                    .fixedSize()
            } else if availableCategories.count > 1 {
                categoryPicker
                    .pickerStyle(.segmented)
            }

            Divider()

            ZStack {
                if visibleHistory.isEmpty {
                    VStack(spacing: 6) {
                        Image(systemName: "magnifyingglass")
                            .font(.title)
                            .foregroundStyle(.tertiary)
                        Text("Nothing matches")
                            .font(.headline)
                        Text("Try a different search or category. Names, extensions, links and senders are all searched.")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: 320)
                    }
                    .padding(24)
                } else {
                    // Rows are identified by the entry's own stable id, so a
                    // filter change is a diff rather than a rebuild.
                    List(visibleHistory, id: \.id) { entry in
                        HistoryRow(entry: entry)
                            .listRowSeparator(.visible)
                            // Swipe left to forget a transfer. The wording is
                            // "Remove", not "Delete": the file it refers to
                            // stays where it was saved.
                            .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                                Button(role: .destructive) {
                                    state.removeFromHistory(entry)
                                } label: {
                                    Label("Remove", systemImage: "xmark.circle")
                                }
                            }
                            // The same action where macOS users look for it.
                            .contextMenu {
                                Button("Remove from History") {
                                    state.removeFromHistory(entry)
                                }
                            }
                    }
                    .listStyle(.inset)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

private struct IncomingCard: View {
    @EnvironmentObject private var state: AppState
    let transfer: ActiveTransfer
    @State private var alwaysAccept = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    // No fixedSize here: inside the split view it reports
                    // an ideal height for the whole column that is taller
                    // than the window, and everything gets pushed off the
                    // top, sidebar included.
                    Text(title)
                        .font(.headline)
                        .lineLimit(2)
                    Text(fileSummary)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if transfer.receiving {
                    Button("Cancel", role: .destructive) { state.cancel(transfer.session) }
                } else {
                    HStack(spacing: 8) {
                        Button("Decline") { state.decline(transfer.session) }
                        Button("Accept") {
                            if alwaysAccept { state.trust(transfer.senderName) }
                            state.accept(transfer.session)
                        }
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                    }
                }
            }

            if !transfer.receiving {
                HStack(spacing: 8) {
                    Text("Code")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    Text(transfer.token)
                        .font(.system(.title, design: .rounded).weight(.semibold).monospacedDigit())
                        .accessibilityLabel(Text("Confirmation code \(transfer.token)"))
                    Text("must match the code shown on the phone")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Toggle(isOn: $alwaysAccept) {
                    Text("Always accept from \u{201C}\(transfer.senderName)\u{201D}")
                        .font(.callout)
                }
                .toggleStyle(.checkbox)
            } else if transfer.totalBytes > 0 {
                ProgressView(
                    value: Double(transfer.bytesReceived),
                    total: Double(transfer.totalBytes))
                    .accessibilityLabel(Text("Transfer progress"))
                HStack {
                    Text("\(Format.bytes(transfer.bytesReceived)) of \(Format.bytes(transfer.totalBytes))")
                    Spacer()
                    if transfer.rate.bytesPerSecond > 0 {
                        Text(Format.rate(transfer.rate.bytesPerSecond))
                        if let left = transfer.rate.secondsRemaining {
                            Text("·")
                            Text(Format.remaining(left))
                        }
                    }
                }
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
            }

            // Per-file rows: only the window has room for these, and only
            // so many of them. A long batch scrolls instead of growing the
            // card past the height of the window.
            if transfer.files.count > 4 {
                ScrollView { fileRows }
                    .frame(maxHeight: 132)
            } else {
                fileRows
            }
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(.quaternary.opacity(0.5))
        )
    }

    private var fileRows: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(transfer.files) { file in
                FileProgressRow(
                    file: file,
                    current: transfer.currentFile == file.name,
                    showsProgress: transfer.files.count > 1)
            }
        }
    }

    private var fileSummary: String {
        if let preview = transfer.textPreview {
            return preview.isEmpty ? String(localized: "A link or text") : preview
        }
        let size = Format.bytes(transfer.totalBytes)
        return transfer.files.count == 1
            ? String(localized: "1 file · \(size)")
            : String(localized: "\(transfer.files.count) files · \(size)")
    }

    private var title: String {
        transfer.receiving
            ? String(localized: "Receiving from \u{201C}\(transfer.senderName)\u{201D}")
            : String(localized: "\u{201C}\(transfer.senderName)\u{201D} wants to send files")
    }
}

/// Text and links never touch the disk, so they are shown here until
/// dismissed: the clipboard already has the content.
private struct ReceivedTextCard: View {
    let text: ReceivedText
    let onDismiss: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: text.kind == "link" ? "link" : "doc.on.clipboard")
                .font(.title3)
                .foregroundStyle(Color.teal)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 3) {
                Text(text.kind == "link"
                    ? String(localized: "Link copied to the clipboard")
                    : String(localized: "Text copied to the clipboard"))
                    .font(.callout.weight(.medium))
                Text(text.content)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(3)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                // Same restriction as the history row: only web links are
                // offered, since the content came from another device.
                if text.kind == "link", let url = AppState.webURL(from: text.content) {
                    Link("Open link", destination: url)
                        .font(.callout)
                }
            }
            Spacer()
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text("Dismiss"))
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(.quaternary.opacity(0.5))
        )
    }
}

private struct FileProgressRow: View {
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

private struct HistoryRow: View {
    @EnvironmentObject private var state: AppState
    let entry: HistoryEntry
    @State private var hovered = false

    /// Only files can go missing from disk; a link is always "there".
    private var isMissing: Bool {
        guard entry.isFile, entry.direction == .received else { return false }
        guard let path = entry.paths.first else { return true }
        return !FileManager.default.fileExists(atPath: path)
    }

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: entry.symbolName)
                .foregroundStyle(entry.direction == .received ? Color.teal : Color.secondary)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 1) {
                Text(entry.summary)
                    .lineLimit(1)
                    .truncationMode(entry.isFile ? .middle : .tail)
                Text(entry.direction == .received
                    ? String(localized: "from \(entry.peer) · \(entry.date.formatted(date: .abbreviated, time: .shortened))")
                    : String(localized: "to \(entry.peer) · \(entry.date.formatted(date: .abbreviated, time: .shortened))"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .opacity(isMissing ? 0.5 : 1)

            Spacer()

            if hovered {
                HStack(spacing: 2) { actions(iconOnly: true) }
            }
        }
        .padding(.vertical, 3)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        // Titles in the menu, icons on the row: the same actions either way.
        .contextMenu { actions(iconOnly: false) }
    }

    /// What a row offers depends on what it is: a link opens in the browser
    /// and can be copied, but has nothing to reveal in Finder.
    @ViewBuilder private func actions(iconOnly: Bool) -> some View {
        // A file on disk can always be revealed and copied, whatever it
        // holds; the kind adds what is meaningful on top of that.
        switch entry.kind {
        case .files:
            fileActions(iconOnly: iconOnly)

        case .contact:
            if let path = entry.paths.first {
                RowButton(symbol: "person.crop.circle.badge.plus",
                          label: "Add to Contacts", iconOnly: iconOnly) {
                    state.open(path: path)
                }
                fileActions(iconOnly: iconOnly, includeOpen: false)
            } else if let content = entry.content {
                RowButton(symbol: "person.crop.circle.badge.plus",
                          label: "Add to Contacts", iconOnly: iconOnly) {
                    state.openAsFile(content, extension: "vcf")
                }
                copyButton(content, label: "Copy contact", iconOnly: iconOnly)
            }

        case .calendar:
            if let path = entry.paths.first {
                RowButton(symbol: "calendar.badge.plus",
                          label: "Add to Calendar", iconOnly: iconOnly) {
                    state.open(path: path)
                }
                fileActions(iconOnly: iconOnly, includeOpen: false)
            } else if let content = entry.content {
                RowButton(symbol: "calendar.badge.plus",
                          label: "Add to Calendar", iconOnly: iconOnly) {
                    state.openAsFile(content, extension: "ics")
                }
                copyButton(content, label: "Copy event", iconOnly: iconOnly)
            }

        case .phone:
            if let content = entry.content {
                RowButton(symbol: "phone", label: "Call", iconOnly: iconOnly) {
                    state.call(content)
                }
                copyButton(content, label: "Copy number", iconOnly: iconOnly)
            }

        case .email:
            if let content = entry.content {
                RowButton(symbol: "envelope", label: "Write email", iconOnly: iconOnly) {
                    state.compose(to: content)
                }
                copyButton(content, label: "Copy address", iconOnly: iconOnly)
            }

        case .map:
            if let content = entry.content {
                RowButton(symbol: "map", label: "Open in Maps", iconOnly: iconOnly) {
                    state.showOnMap(content)
                }
                copyButton(content, label: "Copy place", iconOnly: iconOnly)
            }

        case .link:
            if let content = entry.content {
                // Anything can be copied; only web links can be opened, so
                // schemes that would launch another app are kept but offer
                // no Open action rather than one that always fails.
                if AppState.webURL(from: content) != nil {
                    RowButton(
                        symbol: "arrow.up.forward.app", label: "Open link", iconOnly: iconOnly
                    ) {
                        state.openLink(content)
                    }
                }
                copyButton(content, label: "Copy link", iconOnly: iconOnly)
            }

        case .text, .wifi:
            if let content = entry.content {
                copyButton(content, label: "Copy text", iconOnly: iconOnly)
            }
        }
    }

    @ViewBuilder private func fileActions(iconOnly: Bool, includeOpen: Bool = true) -> some View {
        if entry.direction == .received, let path = entry.paths.first {
            if includeOpen {
                RowButton(symbol: "arrow.up.forward.app", label: "Open", iconOnly: iconOnly) {
                    state.open(path: path)
                }
            }
            RowButton(symbol: "magnifyingglass", label: "Show in Finder", iconOnly: iconOnly) {
                state.reveal(path: path)
            }
            RowButton(symbol: "doc.on.clipboard", label: "Copy path", iconOnly: iconOnly) {
                state.copyPath(path)
            }
        }
    }

    private func copyButton(
        _ content: String, label: LocalizedStringKey, iconOnly: Bool
    ) -> some View {
        RowButton(symbol: "doc.on.clipboard", label: label, iconOnly: iconOnly) {
            state.copy(content)
        }
    }
}

/// A row action, rendered as an icon on hover and as a titled item in the
/// context menu: the same button in both places.
private struct RowButton: View {
    let symbol: String
    let label: LocalizedStringKey
    let iconOnly: Bool
    let action: () -> Void

    var body: some View {
        Group {
            if iconOnly {
                Button(action: action) { Image(systemName: symbol) }
                    .buttonStyle(.borderless)
            } else {
                Button(action: action) { Label(label, systemImage: symbol) }
            }
        }
        .help(label)
        .accessibilityLabel(Text(label))
    }
}
