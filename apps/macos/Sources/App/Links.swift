import Foundation

/// The app's external addresses, kept in one place so the About panel, the
/// Help menu and the update checker cannot drift apart.
enum Links {
    static let author = "https://kimi.blog"
    static let source = "https://github.com/eaceto/DroidHarbor.app"
    static let license = "https://www.gnu.org/licenses/gpl-3.0.html"
    /// Read from the release assets rather than a file in the repository:
    /// release.sh writes it beside the DMG, both are uploaded together, and
    /// "latest" resolves to whichever release is current, so the manifest
    /// cannot describe a version the download does not match.
    static let updateManifest =
        "https://github.com/eaceto/DroidHarbor.app/releases/latest/download/updates.json"
}
