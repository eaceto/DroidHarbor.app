import SwiftUI

/// Window root: a sidebar over the three things the app does.
struct MainWindow: View {
    enum Section: String, CaseIterable, Identifiable, Codable {
        case transfers
        case send
        case settings
        case about

        var id: String { rawValue }

        /// What the app does. `about` is not among them: it is about the app
        /// rather than a place to work, so the sidebar keeps it below a rule.
        static let primary: [Section] = [.transfers, .send, .settings]

        var title: LocalizedStringKey {
            switch self {
            case .transfers: return "Transfers"
            case .send: return "Send"
            case .settings: return "Settings"
            case .about: return "About"
            }
        }

        var symbolName: String {
            switch self {
            case .transfers: return "tray.and.arrow.down"
            case .send: return "paperplane"
            case .settings: return "gearshape"
            case .about: return "info.circle"
            }
        }
    }

    @EnvironmentObject private var state: AppState
    /// Kept in state so the sidebar starts visible; the toggle SwiftUI puts
    /// in the toolbar drives this, and nothing else collapses it.
    @State private var columns: NavigationSplitViewVisibility = .all

    private var selectionBinding: Binding<Section> {
        Binding(
            get: { Section(rawValue: state.selectedSection) ?? .transfers },
            set: { state.selectSection($0.rawValue) }
        )
    }

    /// The introduction is a sheet rather than a branch that replaces the
    /// window. Replacing it meant "Show the introduction again" took the
    /// sections away with it, and receiving is restored across launches, so
    /// a transfer arriving during the introduction had its consent card,
    /// its badge and its section switch all inside the branch that was not
    /// being drawn. A sheet leaves that machinery running underneath, and
    /// an arriving transfer closes it (the card is the more urgent thing;
    /// Help → DroidHarbor Help brings the introduction back).
    var body: some View {
        sections
            .frame(minWidth: Self.minWidth, minHeight: Self.minHeight)
            // The setter deliberately ignores writes: the sheet closes when
            // the state says the introduction is done, and nothing else may
            // decide that. SwiftUI also calls the setter when it tears the
            // sheet down at quit, and completing onboarding there marked the
            // introduction as read for anyone who quit while it was open.
            .sheet(isPresented: Binding(
                get: { !state.hasOnboarded },
                set: { _ in }
            )) {
                OnboardingView()
                    .frame(width: 720, height: 520)
                    .presentationBackground(.thinMaterial)
            }
            .onChange(of: state.transfer?.session) { _, session in
                if session != nil, !state.hasOnboarded { state.completeOnboarding() }
            }
    }

    /// Smallest useful window: the sidebar at its 160pt minimum beside a
    /// detail column wide enough for a transfer row's name and size, and
    /// never smaller than the introduction sheet it has to host — a sheet is
    /// its own window and is not clipped to its parent, so one wider than the
    /// window simply hangs over the edges.
    static let minWidth: CGFloat = 760
    static let minHeight: CGFloat = 580

    private var sections: some View {
        NavigationSplitView(columnVisibility: $columns) {
            List(selection: selectionBinding) {
                ForEach(Section.primary) { row($0) }
                Divider()
                row(.about)
            }
            .navigationSplitViewColumnWidth(min: 160, ideal: 180, max: 220)
            .listStyle(.sidebar)
        } detail: {
            Group {
                switch selectionBinding.wrappedValue {
                case .transfers: TransfersView()
                case .send: SendView()
                case .settings: SettingsView()
                // The same panel the menu's About opens. It is a fixed-width
                // column, so it is centred here rather than left in the
                // corner of a wide detail pane.
                case .about: AboutView().frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        // Balanced keeps the sidebar beside the detail instead of letting
        // the detail claim the whole window and hide it, which is what
        // happened at smaller sizes.
        .navigationSplitViewStyle(.balanced)
        .tint(.teal)
        // An incoming transfer should be visible wherever the user was.
        .onChange(of: state.transfer?.session) { _, session in
            if session != nil { selectionBinding.wrappedValue = .transfers }
        }
        .onChange(of: state.pendingSend) { _, staged in
            if staged != nil { selectionBinding.wrappedValue = .send }
        }
    }

    private func row(_ item: Section) -> some View {
        Label(item.title, systemImage: item.symbolName)
            .badge(badge(for: item))
            .tag(item)
    }

    private func badge(for item: Section) -> Int {
        switch item {
        case .transfers: return state.transfer != nil ? 1 : 0
        case .send: return (state.outbound != nil || state.pendingSend != nil) ? 1 : 0
        case .settings, .about: return 0
        }
    }
}

/// Shared empty-state block, with room for the action it suggests.
struct EmptyStateView<Action: View>: View {
    let symbol: String
    let title: String
    let message: String
    @ViewBuilder var action: () -> Action

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 34, weight: .light))
                .foregroundStyle(.tertiary)
                .accessibilityHidden(true)
            Text(title)
                .font(.headline)
            Text(message)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                // No fixedSize here. Inside a view that fills its container,
                // asking the text for its ideal height makes it size against
                // an unconstrained width proposal and report a hugely tall
                // result, which propagated up and pushed the split view (and
                // the sidebar with it) far past the window. A bounded width is
                // enough for the text to wrap.
                .frame(maxWidth: 340)
            action()
                .padding(.top, 2)
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

extension EmptyStateView where Action == EmptyView {
    init(symbol: String, title: String, message: String) {
        self.init(symbol: symbol, title: title, message: message) { EmptyView() }
    }
}

/// Whatever the app currently has to say, in the order it should be read.
/// Every section shows this in the same place rather than picking its own
/// subset, which is how the introduction ended up with no way to report a
/// failure on three of its four pages.
struct AppMessages: View {
    @EnvironmentObject private var state: AppState

    var body: some View {
        // An explicit EmptyView when there is nothing to say. A stack skips
        // EmptyView entirely, where a group that merely renders nothing still
        // takes its share of the surrounding spacing and left a gap above
        // the first card.
        if state.lastNotice == nil, state.lastError == nil {
            EmptyView()
        } else {
            VStack(spacing: 8) {
                if let notice = state.lastNotice {
                    MessageStrip(kind: .notice, message: notice) { state.dismissNotice() }
                }
                if let error = state.lastError {
                    MessageStrip(kind: .error, message: error) { state.dismissError() }
                }
            }
        }
    }
}

/// Inline message strip used by the window sections.
struct MessageStrip: View {
    /// A failure and a piece of news look nothing alike to a reader, and
    /// dressing "DroidHarbor is up to date." as an orange warning was
    /// telling the user something had gone wrong.
    enum Kind {
        case error
        case notice

        var symbol: String {
            switch self {
            case .error: return "exclamationmark.triangle.fill"
            case .notice: return "info.circle.fill"
            }
        }

        var color: Color {
            switch self {
            case .error: return .orange
            case .notice: return .teal
            }
        }
    }

    var kind: Kind = .error
    let message: String
    let onDismiss: () -> Void

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: kind.symbol)
                .foregroundStyle(kind.color)
            Text(message)
                .fixedSize(horizontal: false, vertical: true)
            Spacer()
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
        .font(.callout)
        .padding(10)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(kind.color.opacity(0.12))
        )
    }
}
