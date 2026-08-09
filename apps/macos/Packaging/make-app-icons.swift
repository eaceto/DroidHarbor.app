#!/usr/bin/env swift
// Renders the candidate app icons. Run from apps/macos:
//
//   swift Packaging/make-app-icons.swift
//
// Writes Packaging/icons/<name>.icns for each design plus a preview sheet.
// Every size is re-rendered from the same vector code rather than downscaled,
// so the 16pt version stays crisp instead of turning to mush.

import AppKit

// MARK: - Canvas

/// macOS icons sit inside their canvas: the rounded square covers about 80%
/// of the width, leaving room for the shadow Apple's grid expects.
let artworkInset: CGFloat = 0.1
let cornerRatio: CGFloat = 0.225

func artworkRect(_ size: CGFloat) -> NSRect {
    let inset = size * artworkInset
    return NSRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
}

func plate(_ rect: NSRect) -> NSBezierPath {
    NSBezierPath(
        roundedRect: rect,
        xRadius: rect.width * cornerRatio,
        yRadius: rect.width * cornerRatio)
}

func gradient(_ top: NSColor, _ bottom: NSColor) -> NSGradient {
    NSGradient(colors: [top, bottom])!
}

func rgb(_ r: Int, _ g: Int, _ b: Int) -> NSColor {
    NSColor(
        calibratedRed: CGFloat(r) / 255, green: CGFloat(g) / 255,
        blue: CGFloat(b) / 255, alpha: 1)
}

/// Soft drop shadow under the plate, matching how system icons sit.
func drawPlate(_ rect: NSRect, _ top: NSColor, _ bottom: NSColor) {
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.28)
    shadow.shadowOffset = NSSize(width: 0, height: -rect.width * 0.022)
    shadow.shadowBlurRadius = rect.width * 0.05
    shadow.set()
    NSColor.black.setFill()
    plate(rect).fill()
    NSGraphicsContext.restoreGraphicsState()

    let path = plate(rect)
    path.addClip()
    gradient(top, bottom).draw(in: rect, angle: -90)

    // A hint of light along the top edge keeps it from looking flat.
    let sheen = NSGradient(colors: [
        NSColor.white.withAlphaComponent(0.20), NSColor.white.withAlphaComponent(0),
    ])!
    sheen.draw(
        in: NSRect(x: rect.minX, y: rect.midY, width: rect.width, height: rect.height / 2),
        angle: -90)
}

// MARK: - Designs

/// 1. Harbor: an anchor, for the name.
func drawHarbor(_ size: CGFloat) {
    let rect = artworkRect(size)
    NSGraphicsContext.saveGraphicsState()
    drawPlate(rect, rgb(24, 178, 184), rgb(9, 89, 108))

    let unit = rect.width
    let cx = rect.midX
    let stroke = unit * 0.075
    NSColor.white.setStroke()
    NSColor.white.setFill()

    // Ring
    let ringRadius = unit * 0.085
    let ringCenter = NSPoint(x: cx, y: rect.minY + unit * 0.80)
    let ring = NSBezierPath(ovalIn: NSRect(
        x: ringCenter.x - ringRadius, y: ringCenter.y - ringRadius,
        width: ringRadius * 2, height: ringRadius * 2))
    ring.lineWidth = stroke
    ring.stroke()

    // Shank
    let shank = NSBezierPath()
    shank.move(to: NSPoint(x: cx, y: ringCenter.y - ringRadius))
    shank.line(to: NSPoint(x: cx, y: rect.minY + unit * 0.20))
    shank.lineWidth = stroke
    shank.lineCapStyle = .round
    shank.stroke()

    // Stock (crossbar)
    let stock = NSBezierPath()
    let stockY = rect.minY + unit * 0.615
    stock.move(to: NSPoint(x: cx - unit * 0.20, y: stockY))
    stock.line(to: NSPoint(x: cx + unit * 0.20, y: stockY))
    stock.lineWidth = stroke
    stock.lineCapStyle = .round
    stock.stroke()

    // Arms: an arc opening upward, with flukes at the tips.
    let arms = NSBezierPath()
    let armRadius = unit * 0.275
    arms.appendArc(
        withCenter: NSPoint(x: cx, y: rect.minY + unit * 0.30),
        radius: armRadius, startAngle: 200, endAngle: 340, clockwise: false)
    arms.lineWidth = stroke
    arms.lineCapStyle = .round
    arms.stroke()

    for side in [-1.0, 1.0] as [CGFloat] {
        let tipX = cx + side * armRadius * cos(20 * .pi / 180)
        let tipY = rect.minY + unit * 0.30 - armRadius * sin(20 * .pi / 180)
        let fluke = NSBezierPath()
        fluke.move(to: NSPoint(x: tipX, y: tipY + unit * 0.055))
        fluke.line(to: NSPoint(x: tipX + side * unit * 0.075, y: tipY + unit * 0.035))
        fluke.line(to: NSPoint(x: tipX, y: tipY - unit * 0.065))
        fluke.close()
        fluke.fill()
    }
    NSGraphicsContext.restoreGraphicsState()
}

/// 2. Beacon: broadcast arcs, echoing the menu-bar glyph.
func drawBeacon(_ size: CGFloat) {
    let rect = artworkRect(size)
    NSGraphicsContext.saveGraphicsState()
    drawPlate(rect, rgb(31, 62, 82), rgb(11, 25, 38))

    let unit = rect.width
    let center = NSPoint(x: rect.midX, y: rect.midY - unit * 0.02)
    let teal = rgb(45, 212, 205)

    // Core
    teal.setFill()
    let dot = unit * 0.075
    NSBezierPath(ovalIn: NSRect(
        x: center.x - dot, y: center.y - dot, width: dot * 2, height: dot * 2)).fill()

    // Arcs on both sides, fading outward.
    for (index, radius) in [0.19, 0.29, 0.39].enumerated() {
        let alpha = 1.0 - Double(index) * 0.26
        teal.withAlphaComponent(alpha).setStroke()
        for side in [0.0, 180.0] {
            let arc = NSBezierPath()
            arc.appendArc(
                withCenter: center, radius: unit * radius,
                startAngle: -42 + side, endAngle: 42 + side, clockwise: false)
            arc.lineWidth = unit * 0.055
            arc.lineCapStyle = .round
            arc.stroke()
        }
    }
    NSGraphicsContext.restoreGraphicsState()
}

/// 3. Downlink: an arrow dropping into a tray, which is what the app does.
func drawDownlink(_ size: CGFloat) {
    let rect = artworkRect(size)
    NSGraphicsContext.saveGraphicsState()
    drawPlate(rect, rgb(38, 132, 255), rgb(14, 61, 145))

    let unit = rect.width
    let cx = rect.midX
    NSColor.white.setFill()
    NSColor.white.setStroke()

    // Arrow shaft and head
    let shaft = NSBezierPath()
    shaft.move(to: NSPoint(x: cx, y: rect.minY + unit * 0.72))
    shaft.line(to: NSPoint(x: cx, y: rect.minY + unit * 0.42))
    shaft.lineWidth = unit * 0.085
    shaft.lineCapStyle = .round
    shaft.stroke()

    let head = NSBezierPath()
    let tipY = rect.minY + unit * 0.335
    head.move(to: NSPoint(x: cx - unit * 0.145, y: tipY + unit * 0.115))
    head.line(to: NSPoint(x: cx + unit * 0.145, y: tipY + unit * 0.115))
    head.line(to: NSPoint(x: cx, y: tipY))
    head.close()
    head.fill()

    // Tray: an open box the arrow lands in.
    let tray = NSBezierPath()
    let left = cx - unit * 0.27
    let right = cx + unit * 0.27
    let top = rect.minY + unit * 0.30
    let bottom = rect.minY + unit * 0.175
    tray.move(to: NSPoint(x: left, y: top))
    tray.line(to: NSPoint(x: left, y: bottom))
    tray.line(to: NSPoint(x: right, y: bottom))
    tray.line(to: NSPoint(x: right, y: top))
    tray.lineWidth = unit * 0.075
    tray.lineCapStyle = .round
    tray.lineJoinStyle = .round
    tray.stroke()

    NSGraphicsContext.restoreGraphicsState()
}

let designs: [(name: String, draw: (CGFloat) -> Void)] = [
    ("harbor", drawHarbor),
    ("beacon", drawBeacon),
    ("downlink", drawDownlink),
]

// MARK: - Output

func render(_ draw: (CGFloat) -> Void, pixels: Int) -> NSBitmapImageRep {
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
    else { fatalError("could not allocate \(pixels)px bitmap") }
    rep.size = NSSize(width: pixels, height: pixels)

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current?.imageInterpolation = .high
    draw(CGFloat(pixels))
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

func write(_ rep: NSBitmapImageRep, to url: URL) throws {
    guard let data = rep.representation(using: .png, properties: [:]) else {
        fatalError("could not encode \(url.lastPathComponent)")
    }
    try data.write(to: url)
}

let fm = FileManager.default
let outDir = URL(fileURLWithPath: "Packaging/icons")
try? fm.createDirectory(at: outDir, withIntermediateDirectories: true)

// (point size, scale) pairs an .iconset must contain.
let iconsetSizes: [(Int, Int)] = [
    (16, 1), (16, 2), (32, 1), (32, 2), (128, 1),
    (128, 2), (256, 1), (256, 2), (512, 1), (512, 2),
]

for design in designs {
    let iconset = outDir.appendingPathComponent("\(design.name).iconset")
    try? fm.removeItem(at: iconset)
    try fm.createDirectory(at: iconset, withIntermediateDirectories: true)

    for (points, scale) in iconsetSizes {
        let suffix = scale == 1 ? "" : "@2x"
        let rep = render(design.draw, pixels: points * scale)
        try write(rep, to: iconset.appendingPathComponent(
            "icon_\(points)x\(points)\(suffix).png"))
    }

    let icns = outDir.appendingPathComponent("\(design.name).icns")
    let convert = Process()
    convert.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
    convert.arguments = ["-c", "icns", iconset.path, "-o", icns.path]
    try convert.run()
    convert.waitUntilExit()
    guard convert.terminationStatus == 0 else { fatalError("iconutil failed for \(design.name)") }
    print("wrote \(icns.lastPathComponent)")
}

// Preview sheet: each design large, plus the 32pt rendering beside it, since
// small-size legibility is what usually decides between candidates.
let cell: CGFloat = 260
let sheetSize = NSSize(width: cell * CGFloat(designs.count), height: cell + 90)
guard let sheet = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(sheetSize.width * 2),
    pixelsHigh: Int(sheetSize.height * 2), bitsPerSample: 8, samplesPerPixel: 4,
    hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
    bytesPerRow: 0, bitsPerPixel: 0)
else { fatalError("no preview bitmap") }
sheet.size = sheetSize

NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: sheet)
rgb(246, 246, 248).setFill()
NSRect(origin: .zero, size: sheetSize).fill()

for (index, design) in designs.enumerated() {
    let originX = CGFloat(index) * cell
    NSGraphicsContext.saveGraphicsState()
    let transform = NSAffineTransform()
    transform.translateX(by: originX, yBy: 60)
    transform.concat()
    design.draw(cell)
    NSGraphicsContext.restoreGraphicsState()

    // 32pt beside the name, at true size
    NSGraphicsContext.saveGraphicsState()
    let small = NSAffineTransform()
    small.translateX(by: originX + cell / 2 - 60, yBy: 12)
    small.concat()
    design.draw(32)
    NSGraphicsContext.restoreGraphicsState()

    let label = design.name
    let attributes: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: 15, weight: .medium),
        .foregroundColor: NSColor(calibratedWhite: 0.2, alpha: 1),
    ]
    let labelSize = label.size(withAttributes: attributes)
    label.draw(
        at: NSPoint(x: originX + cell / 2 - labelSize.width / 2 + 14, y: 20),
        withAttributes: attributes)
}
NSGraphicsContext.restoreGraphicsState()

try write(sheet, to: outDir.appendingPathComponent("preview.png"))
print("wrote preview.png")
