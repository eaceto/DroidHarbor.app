import Foundation

// How the share extension talks to the app.
//
// The extension runs in its own sandboxed process and cannot reach into the
// app, so it opens a `droidharbor://send` URL. The app is not sandboxed and
// can read the paths directly, which keeps the handover to a URL: no shared
// container, nothing copied, nothing to clean up afterwards.
//
// This file is compiled into both targets, so it deliberately deals in plain
// URLs and strings; the app turns the result into its own payload type.

enum ShareRequest {
    /// `droidharbor`, or `droidharbor-dev` for a development build. Both
    /// sides read it from their own bundle, so a development app and a
    /// release app can never pick up each other's requests.
    static var scheme: String { AppInfo.urlScheme }
    static let sendHost = "send"

    enum Shared: Equatable {
        case files([URL])
        case text(String)
    }

    /// Build the URL the extension opens. Paths and text travel as query
    /// items, so spaces and non-ASCII names survive percent-encoding.
    static func url(for shared: Shared) -> URL? {
        var components = URLComponents()
        components.scheme = scheme
        components.host = sendHost
        switch shared {
        case .files(let urls):
            guard !urls.isEmpty else { return nil }
            components.queryItems = urls.map { URLQueryItem(name: "path", value: $0.path) }
        case .text(let text):
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return nil }
            components.queryItems = [URLQueryItem(name: "text", value: trimmed)]
        }
        return components.url
    }

    /// What a received URL is asking for; `nil` when it is not ours or
    /// carries nothing usable.
    static func parse(_ url: URL) -> Shared? {
        guard url.scheme?.lowercased() == scheme, url.host?.lowercased() == sendHost,
              let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems
        else {
            return nil
        }

        // Files win when both are present: some apps offer a link's title as
        // text beside the file it points at.
        let paths = items.filter { $0.name == "path" }.compactMap(\.value)
        if !paths.isEmpty {
            return .files(paths.map { URL(fileURLWithPath: $0) })
        }
        if let text = items.first(where: { $0.name == "text" })?.value,
           !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        {
            return .text(text)
        }
        return nil
    }
}
