// A placeholder app icon, drawn rather than committed as a binary: the
// agents' cursor (Overlay) on a paper square. cu-store makes the real one.
import AppKit

enum Icon {
    static func writeIconset(_ dir: String) {
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        for base in [16, 32, 128, 256, 512] {
            for scale in [1, 2] {
                let px = base * scale
                guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                                                 samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                                 colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { continue }
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
                let s = CGFloat(px)
                let bg = NSBezierPath(roundedRect: NSRect(x: s * 0.06, y: s * 0.06, width: s * 0.88, height: s * 0.88),
                                      xRadius: s * 0.2, yRadius: s * 0.2)
                CursorView.edge.setFill(); bg.fill()
                let ctx = NSGraphicsContext.current!.cgContext
                ctx.translateBy(x: s * 0.33, y: s * 0.8)
                ctx.scaleBy(x: s / 34, y: -s / 34)
                let v = CursorView(frame: NSRect(x: 0, y: 0, width: 40, height: 40))
                v.point = .zero
                v.draw(v.bounds)
                NSGraphicsContext.restoreGraphicsState()
                let name = scale == 1 ? "icon_\(base)x\(base).png" : "icon_\(base)x\(base)@2x.png"
                try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
            }
        }
    }
}
