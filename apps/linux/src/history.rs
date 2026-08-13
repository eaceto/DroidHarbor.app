//! Persistent record of what arrived and what was sent.
//!
//! The field names match the macOS app's `history.json` deliberately, so the
//! two produce the same shape and a file from one is readable by the other.
//! What differs is where it lives: macOS keeps `~/.droidharbor`, while here it
//! follows the XDG data directory, which is what a Linux user expects and what
//! backup tools already know about.
//!
//! Entries are never truncated. This is the user's own record of what they
//! received, and silently dropping the old end of it loses history they may
//! still want.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Received,
    Sent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Files,
    Link,
    Text,
    Wifi,
    Contact,
    Calendar,
    Phone,
    Email,
    Map,
}

impl Kind {
    /// The domain reports payload kinds as free-form strings, so anything
    /// unrecognised becomes plain text rather than being dropped.
    pub fn from_domain(kind: &str) -> Self {
        match kind {
            "link" => Kind::Link,
            "wifi" => Kind::Wifi,
            "contact" => Kind::Contact,
            "calendar" => Kind::Calendar,
            "phone" => Kind::Phone,
            "email" => Kind::Email,
            "map" => Kind::Map,
            _ => Kind::Text,
        }
    }
}

/// What the filter groups entries by. Files are classified by extension,
/// because "an image" is what someone looks for, not "a file transfer".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    All,
    Image,
    Video,
    Audio,
    Document,
    Link,
    Contact,
    Calendar,
    Phone,
    Email,
    Map,
    Other,
}

impl Category {
    pub const ALL: [Category; 12] = [
        Category::All,
        Category::Image,
        Category::Video,
        Category::Audio,
        Category::Document,
        Category::Link,
        Category::Contact,
        Category::Calendar,
        Category::Phone,
        Category::Email,
        Category::Map,
        Category::Other,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::All => "All",
            Category::Image => "Images",
            Category::Video => "Video",
            Category::Audio => "Audio",
            Category::Document => "Docs",
            Category::Link => "Links",
            Category::Contact => "Contacts",
            Category::Calendar => "Events",
            Category::Phone => "Phones",
            Category::Email => "Emails",
            Category::Map => "Places",
            Category::Other => "Other",
        }
    }

    fn for_extension(extension: &str) -> Self {
        match extension.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "heic" | "heif" | "webp" | "bmp" | "tiff" | "tif"
            | "svg" | "avif" | "raw" | "dng" => Category::Image,
            "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "3gp" | "mpg" | "mpeg" | "wmv" => {
                Category::Video
            }
            "mp3" | "m4a" | "wav" | "aac" | "flac" | "ogg" | "opus" | "aiff" | "wma" => {
                Category::Audio
            }
            "pdf" | "doc" | "docx" | "pages" | "txt" | "rtf" | "md" | "csv" | "xls" | "xlsx"
            | "numbers" | "ppt" | "pptx" | "key" | "epub" => Category::Document,
            _ => Category::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: uuid::Uuid,
    pub direction: Direction,
    /// Peer device name as it presented itself.
    pub peer: String,
    pub date: chrono::DateTime<chrono::Utc>,
    /// Absolute paths for file transfers; empty for links and text.
    #[serde(default)]
    pub paths: Vec<String>,
    /// The link or text itself, which never touches the disk.
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: Kind,
}

fn default_kind() -> Kind {
    Kind::Files
}

impl Entry {
    pub fn new(direction: Direction, peer: String, paths: Vec<String>) -> Self {
        Entry {
            id: uuid::Uuid::new_v4(),
            direction,
            peer,
            date: chrono::Utc::now(),
            paths,
            content: None,
            kind: Kind::Files,
        }
    }

    pub fn text(direction: Direction, peer: String, kind: Kind, content: String) -> Self {
        Entry {
            id: uuid::Uuid::new_v4(),
            direction,
            peer,
            date: chrono::Utc::now(),
            paths: Vec::new(),
            content: Some(content),
            kind,
        }
    }

    /// Whether the row is named by a filename rather than by its content.
    pub fn is_file(&self) -> bool {
        self.kind == Kind::Files || (!self.paths.is_empty() && self.content.is_none())
    }

    pub fn primary_name(&self) -> String {
        if let Some(content) = &self.content {
            if !self.is_file() {
                return content.clone();
            }
        }
        match self.paths.first() {
            Some(path) => Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone()),
            None => "Transfer".to_string(),
        }
    }

    pub fn summary(&self) -> String {
        if !self.is_file() || self.paths.len() <= 1 {
            return self.primary_name();
        }
        format!("{} and {} more", self.primary_name(), self.paths.len() - 1)
    }

    pub fn category(&self) -> Category {
        match self.kind {
            Kind::Link => Category::Link,
            Kind::Contact => Category::Contact,
            Kind::Calendar => Category::Calendar,
            Kind::Phone => Category::Phone,
            Kind::Email => Category::Email,
            Kind::Map => Category::Map,
            Kind::Text | Kind::Wifi => Category::Other,
            Kind::Files => match self.paths.first() {
                Some(path) => Category::for_extension(
                    Path::new(path)
                        .extension()
                        .map(|e| e.to_string_lossy())
                        .unwrap_or_default()
                        .as_ref(),
                ),
                None => Category::Other,
            },
        }
    }

    /// True when the payload is a file on disk rather than content we hold.
    pub fn has_file(&self) -> bool {
        !self.paths.is_empty()
    }

    /// Free-text match over what someone would actually type: part of a name,
    /// an extension, a domain, or who sent it.
    pub fn matches(&self, query: &str) -> bool {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        // A leading dot reads as "the extension", not part of a name.
        let as_extension = needle.strip_prefix('.').unwrap_or(&needle);

        if self.peer.to_lowercase().contains(&needle) {
            return true;
        }
        if let Some(content) = &self.content {
            if content.to_lowercase().contains(&needle) {
                return true;
            }
        }
        self.paths.iter().any(|path| {
            let path = Path::new(path);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if name.contains(&needle) {
                return true;
            }
            path.extension()
                .map(|e| e.to_string_lossy().to_lowercase() == as_extension)
                .unwrap_or(false)
        })
    }

    pub fn icon(&self) -> &'static str {
        match self.kind {
            Kind::Files => match self.direction {
                Direction::Received => "document-save-symbolic",
                Direction::Sent => "document-send-symbolic",
            },
            Kind::Link => "insert-link-symbolic",
            Kind::Text => "text-x-generic-symbolic",
            Kind::Wifi => "network-wireless-symbolic",
            Kind::Contact => "avatar-default-symbolic",
            Kind::Calendar => "x-office-calendar-symbolic",
            Kind::Phone => "phone-symbolic",
            Kind::Email => "mail-unread-symbolic",
            Kind::Map => "mark-location-symbolic",
        }
    }
}

/// `$XDG_DATA_HOME/droidharbor/history.json`, falling back to the specified
/// default of `~/.local/share`.
pub fn file_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join("droidharbor").join("history.json")
}

/// Unreadable or corrupt history yields an empty list rather than an error:
/// losing the record is bad, but refusing to start over it is worse.
pub fn load(path: &Path) -> Vec<Entry> {
    let Ok(data) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str(&data) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::error!(%error, path = %path.display(), "history could not be read");
            Vec::new()
        }
    }
}

/// Written pretty-printed: this is a file someone might open, so it deserves
/// to be readable. Saved via a temporary file and renamed, so an interrupted
/// write cannot leave a truncated history behind.
pub fn save(path: &Path, entries: &[Entry]) {
    let Some(directory) = path.parent() else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(directory) {
        tracing::error!(%error, "could not create the history directory");
        return;
    }

    let Ok(json) = serde_json::to_string_pretty(entries) else {
        return;
    };
    let temporary = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&temporary, json) {
        tracing::error!(%error, "could not write history");
        return;
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        tracing::error!(%error, "could not replace history");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_entry(name: &str) -> Entry {
        Entry::new(
            Direction::Received,
            "Pixel 8".into(),
            vec![format!("/home/kimi/Downloads/{name}")],
        )
    }

    #[test]
    fn names_and_summaries() {
        let one = file_entry("holiday.jpg");
        assert_eq!(one.primary_name(), "holiday.jpg");
        assert_eq!(one.summary(), "holiday.jpg");

        let mut many = file_entry("holiday.jpg");
        many.paths.push("/home/kimi/Downloads/beach.jpg".into());
        assert_eq!(many.summary(), "holiday.jpg and 1 more");
    }

    #[test]
    fn categories_by_extension() {
        assert_eq!(file_entry("a.JPG").category(), Category::Image);
        assert_eq!(file_entry("a.mkv").category(), Category::Video);
        assert_eq!(file_entry("a.flac").category(), Category::Audio);
        assert_eq!(file_entry("a.pdf").category(), Category::Document);
        assert_eq!(file_entry("a.bin").category(), Category::Other);
        assert_eq!(file_entry("noextension").category(), Category::Other);
    }

    #[test]
    fn links_are_named_by_content() {
        let link = Entry::text(
            Direction::Received,
            "Pixel 8".into(),
            Kind::Link,
            "https://example.com/page".into(),
        );
        assert!(!link.is_file());
        assert_eq!(link.primary_name(), "https://example.com/page");
        assert_eq!(link.category(), Category::Link);
        assert!(!link.has_file());
    }

    #[test]
    fn search_covers_name_extension_peer_and_content() {
        let entry = file_entry("Holiday Photo.jpg");
        assert!(entry.matches(""), "empty query matches everything");
        assert!(entry.matches("holiday"), "partial name, case-insensitive");
        assert!(entry.matches(".jpg"), "leading dot means extension");
        assert!(entry.matches("jpg"));
        assert!(entry.matches("pixel"), "sender name");
        assert!(!entry.matches("beach"));

        let link = Entry::text(
            Direction::Sent,
            "Pixel 8".into(),
            Kind::Link,
            "https://example.com/page".into(),
        );
        assert!(link.matches("example.com"));
    }

    #[test]
    fn round_trips_through_json() {
        let entries = vec![
            file_entry("a.png"),
            Entry::text(
                Direction::Sent,
                "Pixel 8".into(),
                Kind::Link,
                "https://example.com".into(),
            ),
        ];
        let json = serde_json::to_string(&entries).unwrap();
        let back: Vec<Entry> = serde_json::from_str(&json).unwrap();
        assert_eq!(entries, back);
    }

    #[test]
    fn tolerates_entries_missing_optional_fields() {
        // What an older build, written before links were recorded, left behind.
        let json = r#"[{
            "id": "6f1b2c3d-4e5f-4a6b-8c9d-0e1f2a3b4c5d",
            "direction": "received",
            "peer": "Pixel 8",
            "date": "2026-08-12T06:48:08Z"
        }]"#;
        let entries: Vec<Entry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, Kind::Files);
        assert!(entries[0].paths.is_empty());
    }
}
