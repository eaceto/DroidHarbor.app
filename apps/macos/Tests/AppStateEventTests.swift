import AppKit
import XCTest

@testable import DroidHarbor

/// Exercises the domain events exactly as the Rust side delivers them. This
/// is the layer where a bug quietly deleted received files, and none of it
/// was covered before.
@MainActor
final class AppStateEventTests: XCTestCase {
    private var state: AppState!

    override func setUp() async throws {
        let suite = UserDefaults(suiteName: "dh-events-\(UUID().uuidString)")!
        // Keep history in a scratch file rather than the user's own.
        suite.set(
            URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("dh-\(UUID().uuidString)/history.json").path,
            forKey: HistoryStore.locationKey)
        state = AppState(defaults: suite, startService: false)
    }

    private func offer(_ name: String, _ size: UInt64 = 100) -> FileOffer {
        FileOffer(name: name, size: size, mimeType: nil)
    }

    private func introduce(sender: String = "Pixel 8", files: [FileOffer]) {
        state.handle(.introductionReceived(
            session: 1, senderName: sender, files: files,
            totalBytes: files.reduce(0) { $0 + $1.size }, token: "1234",
            textPreview: nil))
    }

    // MARK: - Receiving

    func testIntroductionPopulatesTheTransfer() {
        introduce(files: [offer("a.png", 100), offer("b.png", 200)])

        let transfer = state.transfer
        XCTAssertEqual(transfer?.senderName, "Pixel 8")
        XCTAssertEqual(transfer?.files.count, 2)
        XCTAssertEqual(transfer?.totalBytes, 300)
        XCTAssertEqual(transfer?.token, "1234")
        XCTAssertEqual(transfer?.receiving, false, "consent has not been given yet")
    }

    func testProgressUpdatesPerFileAndOverall() {
        introduce(files: [offer("a.png", 100), offer("b.png", 200)])

        state.handle(.progress(
            session: 1, bytesReceived: 150, totalBytes: 300, currentFile: "b.png",
            files: [
                FileProgress(name: "a.png", bytesTransferred: 100, size: 100, completed: true),
                FileProgress(name: "b.png", bytesTransferred: 50, size: 200, completed: false),
            ]))

        XCTAssertEqual(state.transfer?.bytesReceived, 150)
        XCTAssertEqual(state.transfer?.currentFile, "b.png")
        XCTAssertEqual(state.transfer?.files.first?.completed, true)
        XCTAssertEqual(state.transfer?.files.last?.bytesTransferred, 50)
        XCTAssertEqual(state.transfer?.files.last?.fraction, 0.25)
    }

    func testProgressForAnUnknownSessionIsIgnored() {
        introduce(files: [offer("a.png")])
        state.handle(.progress(
            session: 999, bytesReceived: 50, totalBytes: 100,
            currentFile: "", files: []))
        XCTAssertEqual(state.transfer?.bytesReceived, 0)
    }

    func testCompletedTransferIsRecordedInHistory() {
        introduce(files: [offer("a.png")])
        state.handle(.fileFinalized(session: 1, path: "/tmp/a.png"))
        state.handle(.sessionEnded(session: 1, outcome: .completed))

        XCTAssertNil(state.transfer, "the card should clear when the session ends")
        XCTAssertEqual(state.history.count, 1)
        XCTAssertEqual(state.history.first?.direction, .received)
        XCTAssertEqual(state.history.first?.peer, "Pixel 8")
        XCTAssertEqual(state.history.first?.paths, ["/tmp/a.png"])
    }

    func testDeclinedTransferLeavesNoHistory() {
        introduce(files: [offer("a.png")])
        state.handle(.sessionEnded(session: 1, outcome: .rejected))
        XCTAssertTrue(state.history.isEmpty)
        XCTAssertNil(state.transfer)
    }

    func testFailedTransferReportsAnError() {
        introduce(files: [offer("a.png")])
        state.handle(.sessionEnded(session: 1, outcome: .failed))
        XCTAssertNotNil(state.lastError)
    }

    // MARK: - Trusted devices

    func testTrustedSenderSkipsTheConsentPrompt() {
        state.trust("Pixel 8")
        introduce(sender: "Pixel 8", files: [offer("a.png")])
        XCTAssertEqual(
            state.transfer?.receiving, true,
            "a trusted device should be accepted without asking")
    }

    func testUntrustedSenderStillWaits() {
        state.trust("Someone Else")
        introduce(sender: "Pixel 8", files: [offer("a.png")])
        XCTAssertEqual(state.transfer?.receiving, false)
    }

    func testTrustCanBeRevoked() {
        state.trust("Pixel 8")
        XCTAssertTrue(state.isTrusted("Pixel 8"))
        state.revokeTrust("Pixel 8")
        XCTAssertFalse(state.isTrusted("Pixel 8"))
    }

    func testTrustIsNotDuplicated() {
        state.trust("Pixel 8")
        state.trust("Pixel 8")
        XCTAssertEqual(state.trustedDevices.count, 1)
    }

    // MARK: - Text payloads

    func testReceivedLinkIsRecordedInHistory() {
        introduce(sender: "Pixel 8", files: [])
        state.handle(.textReceived(
            session: 1, kind: "link", description: "A link",
            content: "https://example.com/page"))

        let entry = state.history.first
        XCTAssertEqual(entry?.kind, .link, "links belong in history, not only in a banner")
        XCTAssertEqual(entry?.content, "https://example.com/page")
        XCTAssertEqual(entry?.peer, "Pixel 8")
        XCTAssertEqual(entry?.summary, "https://example.com/page")
        XCTAssertFalse(entry?.isFile ?? true)
    }

    func testReceivedTextIsRecordedAsText() {
        state.handle(.textReceived(
            session: 1, kind: "text", description: "", content: "some notes"))
        XCTAssertEqual(state.history.first?.kind, .text)
    }

    func testTextTransferDoesNotAlsoRecordAnEmptyFileTransfer() {
        introduce(sender: "Pixel 8", files: [])
        state.handle(.textReceived(
            session: 1, kind: "link", description: "", content: "https://example.com"))
        state.handle(.sessionEnded(session: 1, outcome: .completed))

        XCTAssertEqual(state.history.count, 1, "the link is one entry, not two")
        XCTAssertEqual(state.history.first?.kind, .link)
    }

    /// A link with a scheme that cannot be opened is still a first-class
    /// arrival: recorded, and on the clipboard.
    func testUnopenableLinksAreStillReceivedAndCopied() {
        let link = "ssh://someone@example.com"
        introduce(sender: "Pixel 8", files: [])
        state.handle(.textReceived(
            session: 1, kind: "link", description: "", content: link))

        XCTAssertEqual(state.history.first?.kind, .link)
        XCTAssertEqual(state.history.first?.content, link)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), link)
        XCTAssertNil(AppState.webURL(from: link), "but it must not be openable")
    }

    func testReceivedTextGoesToTheClipboard() {
        let unique = "https://example.com/\(UUID().uuidString)"
        state.handle(.textReceived(
            session: 1, kind: "link", description: "A link", content: unique))

        XCTAssertEqual(state.receivedText?.kind, "link")
        XCTAssertEqual(state.receivedText?.content, unique)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), unique)
    }

    // MARK: - Discovery

    func testEndpointsAppearAndAreMarkedAwayRatherThanRemoved() {
        state.handle(.endpointUpdated(
            endpoint: "ep-1", name: "Pixel 8", kind: "phone", present: true))
        XCTAssertEqual(state.endpoints.count, 1)
        XCTAssertEqual(state.endpoints.first?.present, true)

        state.handle(.endpointUpdated(
            endpoint: "ep-1", name: "Pixel 8", kind: "phone", present: false))
        XCTAssertEqual(state.endpoints.count, 1, "a device that left stays listed, dimmed")
        XCTAssertEqual(state.endpoints.first?.present, false)
    }

    func testTheSameDeviceUnderANewIdIsNotDuplicated() {
        // mDNS hands out a fresh instance id per advertisement, so matching on
        // id alone would list one phone several times.
        state.handle(.endpointUpdated(
            endpoint: "ep-1", name: "Pixel 8", kind: "phone", present: true))
        state.handle(.endpointUpdated(
            endpoint: "ep-2", name: "Pixel 8", kind: "phone", present: true))
        XCTAssertEqual(state.endpoints.count, 1)
        XCTAssertEqual(state.endpoints.first?.id, "ep-2")
    }

    func testDeviceKindPicksAnIcon() {
        state.handle(.endpointUpdated(
            endpoint: "ep-1", name: "Tab", kind: "tablet", present: true))
        XCTAssertEqual(state.endpoints.first?.symbolName, "ipad")
    }

    // MARK: - Sending

    func testAdvertisingStateFollowsTheDomain() {
        state.handle(.advertisingChanged(on: true))
        XCTAssertTrue(state.receiving)
        state.handle(.advertisingChanged(on: false))
        XCTAssertFalse(state.receiving)
    }

    func testOutboundConsentThenProgress() {
        state.handle(.sendAwaitingConsent(session: 1 << 63, totalBytes: 500, token: "4821"))
        XCTAssertEqual(state.outbound?.awaitingConsent, true)
        XCTAssertEqual(state.outbound?.totalBytes, 500)
        XCTAssertEqual(
            state.outbound?.token, "4821",
            "the phone shows this code and asks for it to be compared with this screen")

        state.handle(.progress(
            session: 1 << 63, bytesReceived: 250, totalBytes: 500,
            currentFile: "", files: []))
        XCTAssertEqual(state.outbound?.awaitingConsent, false, "bytes mean the phone accepted")
        XCTAssertEqual(state.outbound?.bytesSent, 250)
    }

    func testCompletedSendIsRecordedAndClearsRetry() {
        state.handle(.sendAwaitingConsent(session: 1 << 63, totalBytes: 10, token: "4821"))
        state.handle(.sessionEnded(session: 1 << 63, outcome: .completed))
        XCTAssertNil(state.outbound)
        XCTAssertEqual(state.history.first?.direction, .sent)
        XCTAssertNil(state.lastSend, "a successful send leaves nothing to retry")
    }

    func testDeclinedSendExplainsWhy() {
        state.handle(.sendAwaitingConsent(session: 1 << 63, totalBytes: 10, token: "4821"))
        state.handle(.sessionEnded(session: 1 << 63, outcome: .rejected))
        XCTAssertNotNil(state.lastError)
        XCTAssertTrue(state.history.isEmpty)
    }
}
