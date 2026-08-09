//! Filename sanitization for sender-supplied names.
//!
//! Senders are untrusted: names may contain path separators, traversal
//! components, control characters, or be absurdly long. Sanitization always
//! yields a usable name rather than failing: a weird name must not abort a
//! transfer that already moved gigabytes.

use unicode_normalization::UnicodeNormalization;

/// Maximum filename length in bytes (common denominator for APFS/ext4/btrfs).
const MAX_NAME_BYTES: usize = 255;

/// Fallback when nothing usable survives sanitization.
const FALLBACK_NAME: &str = "unnamed";

/// Sanitize a sender-supplied filename into a single safe path component.
///
/// Steps: take the final path component (drops traversal and absolute
/// prefixes), NFC-normalize, strip control characters, trim surrounding
/// whitespace, reject `.` / `..` / empty, and cap the byte length while
/// preserving a short extension.
pub fn sanitize_file_name(raw: &str) -> String {
    // Final component only: handles both `/` and `\` separators.
    let base = raw
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or("");

    let cleaned: String = base
        .nfc()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string();

    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return FALLBACK_NAME.to_string();
    }

    truncate_preserving_extension(&cleaned, MAX_NAME_BYTES)
}

/// Truncate `name` to at most `max_bytes` bytes on a char boundary, keeping
/// the extension when it is reasonably short.
fn truncate_preserving_extension(name: &str, max_bytes: usize) -> String {
    if name.len() <= max_bytes {
        return name.to_string();
    }

    let (stem, ext) = match name.rfind('.') {
        // Keep extensions up to 32 bytes (covers e.g. `.tar.zst` pieces).
        Some(idx) if idx > 0 && name.len() - idx <= 32 => name.split_at(idx),
        _ => (name, ""),
    };

    let budget = max_bytes.saturating_sub(ext.len()).max(1);
    let mut cut = budget.min(stem.len());
    while cut > 0 && !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut result = String::with_capacity(cut + ext.len());
    result.push_str(&stem[..cut]);
    result.push_str(ext);
    if result.is_empty() {
        FALLBACK_NAME.to_string()
    } else {
        result
    }
}

/// Split a filename into `(stem, extension)` where the extension includes the
/// leading dot, or is empty. `archive.tar.gz` → `("archive.tar", ".gz")`.
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(idx) if idx > 0 => name.split_at(idx),
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_pass_through() {
        assert_eq!(sanitize_file_name("IMG_20260806.jpg"), "IMG_20260806.jpg");
        assert_eq!(sanitize_file_name("résumé.pdf"), "résumé.pdf");
    }

    #[test]
    fn traversal_is_reduced_to_basename() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_file_name("/etc/shadow"), "shadow");
        assert_eq!(sanitize_file_name("..\\..\\windows\\evil.exe"), "evil.exe");
    }

    #[test]
    fn dot_components_fall_back() {
        assert_eq!(sanitize_file_name(".."), FALLBACK_NAME);
        assert_eq!(sanitize_file_name("."), FALLBACK_NAME);
        assert_eq!(sanitize_file_name(""), FALLBACK_NAME);
        assert_eq!(sanitize_file_name("///"), FALLBACK_NAME);
    }

    #[test]
    fn control_characters_are_stripped() {
        assert_eq!(sanitize_file_name("bad\u{0}name\n.txt"), "badname.txt");
    }

    #[test]
    fn whitespace_is_trimmed() {
        assert_eq!(sanitize_file_name("  spaced.txt  "), "spaced.txt");
    }

    #[test]
    fn long_names_keep_extension() {
        let long = format!("{}.jpg", "a".repeat(300));
        let out = sanitize_file_name(&long);
        assert!(out.len() <= MAX_NAME_BYTES);
        assert!(out.ends_with(".jpg"));
    }

    #[test]
    fn long_multibyte_names_cut_on_char_boundary() {
        let long = "é".repeat(200); // 400 bytes
        let out = sanitize_file_name(&long);
        assert!(out.len() <= MAX_NAME_BYTES);
        assert!(out.chars().all(|c| c == 'é'));
    }

    #[test]
    fn split_extension_cases() {
        assert_eq!(split_extension("a.txt"), ("a", ".txt"));
        assert_eq!(split_extension("archive.tar.gz"), ("archive.tar", ".gz"));
        assert_eq!(split_extension("noext"), ("noext", ""));
        assert_eq!(split_extension(".hidden"), (".hidden", ""));
    }
}
