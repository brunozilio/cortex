// Render the Finder installer artwork at 2× without extra build dependencies.
import AppKit

let width = 660.0
let height = 420.0
guard CommandLine.arguments.count == 2 else {
    fatalError("usage: swift dmg-background.swift OUTPUT.png")
}
let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(width * 2), pixelsHigh: Int(height * 2),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
)!
bitmap.size = NSSize(width: width, height: height)
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
NSColor(calibratedWhite: 0.97, alpha: 1).setFill()
NSRect(x: 0, y: 0, width: width, height: height).fill()

func text(_ value: String, y: Double, size: Double, weight: NSFont.Weight, color: NSColor) {
    let attributes: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: color,
    ]
    let label = NSAttributedString(string: value, attributes: attributes)
    label.draw(at: NSPoint(x: (width - label.size().width) / 2, y: y))
}

text("Superfik", y: 334, size: 28, weight: .semibold, color: NSColor(calibratedWhite: 0.12, alpha: 1))
text("Arraste para Aplicativos para instalar", y: 302, size: 15, weight: .regular,
     color: NSColor(calibratedWhite: 0.40, alpha: 1))
// Finder positions the real, draggable icons over this background.
let arrow = NSBezierPath()
arrow.move(to: NSPoint(x: 302, y: 210))
arrow.line(to: NSPoint(x: 358, y: 210))
arrow.move(to: NSPoint(x: 348, y: 220))
arrow.line(to: NSPoint(x: 358, y: 210))
arrow.line(to: NSPoint(x: 348, y: 200))
arrow.lineWidth = 2.5
arrow.lineCapStyle = .round
arrow.lineJoinStyle = .round
NSColor(calibratedWhite: 0.60, alpha: 1).setStroke()
arrow.stroke()
text("Depois, abra o Superfik em Aplicativos.", y: 48, size: 13, weight: .regular,
     color: NSColor(calibratedWhite: 0.43, alpha: 1))
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using: .png, properties: [:])!.write(
    to: URL(fileURLWithPath: CommandLine.arguments[1])
)
