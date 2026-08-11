import SwiftUI

// About panel. Beyond the usual credits this carries the GPL notice and the
// link to the source, which distributing a binary built on GPL code requires.

struct AboutView: View {
    @EnvironmentObject private var state: AppState

    private var version: String {
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String ?? "?"
        let build = info?["CFBundleVersion"] as? String ?? "?"
        return "\(short) (\(build))"
    }

    var body: some View {
        VStack(spacing: 14) {
            if let icon = NSApp.applicationIconImage {
                Image(nsImage: icon)
                    .resizable()
                    .frame(width: 84, height: 84)
                    .accessibilityHidden(true)
            }

            VStack(spacing: 3) {
                Text("DroidHarbor")
                    .font(.title2.weight(.semibold))
                Text("Version \(version)")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                Link("Made by Ezequiel (Kimi) Aceto", destination: URL(string: Links.author)!)
                    .font(.callout)
                    .padding(.top, 3)
            }

            Text("Receive files from Android's built-in sharing, and send files back, over your local network, with nothing in the cloud.")
                .font(.callout)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Divider()

            VStack(spacing: 6) {
                Text("Free software under the GNU General Public License v3 or later, and built on rquickshare. The complete source code is available:")
                    .font(.caption)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                HStack(spacing: 12) {
                    Link("Source code", destination: URL(string: Links.source)!)
                    Link("License", destination: URL(string: Links.license)!)
                }
                .font(.callout)
            }

            VStack(spacing: 4) {
                Text("Implements an unofficial, reverse-engineered protocol in order to interoperate with the sharing built into Android; not affiliated with or endorsed by Google or Android.")
                Text("Android and Quick Share are trademarks of Google LLC.")
            }
            .font(.caption2)
            .multilineTextAlignment(.center)
            .foregroundStyle(.tertiary)
            .fixedSize(horizontal: false, vertical: true)

            if let update = state.availableUpdate {
                Divider()
                VStack(spacing: 6) {
                    Text("Version \(update.version) is available")
                        .font(.callout.weight(.medium))
                    Link("Download", destination: update.url)
                        .buttonStyle(.borderedProminent)
                }
            } else {
                Button("Check for Updates…") { state.checkForUpdates(userInitiated: true) }
                    .controlSize(.small)
            }
        }
        .padding(24)
        .frame(width: 380)
        .tint(.teal)
    }
}

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
