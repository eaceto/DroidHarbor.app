#!/usr/bin/env swift
// Renders the disk-image background. Run when the artwork changes:
//
//   swift Packaging/make-dmg-background.swift
//
// Produces dmg-background.tiff with 1x and 2x representations, which Finder
// picks between by display. Committed as an asset so releases do not depend
// on regenerating it.

import AppKit

let size = NSSize(width: 660, height: 400)
// Where release.sh positions the two icons, measured from the top-left in
// Finder's coordinates; the arrow is drawn to sit between them.
let appIconCenterX: CGFloat = 165
let applicationsCenterX: CGFloat = 495
let iconCenterYFromTop: CGFloat = 205

func draw(scale: CGFloat) -> NSBitmapImageRep {
    let pixels = NSSize(width: size.width * scale, height: size.height * scale)
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: Int(pixels.width), pixelsHigh: Int(pixels.height),
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
    else { fatalError("could not allocate the bitmap") }
    rep.size = size

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

    // Soft vertical wash; deliberately light, since Finder shows this image
    // as-is in both appearances.
    let backdrop = NSGradient(
        colors: [
            NSColor(calibratedRed: 0.98, green: 0.99, blue: 0.99, alpha: 1),
            NSColor(calibratedRed: 0.90, green: 0.94, blue: 0.95, alpha: 1),
        ])
    backdrop?.draw(in: NSRect(origin: .zero, size: size), angle: -90)

    // Title and hint. AppKit's origin is bottom-left; y values are converted
    // from the top so they line up with the icon positions above.
    let title = "DroidHarbor"
    let titleAttributes: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: 22, weight: .semibold),
        .foregroundColor: NSColor(calibratedWhite: 0.15, alpha: 1),
    ]
    let titleSize = title.size(withAttributes: titleAttributes)
    title.draw(
        at: NSPoint(x: (size.width - titleSize.width) / 2, y: size.height - 62),
        withAttributes: titleAttributes)

    let hint = "Drag the app onto the Applications folder to install"
    let hintAttributes: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: 13, weight: .regular),
        .foregroundColor: NSColor(calibratedWhite: 0.42, alpha: 1),
    ]
    let hintSize = hint.size(withAttributes: hintAttributes)
    hint.draw(
        at: NSPoint(x: (size.width - hintSize.width) / 2, y: size.height - 88),
        withAttributes: hintAttributes)

    // Arrow from the app towards the Applications alias.
    let arrowY = size.height - iconCenterYFromTop
    let start = NSPoint(x: appIconCenterX + 95, y: arrowY)
    let end = NSPoint(x: applicationsCenterX - 95, y: arrowY)
    NSColor(calibratedRed: 0.13, green: 0.60, blue: 0.62, alpha: 0.85).setStroke()

    let shaft = NSBezierPath()
    shaft.move(to: start)
    shaft.line(to: NSPoint(x: end.x - 12, y: end.y))
    shaft.lineWidth = 3
    shaft.lineCapStyle = .round
    shaft.stroke()

    let head = NSBezierPath()
    head.move(to: NSPoint(x: end.x - 16, y: end.y + 9))
    head.line(to: end)
    head.line(to: NSPoint(x: end.x - 16, y: end.y - 9))
    head.lineWidth = 3
    head.lineCapStyle = .round
    head.lineJoinStyle = .round
    head.stroke()

    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let output = URL(fileURLWithPath: "Packaging/dmg-background.tiff")
let image = NSImage(size: size)
image.addRepresentation(draw(scale: 1))
image.addRepresentation(draw(scale: 2))
guard let data = image.tiffRepresentation else { fatalError("no TIFF data") }
try data.write(to: output)
print("wrote \(output.path)")
