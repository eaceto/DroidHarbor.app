//! Human-readable sizes, rates and durations, matching what the macOS app
//! shows so the two read the same way.

/// Sizes in the units people expect from a file manager.
pub fn bytes(count: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = count as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{count} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn rate(bytes_per_second: f64) -> String {
    if bytes_per_second <= 0.0 {
        return String::new();
    }
    format!("{}/s", bytes(bytes_per_second as u64))
}

/// Coarse on purpose: a countdown that ticks every second draws the eye away
/// from the transfer it is describing.
pub fn remaining(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    match seconds {
        0..=1 => "a moment left".to_string(),
        2..=59 => format!("{seconds}s left"),
        60..=3599 => {
            let minutes = seconds / 60;
            format!("{minutes} min left")
        }
        _ => {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            if minutes == 0 {
                format!("{hours} h left")
            } else {
                format!("{hours} h {minutes} min left")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_scale() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1024), "1.0 KB");
        assert_eq!(bytes(1024 * 1024), "1.0 MB");
        // Three significant figures would be noise at this size.
        assert_eq!(bytes(500 * 1024 * 1024), "500 MB");
    }

    #[test]
    fn rate_is_empty_when_unknown() {
        assert_eq!(rate(0.0), "");
        assert_eq!(rate(-1.0), "");
        assert_eq!(rate(2048.0), "2.0 KB/s");
    }

    #[test]
    fn remaining_reads_naturally() {
        assert_eq!(remaining(0.4), "a moment left");
        assert_eq!(remaining(45.0), "45s left");
        assert_eq!(remaining(90.0), "1 min left");
        assert_eq!(remaining(3600.0), "1 h left");
        assert_eq!(remaining(5400.0), "1 h 30 min left");
    }
}
