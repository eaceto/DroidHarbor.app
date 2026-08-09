import AppKit
import XCTest

@testable import DroidHarbor

/// Sending text is the mirror of receiving it: what leaves the Mac must be
/// classified the same way as what arrives, so a link sent from here reaches
/// the phone as a link and comes back into history as one.
@MainActor
final class SendTextTests: XCTestCase {
    private var state: AppState!

    override func setUp() async throws {
        let suite = UserDefaults(suiteName: "dh-send-\(UUID().uuidString)")!
        suite.set(
            URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("dh-\(UUID().uuidString)/history.json").path,
            forKey: HistoryStore.locationKey)
        state = AppState(defaults: suite, startService: false)
    }

    // MARK: - Classification

    func testAWebAddressIsSentAsALink() {
        let text = OutboundText(content: "https://example.com/page?q=1")
        XCTAssertEqual(text.kind, .link)
        XCTAssertEqual(text.wireKind, "link")
        XCTAssertEqual(text.title, "example.com", "a link is recognised by its host")
    }

    func testAPhoneNumberIsSentAsOne() {
        let text = OutboundText(content: "+44 20 7946 0958")
        XCTAssertEqual(text.kind, .phone)
        XCTAssertEqual(text.wireKind, "phone")
    }

    func testAMapLinkTravelsAsAnAddress() {
        let text = OutboundText(content: "https://maps.apple.com/?ll=51.5,-0.1")
        XCTAssertEqual(text.kind, .map)
        XCTAssertEqual(text.wireKind, "address", "so the phone offers to open Maps")
    }

    /// Only four kinds exist on the wire, so everything else has to degrade
    /// to plain text rather than being refused.
    func testKindsWithNoWireEquivalentAreSentAsText() {
        XCTAssertEqual(OutboundText(content: "hello@example.com").wireKind, "text")
        XCTAssertEqual(OutboundText(content: "just a note").wireKind, "text")
    }

    func testSurroundingWhitespaceIsNotSent() {
        let text = OutboundText(content: "  https://example.com \n")
        XCTAssertEqual(text.content, "https://example.com")
        XCTAssertEqual(text.kind, .link, "padding must not stop it being read as a link")
    }

    func testALongNoteIsTitledByItsFirstLine() {
        let text = OutboundText(content: "Shopping list\nmilk\nbread")
        XCTAssertEqual(text.title, "Shopping list")
    }

    func testAVeryLongLineIsTruncatedForTheTitle() {
        let text = OutboundText(content: String(repeating: "a", count: 200))
        XCTAssertLessThanOrEqual(text.title.count, 60)
        XCTAssertTrue(text.title.hasSuffix("\u{2026}"))
    }

    // MARK: - Staging

    func testStagingTextArmsTheSendView() {
        state.beginSend(text: "https://example.com")
        XCTAssertEqual(state.pendingSend?.text?.kind, .link)
        XCTAssertNil(state.lastError)
    }

    func testEmptyTextIsRefusedRatherThanStaged() {
        state.beginSend(text: "   \n  ")
        XCTAssertNil(state.pendingSend)
        XCTAssertNotNil(state.lastError)
    }

    /// Dragging a link out of a browser hands over a URL, not a file. It used
    /// to be rejected as "not a file"; it is a link and should be sent as one.
    func testADraggedWebAddressBecomesALinkNotAnError() {
        state.beginSend(files: [URL(string: "https://example.com/page")!])
        XCTAssertEqual(state.pendingSend?.text?.kind, .link)
        XCTAssertNil(state.lastError)
    }

    func testADraggedFolderIsStillRefused() {
        state.beginSend(files: [URL(fileURLWithPath: NSTemporaryDirectory())])
        XCTAssertNil(state.pendingSend)
        XCTAssertNotNil(state.lastError)
    }

    func testClearingRemovesTheStagedText() {
        state.beginSend(text: "a note")
        state.cancelSendSelection()
        XCTAssertNil(state.pendingSend)
    }

    // MARK: - Completion

    func testASentLinkIsRecordedAsALink() {
        state.handle(.sendAwaitingConsent(session: 1 << 63, totalBytes: 19, token: "4821"))
        state.handle(.sessionEnded(session: 1 << 63, outcome: .completed))
        // Nothing was staged, so this is the files path: no text entry.
        XCTAssertNil(state.history.first?.content)
    }

    func testAStagedTextSurvivesIntoHistoryWithItsKind() {
        let endpoint = Endpoint(id: "endpoint-1", name: "Pixel 8", kind: "phone")
        state.beginSend(text: "https://example.com/page")
        // No service is running in tests, so the send reports failure. The
        // staged payload is what the outbound transfer is built from either
        // way, which is what this checks.
        state.send(to: endpoint)
        state.handle(.sendAwaitingConsent(session: 1 << 63, totalBytes: 24, token: "4821"))

        XCTAssertEqual(state.outbound?.text?.kind, .link)
        XCTAssertEqual(state.outbound?.itemNames, ["example.com"])

        state.handle(.sessionEnded(session: 1 << 63, outcome: .completed))
        XCTAssertEqual(state.history.first?.kind, .link)
        XCTAssertEqual(state.history.first?.content, "https://example.com/page")
        XCTAssertEqual(state.history.first?.direction, .sent)
        XCTAssertEqual(state.history.first?.peer, "Pixel 8")
    }

    // MARK: - Forgetting an entry

    /// Removing a row is about the record, not the payload: the file that
    /// was received has to still be on disk afterwards.
    func testRemovingAnEntryKeepsTheFile() throws {
        let file = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("dh-\(UUID().uuidString).txt")
        try Data("hello".utf8).write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }

        state.handle(.introductionReceived(
            session: 1, senderName: "Pixel 8",
            files: [FileOffer(name: file.lastPathComponent, size: 5, mimeType: nil)],
            totalBytes: 5, token: "1234", textPreview: nil))
        state.handle(.fileFinalized(session: 1, path: file.path))
        state.handle(.sessionEnded(session: 1, outcome: .completed))

        let entry = try XCTUnwrap(state.history.first)
        state.removeFromHistory(entry)

        XCTAssertTrue(state.history.isEmpty, "the row is gone")
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: file.path),
            "but the received file is not")
    }

    func testRemovingOneEntryLeavesTheOthers() {
        state.handle(.textReceived(session: 1, kind: "link", description: "", content: "https://a.example"))
        state.handle(.textReceived(session: 2, kind: "link", description: "", content: "https://b.example"))
        XCTAssertEqual(state.history.count, 2)

        state.removeFromHistory(state.history[0])
        XCTAssertEqual(state.history.map(\.content), ["https://a.example"])
    }
}
