import Foundation

/// Transfer rate and remaining time, derived from progress samples.
struct TransferRate {
    var bytesPerSecond: Double = 0
    var secondsRemaining: Double?

    private var lastBytes: UInt64 = 0
    private var lastAt: Date?

    /// Feed a progress sample; smoothed so the figure does not jump.
    ///
    /// Samples closer together than the window are skipped, but the baseline
    /// is deliberately *not* moved when that happens: a fast transfer reports
    /// every few milliseconds, and resetting the clock each time meant the
    /// window never elapsed and the rate stayed at zero for the whole
    /// transfer.
    mutating func sample(bytes: UInt64, total: UInt64) {
        let now = Date()
        guard let lastAt, bytes >= lastBytes else {
            // First sample, or a transfer that restarted: rebase quietly
            // rather than report a rate from a bogus delta.
            lastBytes = bytes
            self.lastAt = now
            return
        }
        let elapsed = now.timeIntervalSince(lastAt)
        guard elapsed > 0.15 else { return }
        defer { lastBytes = bytes; self.lastAt = now }
        let instant = Double(bytes - lastBytes) / elapsed
        bytesPerSecond = bytesPerSecond == 0 ? instant : bytesPerSecond * 0.7 + instant * 0.3
        if total > bytes, bytesPerSecond > 1 {
            secondsRemaining = Double(total - bytes) / bytesPerSecond
        } else {
            secondsRemaining = nil
        }
    }
}
