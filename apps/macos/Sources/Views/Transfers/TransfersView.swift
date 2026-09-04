import SwiftUI

/// Live transfer plus persistent history.
struct TransfersView: View {
    @EnvironmentObject var state: AppState
    @State var category: HistoryEntry.Category = .all
    @State var query = ""
    @State var confirmingClear = false
    @State var selectedEntry: HistoryEntry.ID?

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

    /// Whether "what is shown" is a subset of the history worth offering to
    /// clear on its own.
    private var isFiltered: Bool {
        category != .all || !query.trimmingCharacters(in: .whitespaces).isEmpty
    }

    /// The entry a selection-scoped menu or double-click refers to.
    private func entry(for ids: Set<HistoryEntry.ID>) -> HistoryEntry? {
        guard let id = ids.first else { return nil }
        return state.history.first { $0.id == id }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.cardGap) {
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
        .padding(Theme.pagePadding)
        .searchable(
            text: $query,
            placement: .toolbar,
            prompt: Text("Search name, extension, link or sender"))
        // A category with nothing left in it would strand the user on an
        // empty list.
        .onChange(of: state.history.count) { _, _ in
            if !availableCategories.contains(category) { category = .all }
        }
        .onAppear { state.pruneMissingHistory() }
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

    /// One card: header and filter above a rule, the list below it running
    /// to the card's edges. The card is what separates the persistent record
    /// from the live cards above it.
    var historySection: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("History")
                        .font(.headline)
                    Spacer()
                    // One click used to wipe the whole record; unlike a
                    // swipe-removed row there is no putting it back, so it
                    // asks first. With a filter or search active the dialog
                    // offers both readings of "clear": just what is shown,
                    // or everything.
                    Button("Clear", role: .destructive) { confirmingClear = true }
                        .controlSize(.small)
                        .confirmationDialog(
                            "Clear history?",
                            isPresented: $confirmingClear
                        ) {
                            if isFiltered {
                                Button("Clear Shown (\(visibleHistory.count))", role: .destructive) {
                                    state.removeFromHistory(visibleHistory)
                                }
                                Button("Clear All", role: .destructive) {
                                    state.clearHistory()
                                }
                            } else {
                                Button("Clear History", role: .destructive) {
                                    state.clearHistory()
                                }
                            }
                            Button("Cancel", role: .cancel) {}
                        } message: {
                            Text("Files stay where they were saved. Only the record of past transfers is cleared.")
                        }
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
            }
            .padding([.horizontal, .top], Theme.cardPadding)
            .padding(.bottom, 12)

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
                    List(visibleHistory, id: \.id, selection: $selectedEntry) { entry in
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
                    }
                    .listStyle(.inset)
                    // Deliberately no alternating stripes: inside the card
                    // they continued past the last row as skeleton-like bars
                    // and fought the panel. Separators + selection carry the
                    // native reading on their own.
                    // The card supplies the surface; the list must not paint
                    // its own behind the rows.
                    .scrollContentBackground(.hidden)
                    // Selection-scoped, the way native lists behave: a
                    // right-click selects the row and menus it, and a
                    // double-click opens the entry in whatever sense it has
                    // an "open".
                    .contextMenu(forSelectionType: HistoryEntry.ID.self) { ids in
                        if let entry = entry(for: ids) {
                            HistoryActions(entry: entry, iconOnly: false)
                            Divider()
                            Button("Remove from History") {
                                state.removeFromHistory(entry)
                            }
                        }
                    } primaryAction: { ids in
                        if let entry = entry(for: ids) {
                            HistoryActions.primary(for: entry, in: state)
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .cardSurface(padding: 0)
    }
}
