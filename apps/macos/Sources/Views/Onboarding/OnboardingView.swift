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
