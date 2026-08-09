import AppKit
import XCTest

@testable import DroidHarbor

/// Regression tests for the two bugs that lost or broke received files.
@MainActor
final class FileHandlingTests: XCTestCase {
    private func makeState() -> AppState {
        let suite = UserDefaults(suiteName: "dh-tests-\(UUID().uuidString)")!
        return AppState(defaults: suite, startService: false)
    }

    /// UNNotificationAttachment moves the file it is handed into the
    /// notification store. Attaching the received file itself deleted it from
    /// the user's folder seconds after it landed; the preview must be a copy.
    func testNotificationPreviewDoesNotConsumeTheOriginal() throws {
        let state = makeState()
        let dir = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let original = dir.appendingPathComponent("photo.png")
        try Data("not really a png".utf8).write(to: original)

        let copy = try XCTUnwrap(state.temporaryCopy(of: original.path))

        XCTAssertNotEqual(copy.path, original.path, "the preview must be a separate file")
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: original.path),
            "the received file must survive being previewed")
        XCTAssertEqual(try Data(contentsOf: copy), try Data(contentsOf: original))
    }

    func testTemporaryCopyOfAMissingFileFails() {
        let state = makeState()
        XCTAssertNil(state.temporaryCopy(of: "/nope/missing.png"))
    }

    /// Quarantine flag 0x02 marks a file as belonging to a sandboxed
    /// container; setting it made every later reveal and open fail with
    /// "sandbox extension creation failed".
    func testQuarantineDoesNotSetTheSandboxFlag() throws {
        let file = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("\(UUID().uuidString).bin")
        try Data("x".utf8).write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }

        Quarantine.mark(path: file.path)

        let value = try XCTUnwrap(readQuarantine(file.path), "the attribute should be set")
        let flags = try XCTUnwrap(value.split(separator: ";").first)
        let bits = try XCTUnwrap(UInt32(flags, radix: 16))
        XCTAssertEqual(bits & 0x0002, 0, "QTN_FLAG_SANDBOX must not be set")
        XCTAssertEqual(bits & 0x0001, 1, "the file should still be marked as downloaded")
    }

    private func readQuarantine(_ path: String) -> String? {
        let name = "com.apple.quarantine"
        let length = getxattr(path, name, nil, 0, 0, 0)
        guard length > 0 else { return nil }
        var buffer = [UInt8](repeating: 0, count: length)
        guard getxattr(path, name, &buffer, length, 0, 0) == length else { return nil }
        return String(bytes: buffer, encoding: .utf8)
    }
}

/// Received content is untrusted: it arrives from another device.
@MainActor
final class LinkSafetyTests: XCTestCase {
    func testWebLinksAreAccepted() {
        XCTAssertNotNil(AppState.webURL(from: "https://example.com/page"))
        XCTAssertNotNil(AppState.webURL(from: "http://example.com"))
        XCTAssertNotNil(AppState.webURL(from: "HTTPS://Example.com"))
    }

    func testSchemesThatCouldLaunchSomethingElseAreRefused() {
        for link in [
            "file:///Applications/Calculator.app",
            "ssh://someone@example.com",
            "smb://server/share",
            "ftp://example.com",
            "vnc://example.com",
            "x-apple-shortcuts://run-shortcut?name=wipe",
            "javascript:alert(1)",
            "some-installed-app://do-something",
        ] {
            XCTAssertNil(AppState.webURL(from: link), "\(link) must not be opened")
        }
    }

    func testMalformedLinksAreRefused() {
        XCTAssertNil(AppState.webURL(from: ""))
        XCTAssertNil(AppState.webURL(from: "not a link"))
        XCTAssertNil(AppState.webURL(from: "example.com"), "no scheme, not a link")
        XCTAssertNil(AppState.webURL(from: "https://"), "no host")
    }

    func testOpeningARefusedLinkReportsWhy() {
        let suite = UserDefaults(suiteName: "dh-link-\(UUID().uuidString)")!
        let state = AppState(defaults: suite, startService: false)
        state.openLink("file:///etc/passwd")
        XCTAssertNotNil(state.lastError)
    }
}

@MainActor
final class HistoryStoreTests: XCTestCase {
    /// Preferences pointed at a scratch file, so tests never touch the real
    /// ~/.droidharbor/history.json.
    private func suite(file: URL? = nil) -> UserDefaults {
        let defaults = UserDefaults(suiteName: "dh-history-\(UUID().uuidString)")!
        let url = file ?? URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("dh-\(UUID().uuidString)/history.json")
        defaults.set(url.path, forKey: HistoryStore.locationKey)
        return defaults
    }

    private func entry(_ name: String) -> HistoryEntry {
        HistoryEntry(direction: .received, peer: "Pixel", paths: ["/tmp/\(name)"])
    }

    func testRoundTrip() {
        let defaults = suite()
        HistoryStore.save([entry("a.png"), entry("b.png")], to: defaults)
        let loaded = HistoryStore.load(from: defaults)
        XCTAssertEqual(loaded.count, 2)
        XCTAssertEqual(loaded.first?.primaryName, "a.png")
    }

    func testEmptyStorageLoadsEmpty() {
        XCTAssertTrue(HistoryStore.load(from: suite()).isEmpty)
    }

    func testLinkEntriesSurviveARoundTrip() {
        let defaults = suite()
        let link = HistoryEntry(
            direction: .received, peer: "Pixel",
            content: "https://example.com", kind: .link)
        HistoryStore.save([link], to: defaults)

        let loaded = HistoryStore.load(from: defaults).first
        XCTAssertEqual(loaded?.kind, .link)
        XCTAssertEqual(loaded?.content, "https://example.com")
        XCTAssertEqual(loaded?.summary, "https://example.com")
    }

    /// History written before links were recorded has neither field, and its
    /// dates were bare numbers; it must still load rather than throwing the
    /// whole list away.
    func testEntriesFromOlderBuildsAreMigratedOutOfPreferences() throws {
        let defaults = suite()
        let legacy = """
            [{"id":"\(UUID().uuidString)","direction":"received","peer":"Pixel",
              "date":760000000,"paths":["/tmp/a.png"]}]
            """
        defaults.set(Data(legacy.utf8), forKey: "history")

        let loaded = HistoryStore.load(from: defaults)
        XCTAssertEqual(loaded.count, 1)
        XCTAssertEqual(loaded.first?.kind, .files)
        XCTAssertEqual(loaded.first?.primaryName, "a.png")

        XCTAssertNil(
            defaults.data(forKey: "history"),
            "the old preference should be cleared once migrated")
        let file = HistoryStore.fileURL(for: defaults)
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: file.path),
            "and rewritten as a file")
    }

    func testHistoryIsWrittenAsReadableJSON() throws {
        let defaults = suite()
        HistoryStore.save([entry("a.png")], to: defaults)

        let url = HistoryStore.fileURL(for: defaults)
        let data = try Data(contentsOf: url)

        // Valid JSON, and an array at the top level.
        let parsed = try JSONSerialization.jsonObject(with: data)
        let array = try XCTUnwrap(parsed as? [[String: Any]])
        XCTAssertEqual(array.count, 1)
        XCTAssertEqual(array.first?["kind"] as? String, "files")
        XCTAssertEqual(array.first?["peer"] as? String, "Pixel")

        let text = try XCTUnwrap(String(data: data, encoding: .utf8))
        XCTAssertTrue(text.contains("\n"), "indented rather than one long line")
        XCTAssertFalse(text.contains("\\/"), "paths should not be escaped")
        // Dates are timestamps a person can read, not a float.
        let date = try XCTUnwrap(array.first?["date"] as? String)
        XCTAssertTrue(date.contains("T"), "expected an ISO-8601 date, got \(date)")
    }

    func testTheLocationIsRecordedInPreferences() {
        let defaults = UserDefaults(suiteName: "dh-loc-\(UUID().uuidString)")!
        XCTAssertNil(defaults.string(forKey: HistoryStore.locationKey))

        let url = HistoryStore.fileURL(for: defaults)

        XCTAssertEqual(defaults.string(forKey: HistoryStore.locationKey), url.path)
        XCTAssertEqual(url.lastPathComponent, "history.json")
        XCTAssertEqual(
            url.deletingLastPathComponent().lastPathComponent, AppInfo.stateDirectoryName,
            "history belongs in the home folder this build owns")
    }

    /// A development build must not write into the real history, or testing
    /// quietly edits the record of what a person actually received.
    func testDevelopmentBuildsKeepTheirOwnHistory() {
        XCTAssertEqual(
            AppInfo.stateDirectoryName,
            AppInfo.isDevelopmentBuild ? ".droidharbor-dev" : ".droidharbor")
    }

    func testSummaryCountsTheRest() {
        let one = HistoryEntry(direction: .received, peer: "Pixel", paths: ["/tmp/a.png"])
        let three = HistoryEntry(
            direction: .received, peer: "Pixel",
            paths: ["/tmp/a.png", "/tmp/b.png", "/tmp/c.png"])
        XCTAssertEqual(one.summary, "a.png")
        XCTAssertEqual(three.summary, "a.png and 2 more")
    }
}

/// Categorising and searching the history: the two things the list's filter
/// and search field rely on.
@MainActor
final class HistoryFilteringTests: XCTestCase {
    private func file(_ path: String) -> HistoryEntry {
        HistoryEntry(direction: .received, peer: "Pixel 8", paths: [path])
    }

    private func link(_ url: String) -> HistoryEntry {
        HistoryEntry(direction: .received, peer: "Pixel 8", content: url, kind: .link)
    }

    func testFilesAreCategorisedByExtension() {
        XCTAssertEqual(file("/tmp/IMG_1.HEIC").category, .image)
        XCTAssertEqual(file("/tmp/clip.mp4").category, .video)
        XCTAssertEqual(file("/tmp/song.flac").category, .audio)
        XCTAssertEqual(file("/tmp/report.pdf").category, .document)
        XCTAssertEqual(file("/tmp/app.apk").category, .other)
        XCTAssertEqual(file("/tmp/noextension").category, .other)
    }

    func testLinksAndTextHaveTheirOwnCategories() {
        XCTAssertEqual(link("https://example.com").category, .link)
        XCTAssertEqual(
            HistoryEntry(direction: .received, peer: "P", content: "hi", kind: .text).category,
            .other)
    }

    func testSearchMatchesWholeAndPartialNames() {
        let entry = file("/tmp/Screenshot_20260807-120552.png")
        XCTAssertTrue(entry.matches("screenshot"))
        XCTAssertTrue(entry.matches("120552"))
        XCTAssertTrue(entry.matches("SCREENSHOT"), "search is case-insensitive")
        XCTAssertFalse(entry.matches("invoice"))
    }

    func testSearchMatchesExtensions() {
        let entry = file("/tmp/photo.png")
        XCTAssertTrue(entry.matches("png"))
        XCTAssertTrue(entry.matches(".png"), "a leading dot reads as an extension")
        XCTAssertFalse(entry.matches("jpg"))
    }

    func testSearchMatchesLinksByHostAndScheme() {
        let entry = link("https://www.lanacion.com.ar/article")
        XCTAssertTrue(entry.matches("lanacion"))
        XCTAssertTrue(entry.matches("https"))
        XCTAssertTrue(entry.matches("article"))
        XCTAssertFalse(entry.matches("ftp"))
    }

    func testSearchMatchesTheSender() {
        XCTAssertTrue(file("/tmp/a.png").matches("pixel"))
    }

    func testEmptySearchMatchesEverything() {
        XCTAssertTrue(file("/tmp/a.png").matches(""))
        XCTAssertTrue(file("/tmp/a.png").matches("   "))
    }

    /// The user asked for the whole record to be kept.
    func testHistoryIsNotTruncated() {
        let defaults = UserDefaults(suiteName: "dh-cap-\(UUID().uuidString)")!
        defaults.set(
            URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("dh-\(UUID().uuidString)/history.json").path,
            forKey: HistoryStore.locationKey)

        let many = (0..<250).map { file("/tmp/f\($0).png") }
        HistoryStore.save(many, to: defaults)

        XCTAssertEqual(HistoryStore.load(from: defaults).count, 250)
    }
}

/// Recovering meaning from payloads the protocol only labels as "file" or
/// "text": contacts, events, numbers, addresses and places.
@MainActor
final class PayloadClassifierTests: XCTestCase {
    func testContactsAndEventsAreRecognisedByExtension() {
        XCTAssertEqual(PayloadClassifier.kind(forFile: "/tmp/Jane.vcf"), .contact)
        XCTAssertEqual(PayloadClassifier.kind(forFile: "/tmp/Jane.VCARD"), .contact)
        XCTAssertEqual(PayloadClassifier.kind(forFile: "/tmp/party.ics"), .calendar)
        XCTAssertEqual(PayloadClassifier.kind(forFile: "/tmp/photo.png"), .files)
    }

    /// Android sometimes hands over a card with no useful extension.
    func testContactsAreRecognisedByContentWhenTheExtensionIsUseless() throws {
        let file = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("\(UUID().uuidString).bin")
        try Data("BEGIN:VCARD\nVERSION:3.0\nFN:Jane\nEND:VCARD".utf8).write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }

        XCTAssertEqual(PayloadClassifier.kind(forFile: file.path), .contact)
    }

    func testTextIsClassifiedByWhatItActuallyIs() {
        func kind(_ text: String, _ declared: String = "text") -> HistoryEntry.Kind {
            PayloadClassifier.kind(forText: text, declared: declared)
        }
        XCTAssertEqual(kind("tel:+34 600 123 456"), .phone)
        XCTAssertEqual(kind("+34 600 123 456"), .phone)
        XCTAssertEqual(kind("mailto:jane@example.com"), .email)
        XCTAssertEqual(kind("jane@example.com"), .email)
        XCTAssertEqual(kind("geo:41.3874,2.1686"), .map)
        XCTAssertEqual(kind("https://maps.apple.com/?q=Sagrada"), .map)
        XCTAssertEqual(kind("BEGIN:VCARD\nFN:Jane\nEND:VCARD"), .contact)
        XCTAssertEqual(kind("BEGIN:VCALENDAR\nBEGIN:VEVENT"), .calendar)
        XCTAssertEqual(kind("https://example.com", "link"), .link)
        XCTAssertEqual(kind("just some notes"), .text)
    }

    /// Codes, IDs and prices must not be offered as phone numbers.
    func testAmbiguousNumbersAreNotTreatedAsPhoneNumbers() {
        XCTAssertFalse(PayloadClassifier.isPhoneNumber("1234567"))
        XCTAssertFalse(PayloadClassifier.isPhoneNumber("12345678901234567890"))
        XCTAssertFalse(PayloadClassifier.isPhoneNumber("order 12345 shipped"))
        XCTAssertTrue(PayloadClassifier.isPhoneNumber("+441234567890"))
        XCTAssertTrue(PayloadClassifier.isPhoneNumber("600 123 456"))
    }

    func testEmailRecognitionRejectsNearMisses() {
        XCTAssertTrue(PayloadClassifier.isEmailAddress("jane.doe+tag@example.co.uk"))
        XCTAssertFalse(PayloadClassifier.isEmailAddress("jane@"))
        XCTAssertFalse(PayloadClassifier.isEmailAddress("write to jane@example.com"))
        XCTAssertFalse(PayloadClassifier.isEmailAddress("@example.com"))
    }

    func testSchemesAreStrippedBeforeUse() {
        XCTAssertEqual(
            PayloadClassifier.value(from: "tel:+34%20600", strippingScheme: "tel"), "+34 600")
        XCTAssertEqual(
            PayloadClassifier.value(from: "jane@example.com", strippingScheme: "mailto"),
            "jane@example.com")
    }

    func testKindDrivesTheFilterCategory() {
        XCTAssertEqual(
            HistoryEntry(direction: .received, peer: "P", paths: ["/tmp/a.vcf"], kind: .contact)
                .category, .contact)
        XCTAssertEqual(
            HistoryEntry(direction: .received, peer: "P", content: "tel:+34600", kind: .phone)
                .category, .phone)
    }
}
