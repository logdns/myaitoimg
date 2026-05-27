import AppKit

let size = 1024
let image = NSImage(size: NSSize(width: size, height: size))

image.lockFocus()
NSColor.clear.setFill()
NSRect(x: 0, y: 0, width: size, height: size).fill()

let background = NSBezierPath(roundedRect: NSRect(x: 0, y: 0, width: size, height: size), xRadius: 210, yRadius: 210)
NSColor.white.setFill()
background.fill()

func path(points: [(CGFloat, CGFloat)], offsetY: CGFloat = 0) -> NSBezierPath {
    let p = NSBezierPath()
    p.move(to: NSPoint(x: points[0].0, y: CGFloat(size) - points[0].1 - offsetY))
    for point in points.dropFirst() {
        p.line(to: NSPoint(x: point.0, y: CGFloat(size) - point.1 - offsetY))
    }
    p.close()
    return p
}

let outer: [(CGFloat, CGFloat)] = [
    (206, 694), (178, 649), (357, 263), (441, 263), (524, 441),
    (606, 263), (691, 263), (870, 649), (842, 694), (757, 694),
    (716, 667), (650, 519), (582, 666), (538, 694), (510, 694),
    (466, 666), (398, 519), (332, 667), (290, 694)
]

NSGraphicsContext.saveGraphicsState()
let shadow = NSShadow()
shadow.shadowColor = NSColor(calibratedRed: 0.15, green: 0.21, blue: 0.30, alpha: 0.18)
shadow.shadowOffset = NSSize(width: 0, height: -26)
shadow.shadowBlurRadius = 32
shadow.set()
NSColor(calibratedRed: 0.48, green: 0.36, blue: 1, alpha: 1).setFill()
path(points: outer).fill()
NSGraphicsContext.restoreGraphicsState()

let gradient = NSGradient(colors: [
    NSColor(calibratedRed: 0.46, green: 0.65, blue: 1.0, alpha: 1),
    NSColor(calibratedRed: 0.48, green: 0.36, blue: 1.0, alpha: 1),
    NSColor(calibratedRed: 0.27, green: 0.82, blue: 0.84, alpha: 1)
])!
gradient.draw(in: path(points: outer), angle: -45)

let inner: [(CGFloat, CGFloat)] = [
    (433, 694), (312, 694), (438, 420), (490, 420), (524, 494),
    (558, 420), (610, 420), (736, 694), (616, 694), (524, 496)
]
let innerGradient = NSGradient(colors: [
    NSColor(calibratedRed: 0.51, green: 0.40, blue: 1.0, alpha: 0.88),
    NSColor(calibratedRed: 0.37, green: 0.72, blue: 1.0, alpha: 0.88)
])!
innerGradient.draw(in: path(points: inner), angle: -80)

image.unlockFocus()

guard
    let tiff = image.tiffRepresentation,
    let bitmap = NSBitmapImageRep(data: tiff),
    let data = bitmap.representation(using: .png, properties: [:])
else {
    fatalError("failed to render icon")
}

try data.write(to: URL(fileURLWithPath: "src-tauri/icons/icon.png"))
