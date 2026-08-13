//! `file://` URI → path, because the XDG portal hands back URIs and
//! `dh-domain` takes absolute paths.
//!
//! `ashpd::Uri` is a newtype over `String` with no decoding of its own, so this
//! is ours to do. Decoding happens at the byte level rather than through
//! `String`: Linux paths are arbitrary bytes, not UTF-8, and a filename that is
//! not valid UTF-8 must still round-trip rather than being rejected or
//! lossily replaced.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
    /// Not a `file://` URI. Portals can return `trash://`, `smb://`, or
    /// similar for locations we cannot hand to the domain as a path.
    NotFileScheme(String),
    /// `file://host/path` pointing at another machine.
    RemoteHost(String),
    /// A `%` escape that is truncated or not hexadecimal.
    BadPercentEncoding(String),
}

impl std::fmt::Display for UriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UriError::NotFileScheme(uri) => {
                write!(f, "not a file:// URI: {uri}")
            }
            UriError::RemoteHost(host) => {
                write!(f, "refers to another machine: {host}")
            }
            UriError::BadPercentEncoding(uri) => {
                write!(f, "malformed percent-encoding: {uri}")
            }
        }
    }
}

impl std::error::Error for UriError {}

/// Convert a `file://` URI into an absolute path.
///
/// Accepts an empty host (`file:///srv`) and `localhost`, which are the two
/// spellings portals actually emit for local files.
pub fn file_uri_to_path(uri: &str) -> Result<PathBuf, UriError> {
    let rest = uri
        .split_once("://")
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
        .map(|(_, rest)| rest)
        .ok_or_else(|| UriError::NotFileScheme(uri.to_string()))?;

    // Everything before the first '/' is the authority; the '/' itself starts
    // the path and must be kept.
    let (host, encoded_path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        // `file://foo` with no path at all is a host, not a relative path.
        None => (rest, ""),
    };

    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return Err(UriError::RemoteHost(host.to_string()));
    }
    if encoded_path.is_empty() {
        return Err(UriError::NotFileScheme(uri.to_string()));
    }

    let bytes = percent_decode(encoded_path)
        .ok_or_else(|| UriError::BadPercentEncoding(uri.to_string()))?;
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}

/// Percent-decode to bytes. `None` on a truncated or non-hex escape, which is
/// worth surfacing rather than passing through as a literal `%`.
fn percent_decode(input: &str) -> Option<Vec<u8>> {
    let raw = input.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;

    while index < raw.len() {
        match raw[index] {
            b'%' => {
                let hex = raw.get(index + 1..index + 3)?;
                let high = (hex[0] as char).to_digit(16)?;
                let low = (hex[1] as char).to_digit(16)?;
                out.push((high * 16 + low) as u8);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_path() {
        assert_eq!(
            file_uri_to_path("file:///home/kimi/Downloads").unwrap(),
            PathBuf::from("/home/kimi/Downloads")
        );
    }

    #[test]
    fn localhost_host_is_local() {
        assert_eq!(
            file_uri_to_path("file://localhost/home/kimi").unwrap(),
            PathBuf::from("/home/kimi")
        );
    }

    #[test]
    fn decodes_spaces_and_accents() {
        // The exact case that bites first: a folder with a space in it.
        assert_eq!(
            file_uri_to_path("file:///home/kimi/My%20Files").unwrap(),
            PathBuf::from("/home/kimi/My Files")
        );
        // Multi-byte UTF-8 arrives as several escapes and must recombine.
        assert_eq!(
            file_uri_to_path("file:///home/kimi/Fotograf%C3%ADas").unwrap(),
            PathBuf::from("/home/kimi/Fotografías")
        );
    }

    #[test]
    fn decodes_percent_and_hash_literals() {
        assert_eq!(
            file_uri_to_path("file:///tmp/100%25%20done%23final").unwrap(),
            PathBuf::from("/tmp/100% done#final")
        );
    }

    #[test]
    fn keeps_non_utf8_bytes() {
        // %FF is not valid UTF-8. The file exists on disk under that name, so
        // decoding must preserve the byte instead of rejecting or replacing it.
        let path = file_uri_to_path("file:///tmp/caf%FF").unwrap();
        assert_eq!(path.as_os_str().as_encoded_bytes(), b"/tmp/caf\xff");
    }

    #[test]
    fn trailing_slash_survives() {
        assert_eq!(
            file_uri_to_path("file:///home/kimi/Downloads/").unwrap(),
            PathBuf::from("/home/kimi/Downloads/")
        );
    }

    #[test]
    fn rejects_other_schemes() {
        for uri in ["trash:///foo", "smb://server/share", "http://example.com/x"] {
            assert!(matches!(
                file_uri_to_path(uri),
                Err(UriError::NotFileScheme(_))
            ));
        }
    }

    #[test]
    fn rejects_remote_host() {
        assert_eq!(
            file_uri_to_path("file://nas.local/volume1/media"),
            Err(UriError::RemoteHost("nas.local".into()))
        );
    }

    #[test]
    fn rejects_malformed_escapes() {
        for uri in ["file:///tmp/a%", "file:///tmp/a%2", "file:///tmp/a%zz"] {
            assert!(matches!(
                file_uri_to_path(uri),
                Err(UriError::BadPercentEncoding(_))
            ));
        }
    }

    #[test]
    fn scheme_is_case_insensitive() {
        assert_eq!(
            file_uri_to_path("FILE:///tmp/x").unwrap(),
            PathBuf::from("/tmp/x")
        );
    }
}
