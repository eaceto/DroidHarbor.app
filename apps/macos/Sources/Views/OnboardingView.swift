import SwiftUI

/// First-run introduction: the three things worth knowing before the first
/// transfer, then the settings that decide whether any of it works at all.
/// Presented as a sheet over the window (see MainWindow), so the sections
/// behind it keep working; shown once, and again from Settings or Help.
///
/// All four pages are one shape: a large symbol over a centred title and
/// lead, then rows. The symbol and the heading live here rather than inside
/// the pages so they survive the page change — a view that is replaced
/// cannot morph into anything, and the symbol's replace effect and the
/// heading's crossfade both depend on the same view being handed new
/// content.
struct OnboardingView: View {
    @EnvironmentObject private var state: AppState
    @State private var page = 0

    /// Shown with `~` rather than `/Users/name`: the home folder is noise,
    /// and the point of the line is which folder, not whose.
    private var destinationPath: String {
        (state.destination.path as NSString).abbreviatingWithTildeInPath
    }

    // Titles, leads and steps are LocalizedStringKey rather than
    // String(localized:). Text renders markdown from a key, so the bold
    // survives, and it is one lookup: passing an already-localized String
    // back through Text(.init(…)) looked it up a second time and ran the
    // interpolated device name and destination through the markdown parser,
    // where a folder called **backup** came out bold.
    private var pages: [OnboardingPage] {
        [
            OnboardingPage(
                symbol: "tray.and.arrow.down",
                title: "Receive from your phone",
                lead: "Turn on receiving, then send from Android's own share sheet.",
                steps: [
                    // Describes what is actually on screen. The switch in the
                    // menu-bar popover carries no visible label, so telling
                    // anyone to "switch Receiving on" sent them looking for a
                    // word that is not there.
                    OnboardingStep(
                        symbol: "switch.2",
                        text: "Click the DroidHarbor icon in the menu bar and turn the switch on. This Mac appears as \u{201C}\(state.deviceName)\u{201D}."),
                    OnboardingStep(
                        symbol: "square.and.arrow.up",
                        text: "On the phone: pick files → **Share** → **Quick Share** → tap this Mac."),
                    OnboardingStep(
                        symbol: "checkmark.circle",
                        text: "Check the 4-digit code matches, then **Accept**. Files land in \(destinationPath), which you can change in Settings."),
                ]),
            OnboardingPage(
                symbol: "paperplane",
                title: "Send to your phone",
                lead: "Drag files, links or text onto the menu-bar icon, or use the Send tab.",
                steps: [
                    OnboardingStep(
                        symbol: "iphone",
                        text: "On the phone, open **Quick Share** (Settings → Connected devices, or the Files app) so it becomes visible."),
                    OnboardingStep(
                        symbol: "arrow.up.doc",
                        text: "Drop files, a link or some text on the DroidHarbor icon, or choose what to send in the **Send** tab."),
                    OnboardingStep(
                        symbol: "hand.tap",
                        text: "Tap the phone in the list, then accept the transfer on the phone."),
                ]),
            OnboardingPage(
                symbol: "folder",
                title: "Send straight from Finder",
                lead: "DroidHarbor installs a Share extension for Finder.",
                steps: [
                    OnboardingStep(
                        symbol: "cursorarrow.click",
                        text: "In Finder, select any file and **right-click**."),
                    OnboardingStep(
                        symbol: "square.and.arrow.up",
                        text: "Choose **Share… → DroidHarbor**."),
                    OnboardingStep(
                        symbol: "gearshape",
                        text: "If it is missing, open **System Settings → General → Login Items & Extensions** and turn the DroidHarbor **extension** on."),
                ]),
            // The closing page carries no steps: its rows are controls, and
            // SetupRows draws them.
            OnboardingPage(
                symbol: "checkmark.seal",
                title: "Ready to go",
                lead: "Three settings, all of them changeable later in Settings.",
                steps: []),
        ]
    }

    private var isLastPage: Bool { page >= pages.count - 1 }
    private var current: OnboardingPage { pages[min(page, pages.count - 1)] }

    var body: some View {
        VStack(spacing: 0) {
            // The geometry is what centres a page in the sheet. A scroll view
            // lays its content out from the top and gives it exactly the
            // height it asks for, so a short page sat against the top edge
            // with all the empty space below it. Asking for at least the
            // visible height, centred, puts the page in the middle until it
            // outgrows the sheet — which the setup page does in French and
            // Spanish — and only then does it scroll.
            GeometryReader { geometry in
                ScrollView {
                    VStack(spacing: 22) {
                        header

                        if isLastPage {
                            SetupRows()
                        } else {
                            VStack(spacing: 8) {
                                ForEach(current.steps) { step in
                                    OnboardingRow(symbol: step.symbol, text: step.text)
                                }
                            }
                            // Only the rows are replaced. The heading above
                            // them is the same view throughout, which is what
                            // lets the symbol morph rather than blink.
                            .id(page)
                            .transition(.opacity)
                        }
                    }
                    .frame(maxWidth: 520)
                    .padding(.horizontal, 28)
                    .padding(.vertical, 28)
                    .frame(
                        maxWidth: .infinity,
                        minHeight: geometry.size.height,
                        alignment: .center)
                }
            }

            // Every page, not just the last one. Turning receiving on can
            // fail, and so can the login item; on the explanation pages there
            // was nowhere for either to be said.
            AppMessages()
                .padding(.horizontal, 20)
                .padding(.bottom, 12)

            Divider()

            HStack(spacing: 10) {
                pageDots

                Spacer()

                if !isLastPage {
                    Button("Skip") { state.completeOnboarding() }
                        .buttonStyle(.link)
                }
                if page > 0 {
                    Button("Back") { go(to: page - 1) }
                }
                if !isLastPage {
                    Button("Next") { go(to: page + 1) }
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                } else {
                    Button("Start Using DroidHarbor") { state.completeOnboarding() }
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                }
            }
            .padding(14)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .tint(.teal)
    }

    /// Symbol, title and lead. Hierarchical and unframed: the tinted rounded
    /// tile it replaces was a smaller idea in a bigger box, and at this size
    /// the symbol can carry the page on its own.
    private var header: some View {
        VStack(spacing: 8) {
            Image(systemName: current.symbol)
                .font(.system(size: 76, weight: .light))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(Color.teal)
                .frame(height: 84)
                .contentTransition(.symbolEffect(.replace))
                .accessibilityHidden(true)

            Text(current.title)
                .font(.title2.weight(.semibold))
                .multilineTextAlignment(.center)
                .contentTransition(.opacity)

            Text(current.lead)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .contentTransition(.opacity)
        }
        .padding(.bottom, 2)
    }

    /// Clickable, so going back three pages is one click rather than three.
    private var pageDots: some View {
        HStack(spacing: 6) {
            ForEach(0..<pages.count, id: \.self) { index in
                Button { go(to: index) } label: {
                    Circle()
                        .fill(index == page ? AnyShapeStyle(Color.teal) : AnyShapeStyle(.quaternary))
                        .frame(width: 7, height: 7)
                        // The dot is small; the target should not be.
                        .padding(4)
                        .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text("Page \(index + 1) of \(pages.count)"))
                .accessibilityAddTraits(index == page ? [.isSelected] : [])
            }
        }
    }

    private func go(to index: Int) {
        withAnimation(.easeInOut(duration: 0.28)) {
            page = min(max(index, 0), pages.count - 1)
        }
    }
}

private struct OnboardingStep: Identifiable {
    let symbol: String
    let text: LocalizedStringKey
    var id: String { symbol + "\(text)" }
}

private struct OnboardingPage {
    let symbol: String
    let title: LocalizedStringKey
    let lead: LocalizedStringKey
    let steps: [OnboardingStep]
}

/// The closing page's rows. None of these can be settled well anywhere else:
/// receiving is what makes the Mac exist as far as the phone is concerned,
/// the notification prompt is a one-shot that should follow an explanation,
/// and a menu-bar app that is not running is invisible, so the login item is
/// worth raising before the first transfer rather than leaving to be
/// discovered in Settings. All three stay changeable in Settings afterwards.
private struct SetupRows: View {
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

/// One row: a symbol, what it says, and optionally the control for it. The
/// explanation steps and the setup settings are the same object drawn twice,
/// which is what makes the four pages read as one design. It also retired
/// the numbered teal circles, whose white digits sat at about 2.2:1 against
/// their own background.
private struct OnboardingRow<Control: View>: View {
    let symbol: String
    var title: LocalizedStringKey?
    let text: LocalizedStringKey
    @ViewBuilder var control: () -> Control

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 14, weight: .semibold))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(Color.teal)
                .frame(width: 22, height: 22)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                if let title {
                    Text(title)
                        .font(.callout.weight(.medium))
                }
                Text(text)
                    .font(.callout)
                    .foregroundStyle(title == nil ? .primary : .secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            control()
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(.quaternary)
        )
    }
}

extension OnboardingRow where Control == EmptyView {
    init(symbol: String, text: LocalizedStringKey) {
        self.init(symbol: symbol, title: nil, text: text) { EmptyView() }
    }
}
