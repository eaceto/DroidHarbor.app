import Foundation
import ServiceManagement

/// Wrapper over `SMAppService` for the "Open at login" setting.
///
/// Registration only works for a real app bundle in a stable location;
/// running from Xcode's DerivedData can fail, which is reported rather than
/// silently ignored.
enum LaunchAtLogin {
    static var isEnabled: Bool {
        SMAppService.mainApp.status == .enabled
    }

    /// Returns an error message on failure, `nil` on success.
    static func set(_ enabled: Bool) -> String? {
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            return nil
        } catch {
            return "Could not change the login item: \(error.localizedDescription)"
        }
    }
}
