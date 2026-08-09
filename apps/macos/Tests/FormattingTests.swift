import XCTest

@testable import DroidHarbor

/// The small pure helpers behind everything the user reads.
final class FormattingTests: XCTestCase {
    func testByteSizes() {
        XCTAssertEqual(Format.bytes(0), "Zero KB")
        XCTAssertTrue(Format.bytes(5_000_000).contains("MB"))
    }

    func testRateAlwaysReadsPerSecond() {
        XCTAssertTrue(Format.rate(1_500_000).hasSuffix("/s"))
    }

    func testNegativeRatesDoNotUnderflow() {
        // The smoothing can briefly go negative; UInt64(-1) would wrap to an
        // absurd figure rather than crash, so it is clamped instead.
        XCTAssertTrue(Format.rate(-10).hasSuffix("/s"))
    }

    func testRemainingIsCoarse() {
        XCTAssertEqual(Format.remaining(3), "a few seconds left")
        XCTAssertEqual(Format.remaining(45), "45 seconds left")
        XCTAssertEqual(Format.remaining(62), "about a minute left")
        XCTAssertEqual(Format.remaining(300), "about 5 minutes left")
    }
}

final class TransferRateTests: XCTestCase {
    func testFirstSampleCannotEstimate() {
        var rate = TransferRate()
        rate.sample(bytes: 1000, total: 10_000)
        XCTAssertEqual(rate.bytesPerSecond, 0, "a single point is not a rate")
        XCTAssertNil(rate.secondsRemaining)
    }

    func testRateAppearsAfterTimePasses() {
        var rate = TransferRate()
        rate.sample(bytes: 0, total: 10_000)
        Thread.sleep(forTimeInterval: 0.2)
        rate.sample(bytes: 2_000, total: 10_000)
        XCTAssertGreaterThan(rate.bytesPerSecond, 0)
        XCTAssertNotNil(rate.secondsRemaining)
    }

    /// A fast transfer reports far more often than the sampling window. The
    /// window has to accumulate across those samples: when each one reset the
    /// clock, the rate stayed at zero for the whole transfer and neither the
    /// speed nor the time remaining ever appeared.
    func testRapidSamplesStillProduceARate() {
        var rate = TransferRate()
        rate.sample(bytes: 0, total: 1_000_000)
        for step in 1...20 {
            Thread.sleep(forTimeInterval: 0.02)
            rate.sample(bytes: UInt64(step * 10_000), total: 1_000_000)
        }
        XCTAssertGreaterThan(rate.bytesPerSecond, 0, "20 samples over 0.4s is a rate")
        XCTAssertNotNil(rate.secondsRemaining)
    }

    func testGoingBackwardsIsIgnored() {
        // A restarted or re-reported transfer must not produce a wild figure.
        var rate = TransferRate()
        rate.sample(bytes: 5_000, total: 10_000)
        Thread.sleep(forTimeInterval: 0.2)
        rate.sample(bytes: 1_000, total: 10_000)
        XCTAssertEqual(rate.bytesPerSecond, 0)
    }
}

final class UpdateCheckerTests: XCTestCase {
    func testNewerVersionsAreDetected() {
        XCTAssertTrue(UpdateChecker.isNewer("1.0.1", than: "1.0.0"))
        XCTAssertTrue(UpdateChecker.isNewer("1.1.0", than: "1.0.9"))
        XCTAssertTrue(UpdateChecker.isNewer("2.0.0", than: "1.99.99"))
    }

    func testComparisonIsNumericNotLexical() {
        // The classic bug: "1.10.0" sorts before "1.9.0" as text.
        XCTAssertTrue(UpdateChecker.isNewer("1.10.0", than: "1.9.3"))
        XCTAssertFalse(UpdateChecker.isNewer("1.9.3", than: "1.10.0"))
    }

    func testSameOrOlderIsNotAnUpdate() {
        XCTAssertFalse(UpdateChecker.isNewer("1.2.3", than: "1.2.3"))
        XCTAssertFalse(UpdateChecker.isNewer("1.2.2", than: "1.2.3"))
    }

    func testShorterVersionsAreTreatedAsZeroPadded() {
        XCTAssertFalse(UpdateChecker.isNewer("1.2", than: "1.2.0"))
        XCTAssertTrue(UpdateChecker.isNewer("1.3", than: "1.2.9"))
    }
}
