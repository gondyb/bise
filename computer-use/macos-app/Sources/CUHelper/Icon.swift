// A placeholder app icon, drawn rather than committed as a binary: the
// agents' cursor (Overlay) on a paper square. cu-store makes the real one.
import AppKit

enum Icon {
    /// The agent's cursor as the overlay draws it, at 2x, on a light and a
    /// dark strip, ring at mid-play: for designer's sign-off (--write-cursor).
    static func writeCursor(_ path: String, agent: String) {
        let w = 300, h = 120, k = 2
        guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w * k, pixelsHigh: h * k, bitsPerSample: 8,
                                         samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                         colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { return }
        rep.size = NSSize(width: w, height: h)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        NSColor.white.setFill(); NSRect(x: 0, y: 0, width: w / 2, height: h).fill()
        NSColor(srgbRed: 0.12, green: 0.12, blue: 0.13, alpha: 1).setFill(); NSRect(x: w / 2, y: 0, width: w / 2, height: h).fill()
        for (x, ring) in [(15.0, CGFloat?.none), (165.0, CGFloat?(0.4))] {
            // the view draws flipped, as in the panel: a flipped context
            let base = NSGraphicsContext.current!
            let ctx = base.cgContext
            ctx.saveGState()
            ctx.translateBy(x: CGFloat(x), y: CGFloat(h) - 15)
            ctx.scaleBy(x: 1, y: -1)
            NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
            let v = CursorView(frame: NSRect(x: 0, y: 0, width: 130, height: 90))
            v.name = agent
            v.point = CGPoint(x: 30, y: 25)
            v.ring = ring
            v.draw(v.bounds)
            NSGraphicsContext.current = base
            ctx.restoreGState()
        }
        NSGraphicsContext.restoreGraphicsState()
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
    }

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
