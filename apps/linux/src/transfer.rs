//! The transfer currently on screen, in either direction.
//!
//! Mirrors the macOS `ActiveTransfer`: the card shows who is sending, what,
//! how far along it is, and — once there is enough evidence — how fast and how
//! much longer.

use std::time::{Duration, Instant};

use dh_domain::SessionId;

/// Rate is smoothed over a window rather than computed from the last two
/// samples: raw deltas swing wildly on a wireless link and produce an estimate
/// that flickers between "12 seconds" and "four minutes".
const RATE_WINDOW: Duration = Duration::from_secs(5);
/// Below this, an estimate is guesswork and showing one is worse than silence.
const MIN_SAMPLE: Duration = Duration::from_millis(1200);

#[derive(Debug, Clone)]
pub struct FileLine {
    pub name: String,
    pub transferred: u64,
    pub size: u64,
    pub completed: bool,
}

impl FileLine {
    pub fn fraction(&self) -> f64 {
        if self.completed {
            return 1.0;
        }
        match self.size {
            0 => 0.0,
            size => (self.transferred as f64 / size as f64).clamp(0.0, 1.0),
        }
    }
}

/// A point in the progress stream, kept to derive speed.
#[derive(Debug, Clone, Copy)]
struct Sample {
    at: Instant,
    bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Active {
    pub session: SessionId,
    pub peer: String,
    /// False while consent is still pending; true once bytes are moving.
    pub running: bool,
    /// True for outbound transfers, where the phone's user accepts.
    pub outgoing: bool,
    pub token: String,
    pub text_preview: Option<String>,
    pub bytes: u64,
    pub total_bytes: u64,
    pub current_file: String,
    pub files: Vec<FileLine>,
    samples: Vec<Sample>,
}

impl Active {
    pub fn incoming(
        session: SessionId,
        peer: String,
        token: String,
        total_bytes: u64,
        text_preview: Option<String>,
        files: Vec<FileLine>,
    ) -> Self {
        Active {
            session,
            peer,
            running: false,
            outgoing: false,
            token,
            text_preview,
            bytes: 0,
            total_bytes,
            current_file: String::new(),
            files,
            samples: Vec::new(),
        }
    }

    pub fn outgoing(session: SessionId, peer: String, token: String, total_bytes: u64) -> Self {
        Active {
            session,
            peer,
            running: false,
            outgoing: true,
            token,
            text_preview: None,
            bytes: 0,
            total_bytes,
            current_file: String::new(),
            files: Vec::new(),
            samples: Vec::new(),
        }
    }

    pub fn fraction(&self) -> f64 {
        match self.total_bytes {
            0 => 0.0,
            total => (self.bytes as f64 / total as f64).clamp(0.0, 1.0),
        }
    }

    pub fn record_progress(&mut self, bytes: u64, total_bytes: u64, now: Instant) {
        self.running = true;
        self.bytes = bytes;
        if total_bytes > 0 {
            self.total_bytes = total_bytes;
        }
        self.samples.push(Sample { at: now, bytes });
        self.samples
            .retain(|sample| now.duration_since(sample.at) <= RATE_WINDOW);
    }

    /// Bytes per second, or `None` until the window holds enough to mean
    /// something.
    pub fn rate(&self) -> Option<f64> {
        let first = self.samples.first()?;
        let last = self.samples.last()?;
        let elapsed = last.at.duration_since(first.at);
        if elapsed < MIN_SAMPLE {
            return None;
        }
        let moved = last.bytes.checked_sub(first.bytes)?;
        if moved == 0 {
            return None;
        }
        Some(moved as f64 / elapsed.as_secs_f64())
    }

    pub fn seconds_remaining(&self) -> Option<f64> {
        let rate = self.rate()?;
        let left = self.total_bytes.checked_sub(self.bytes)?;
        if left == 0 {
            return None;
        }
        Some(left as f64 / rate)
    }

    /// The line under the title: either the text preview, or a file count and
    /// a size.
    pub fn summary(&self) -> String {
        if let Some(preview) = &self.text_preview {
            return if preview.is_empty() {
                "A link or text".to_string()
            } else {
                preview.clone()
            };
        }
        let size = crate::format::bytes(self.total_bytes);
        match self.files.len() {
            0 | 1 => format!("1 file · {size}"),
            count => format!("{count} files · {size}"),
        }
    }

    pub fn title(&self) -> String {
        if self.outgoing {
            return format!("Sending to “{}”", self.peer);
        }
        if self.running {
            format!("Receiving from “{}”", self.peer)
        } else {
            format!("“{}” wants to send files", self.peer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active() -> Active {
        Active::incoming(
            SessionId(1),
            "Pixel 8".into(),
            "4821".into(),
            1_000_000,
            None,
            vec![],
        )
    }

    #[test]
    fn rate_needs_a_wide_enough_window() {
        let mut transfer = active();
        let start = Instant::now();
        transfer.record_progress(0, 1_000_000, start);
        // Two samples a few milliseconds apart say nothing useful.
        transfer.record_progress(50_000, 1_000_000, start + Duration::from_millis(100));
        assert_eq!(transfer.rate(), None);
        assert_eq!(transfer.seconds_remaining(), None);

        transfer.record_progress(200_000, 1_000_000, start + Duration::from_secs(2));
        let rate = transfer.rate().expect("enough samples now");
        assert!((rate - 100_000.0).abs() < 1.0, "200 KB over 2 s");
        let left = transfer.seconds_remaining().expect("estimate available");
        assert!((left - 8.0).abs() < 0.1, "800 KB left at 100 KB/s");
    }

    #[test]
    fn stalled_transfer_reports_no_rate() {
        let mut transfer = active();
        let start = Instant::now();
        transfer.record_progress(500_000, 1_000_000, start);
        transfer.record_progress(500_000, 1_000_000, start + Duration::from_secs(3));
        assert_eq!(transfer.rate(), None, "no bytes moved");
    }

    #[test]
    fn old_samples_leave_the_window() {
        let mut transfer = active();
        let start = Instant::now();
        transfer.record_progress(0, 1_000_000, start);
        transfer.record_progress(900_000, 1_000_000, start + Duration::from_secs(30));
        // Only the newest sample survives, so there is nothing to measure yet.
        assert_eq!(transfer.rate(), None);
    }

    #[test]
    fn fraction_is_clamped() {
        let mut transfer = active();
        transfer.record_progress(2_000_000, 1_000_000, Instant::now());
        assert_eq!(transfer.fraction(), 1.0);

        let empty = Active::outgoing(SessionId(2), "Pixel 8".into(), String::new(), 0);
        assert_eq!(
            empty.fraction(),
            0.0,
            "unknown size must not divide by zero"
        );
    }

    #[test]
    fn titles_track_state() {
        let mut transfer = active();
        assert_eq!(transfer.title(), "“Pixel 8” wants to send files");
        transfer.record_progress(1, 1_000_000, Instant::now());
        assert_eq!(transfer.title(), "Receiving from “Pixel 8”");
    }
}
