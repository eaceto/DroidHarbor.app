import ProjectDescription

// Two channels, one manifest.
//
// A development build and the shipping build are separate apps as far as
// macOS is concerned: their own bundle identifiers, names, URL schemes,
// preferences and state. Sharing an identity is what made LaunchServices
// hand URLs, Services entries and the share extension to whichever copy it
// felt like, which was rarely the one being tested.
//
// `release.sh` sets TUIST_DH_CHANNEL=release along with the version; a plain
// `tuist generate` gives you the development app, which can sit in
// /Applications beside a real one without either noticing.

let isRelease = Environment.dhChannel.getString(default: "development") == "release"

let baseBundleId = "dev.eaceto.apps.macos.droidharbor"
let bundleId = isRelease ? baseBundleId : "\(baseBundleId).dev"
// Two names on purpose. The product name becomes the file on disk and the
// executable inside it, so it has to stay free of spaces; the display name is
// what Finder, the menu bar and the phone show, and can read naturally.
let productName = isRelease ? "DroidHarbor" : "DroidHarborDev"
let appName = isRelease ? "DroidHarbor" : "DroidHarbor Dev"
let urlScheme = isRelease ? "droidharbor" : "droidharbor-dev"
let stateDirectory = isRelease ? ".droidharbor" : ".droidharbor-dev"

// release.sh exports TUIST_DH_VERSION / TUIST_DH_BUILD; a plain
// `tuist generate` falls back to a development version.
let marketingVersion = Environment.dhVersion.getString(default: "0.0.0")
let buildNumber = Environment.dhBuild.getString(default: "0")

let project = Project(
    name: "DroidHarbor",
    options: .options(developmentRegion: "en"),
    targets: [
        .target(
            name: "DroidHarbor",
            destinations: .macOS,
            product: .app,
            productName: productName,
            bundleId: bundleId,
            deploymentTargets: .macOS("14.0"),
            infoPlist: .extendingDefault(with: [
                "CFBundleDisplayName": .string(appName),
                // Read at runtime by AppInfo, so the Swift side never has to
                // know which channel it is.
                "DHURLScheme": .string(urlScheme),
                "DHStateDirectory": .string(stateDirectory),
                "CFBundleIconFile": "AppIcon",
                "CFBundleShortVersionString": .string(marketingVersion),
                "CFBundleVersion": .string(buildNumber),
                "LSApplicationCategoryType": "public.app-category.productivity",
                // The share extension hands work over by opening this.
                "CFBundleURLTypes": .array([
                    .dictionary([
                        "CFBundleURLName": .string("\(bundleId).send"),
                        "CFBundleURLSchemes": .array([.string(urlScheme)]),
                    ])
                ]),
                // Menu-bar app: no Dock icon, no main window.
                "LSUIElement": true,
                // rqs_lib listens for BLE advertisements from phones about to
                // share, so it can re-announce the mDNS service; macOS kills
                // the app without this usage string.
                // English lives here; the other languages are in
                // Resources/InfoPlist.xcstrings, which macOS reads first.
                "NSBluetoothAlwaysUsageDescription": .string(
                    "DroidHarbor uses Bluetooth to notice when a nearby Android device starts sharing, so this Mac shows up in its share sheet faster."
                ),
                "NSLocalNetworkUsageDescription": .string(
                    "DroidHarbor finds nearby Android devices on your local network and transfers files directly with them, without going through the internet."
                ),
            ]),
            sources: ["Sources/**", "Generated/*.swift"],
            resources: [
                "Resources/Localizable.xcstrings",
                "Resources/InfoPlist.xcstrings",
                "Resources/AppIcon.icns",
            ],
            scripts: [
                .pre(
                    script: "exec \"${SRCROOT}/build-rust.sh\"",
                    name: "Build Rust core",
                    basedOnDependencyAnalysis: false
                )
            ],
            // Embeds the share extension in Contents/PlugIns.
            dependencies: [.target(name: "DroidHarborShare")],
            settings: .settings(base: [
                "SWIFT_VERSION": "5.0",
                // The product is renamed per channel; the module is not, so
                // `@testable import DroidHarbor` keeps working either way.
                "PRODUCT_MODULE_NAME": "DroidHarbor",
                "SWIFT_EMIT_LOC_STRINGS": true,
                "SWIFT_OBJC_BRIDGING_HEADER": "$(SRCROOT)/Generated/dh_ffiFFI.h",
                "LIBRARY_SEARCH_PATHS": "$(SRCROOT)/../../target/universal",
                "OTHER_LDFLAGS": "-ldh_ffi -framework CoreBluetooth -framework IOKit -framework Security -framework SystemConfiguration",
                "CODE_SIGN_STYLE": "Automatic",
                "DEVELOPMENT_TEAM": "2U378HJ7FG", // Ezequiel Leonardo Aceto
                "ENABLE_HARDENED_RUNTIME": true,
                "ENABLE_USER_SCRIPT_SANDBOXING": false,
            ])
        ),
        // "Share… → DroidHarbor" everywhere macOS offers a share sheet. A
        // share extension is the only way into that menu.
        .target(
            name: "DroidHarborShare",
            destinations: .macOS,
            product: .appExtension,
            productName: "DroidHarborShare",
            bundleId: "\(bundleId).share",
            deploymentTargets: .macOS("14.0"),
            infoPlist: .extendingDefault(with: [
                "CFBundleDisplayName": .string(appName),
                "DHURLScheme": .string(urlScheme),
                "DHStateDirectory": .string(stateDirectory),
                // Both versions have to match the app exactly. Left unset,
                // the extension took Tuist's defaults (1.0 / 1) against the
                // app's, which Xcode warns about and which app validation
                // rejects outright.
                "CFBundleShortVersionString": .string(marketingVersion),
                "CFBundleVersion": .string(buildNumber),
                "NSExtension": .dictionary([
                    "NSExtensionPointIdentifier": "com.apple.share-services",
                    "NSExtensionPrincipalClass":
                        "$(PRODUCT_MODULE_NAME).ShareViewController",
                    "NSExtensionAttributes": .dictionary([
                        // Anything the app can actually send: files of any
                        // kind, a web address, or a selection of text.
                        "NSExtensionActivationRule": .dictionary([
                            "NSExtensionActivationSupportsFileWithMaxCount": 100,
                            "NSExtensionActivationSupportsImageWithMaxCount": 100,
                            "NSExtensionActivationSupportsMovieWithMaxCount": 100,
                            "NSExtensionActivationSupportsWebURLWithMaxCount": 1,
                            "NSExtensionActivationSupportsText": true,
                        ]),
                    ]),
                ]),
            ]),
            // ShareRequest is shared with the app: one definition of the URL
            // both sides speak.
            // Shared with the app: one definition of the URL both sides
            // speak, and one place that knows which channel this is.
            sources: [
                "ShareExtension/**",
                "Sources/Model/ShareRequest.swift",
                "Sources/Model/AppInfo.swift",
            ],
            // One catalog for the whole app, extension included.
            resources: ["Resources/Localizable.xcstrings"],
            entitlements: "ShareExtension/DroidHarborShare.entitlements",
            settings: .settings(base: [
                "SWIFT_VERSION": "5.0",
                "SWIFT_EMIT_LOC_STRINGS": true,
                "CODE_SIGN_STYLE": "Automatic",
                "DEVELOPMENT_TEAM": "2U378HJ7FG",
                "ENABLE_HARDENED_RUNTIME": true,
            ])
        ),
        .target(
            name: "DroidHarborTests",
            destinations: .macOS,
            product: .unitTests,
            bundleId: "\(bundleId).tests",
            deploymentTargets: .macOS("14.0"),
            sources: ["Tests/**"],
            dependencies: [.target(name: "DroidHarbor")],
            settings: .settings(base: [
                "SWIFT_VERSION": "5.0",
                "CODE_SIGN_STYLE": "Automatic",
                "DEVELOPMENT_TEAM": "2U378HJ7FG",
            ])
        ),
    ]
)
