import Foundation

// Which copy of DroidHarbor this is.
//
// A development build and a release build are two separate apps to macOS:
// different bundle identifiers, names, URL schemes and state. That is
// deliberate. When both share an identity, LaunchServices picks one of them
// for URLs, Services and share extensions, and it is rarely the one you are
// testing. Keeping them apart means a release copy can stay installed and
// working while a development copy runs beside it.
//
// The values come from Info.plist, which Project.swift fills in per channel,
// so nothing here has to be edited to add a channel. This file is compiled
// into the app and the share extension alike.

enum AppInfo {
    /// The scheme the share extension opens to hand work to the app.
    /// `droidharbor` for a release build, `droidharbor-dev` beside it.
    static let urlScheme = string(for: "DHURLScheme", default: "droidharbor")

    /// Folder under the home directory holding history and other state.
    static let stateDirectoryName = string(for: "DHStateDirectory", default: ".droidharbor")

    /// The name shown in the UI and to nearby phones.
    static var displayName: String {
        string(for: "CFBundleDisplayName", default: "DroidHarbor")
    }

    /// Bundle-scoped so a development copy and a release copy never hear
    /// each other's messages.
    static var forwardedURLNotification: Notification.Name {
        let id = Bundle.main.bundleIdentifier ?? "dev.eaceto.apps.macos.droidharbor"
        // The extension's identifier is the app's with a suffix; strip it so
        // both sides name the same notification.
        let app = id.hasSuffix(".share") ? String(id.dropLast(".share".count)) : id
        return Notification.Name("\(app).forwardedURL")
    }

    /// True for a build that is not the shipping one, so the UI can say so.
    static var isDevelopmentBuild: Bool {
        Bundle.main.bundleIdentifier?.contains(".dev") == true
    }

    private static func string(for key: String, default fallback: String) -> String {
        let value = Bundle.main.object(forInfoDictionaryKey: key) as? String
        return value?.isEmpty == false ? value! : fallback
    }
}
