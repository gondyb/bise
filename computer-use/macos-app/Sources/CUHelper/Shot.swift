// screenshot (C1/C5): one window with ScreenCaptureKit (covered windows
// too, never the desktop), or the crop of one element; JPEG, base64.
import AppKit
import ApplicationServices
import CUCore
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

enum Shot {
    static func take(engine: Engine, agent: String, args: [String: Any]) throws -> [String: Any] {
        try engine.needAccessibility()
        if engine.isStopped(agent) { throw engine.stoppedError() }
        guard CGPreflightScreenCaptureAccess() else {
            throw CUError("no_permission", "bise Computer Use has no Screen Recording permission; ask the user to turn it on in /computer-use")
        }
        let t = try engine.resolve(args["target"] as? String)
        let w = try engine.window(t, args["window"] as? String)
        guard let wid = w.windowID, let wf = w.frame else {
            throw CUError("not_found", "this window of \(t.name) can't be captured (no window id)")
        }
        var crop: CGRect? = nil
        if let r = args["ref"] as? String {
            let n = Int(r.hasPrefix("e") ? String(r.dropFirst()) : r) ?? -1
            guard let el = engine.registry(t.target).element(n), el.isAlive else {
                throw CUError("stale_ref", "\(r) is gone (the window changed); take a new snapshot")
            }
            guard let f = el.frame else { throw CUError("not_found", "\(r) has no frame to capture") }
            crop = f.offsetBy(dx: -wf.minX, dy: -wf.minY).intersection(CGRect(origin: .zero, size: wf.size))
            if crop!.isEmpty { throw CUError("not_found", "\(r) is outside its window") }
        }
        let maxWidth = max(64, (args["max_width"] as? NSNumber)?.intValue ?? 1280)
        let image = try capture(windowID: wid, size: wf.size)
        var img = image
        if let c = crop {
            let k = CGFloat(image.width) / wf.width
            let px = CGRect(x: c.minX * k, y: c.minY * k, width: c.width * k, height: c.height * k).integral
            guard let cropped = image.cropping(to: px) else { throw CUError("not_found", "the crop of \(args["ref"] ?? "") is empty") }
            img = cropped
        }
        if img.width > maxWidth { img = scale(img, width: maxWidth) ?? img }
        guard let jpeg = jpeg(img) else { throw CUError("timeout", "could not encode the screenshot") }
        return ["data": jpeg.base64EncodedString(), "mime": "image/jpeg", "width": img.width, "height": img.height]
    }

    /// ScreenCaptureKit is async: bridge it to this worker thread.
    static func capture(windowID: CGWindowID, size: CGSize) throws -> CGImage {
        let sem = DispatchSemaphore(value: 0)
        var out: Result<CGImage, CUError> = .failure(CUError("timeout", "the screenshot took more than 10 s"))
        Task.detached {
            do {
                let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
                guard let win = content.windows.first(where: { $0.windowID == windowID }) else {
                    out = .failure(CUError("not_found", "the window is not capturable (minimized or on no display)"))
                    sem.signal(); return
                }
                let filter = SCContentFilter(desktopIndependentWindow: win)
                let cfg = SCStreamConfiguration()
                let scale = CGFloat(filter.pointPixelScale)
                cfg.width = max(1, Int(win.frame.width * scale))
                cfg.height = max(1, Int(win.frame.height * scale))
                cfg.showsCursor = false
                cfg.ignoreShadowsSingleWindow = true
                let img = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: cfg)
                out = .success(img)
            } catch {
                out = .failure(CUError("no_permission", "the screenshot failed (\(error.localizedDescription)); check Screen Recording in /computer-use"))
            }
            sem.signal()
        }
        if sem.wait(timeout: .now() + 10) == .timedOut { throw CUError("timeout", "the screenshot took more than 10 s") }
        return try out.get()
    }

    static func scale(_ img: CGImage, width: Int) -> CGImage? {
        let height = max(1, Int(Double(img.height) * Double(width) / Double(img.width)))
        guard let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
        ctx.interpolationQuality = .high
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: width, height: height))
        return ctx.makeImage()
    }

    static func jpeg(_ img: CGImage) -> Data? {
        let data = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(data, UTType.jpeg.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dest, img, [kCGImageDestinationLossyCompressionQuality: 0.8] as CFDictionary)
        return CGImageDestinationFinalize(dest) ? data as Data : nil
    }
}
