import XCTest

@testable import DroidHarbor

/// The share extension and the app talk through a URL, and the two sides are
/// built into different processes. These tests pin the contract between them.
final class ShareRequestTests: XCTestCase {
    func testFilesSurviveTheRoundTrip() throws {
        let files = [
            URL(fileURLWithPath: "/tmp/a photo.jpg"),
            URL(fileURLWithPath: "/tmp/Ünïcode & symbols.pdf"),
        ]
        let url = try XCTUnwrap(ShareRequest.url(for: .files(files)))
        XCTAssertEqual(ShareRequest.parse(url), .files(files), "names must survive encoding")
    }

    func testTextSurvivesTheRoundTrip() throws {
        let url = try XCTUnwrap(ShareRequest.url(for: .text("https://example.com/?a=1&b=2")))
        XCTAssertEqual(ShareRequest.parse(url), .text("https://example.com/?a=1&b=2"))
    }

    func testWhitespaceOnlyTextIsNotARequest() {
        XCTAssertNil(ShareRequest.url(for: .text("   \n ")))
        XCTAssertNil(ShareRequest.url(for: .files([])))
    }

    /// A URL scheme can be opened by anything on the machine, so anything
    /// that is not a well-formed send request has to be ignored.
    func testForeignUrlsAreIgnored() {
        let scheme = ShareRequest.scheme
        for raw in [
            "https://example.com/send?path=/etc/passwd",
            "droidharbor-nope://send?path=/tmp/a.png",
            "\(scheme)://quit",
            "\(scheme)://send",
            "\(scheme)://send?other=1",
        ] {
            XCTAssertNil(ShareRequest.parse(URL(string: raw)!), "\(raw) is not a send request")
        }
    }

    func testFilesWinWhenBothArePresent() throws {
        let url = try XCTUnwrap(
            URL(string: "\(ShareRequest.scheme)://send?text=hello&path=/tmp/a.png"))
        XCTAssertEqual(ShareRequest.parse(url), .files([URL(fileURLWithPath: "/tmp/a.png")]))
    }

    /// The two channels are separate apps and must not answer each other.
    func testTheSchemeFollowsTheBuild() {
        XCTAssertEqual(
            ShareRequest.scheme,
            AppInfo.isDevelopmentBuild ? "droidharbor-dev" : "droidharbor")
    }
}
