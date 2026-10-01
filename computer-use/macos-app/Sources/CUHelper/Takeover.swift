// Takeover (design §5.2): the user clicks or types in the app an agent
// drives → that agent is paused there (`paused` event, C4/C5), until the
// broker sends `resume`. Our own pid-posted events don't reach a global
// monitor; Engine.userInput also ignores what follows them closely.
import AppKit

enum Takeover {
    static var monitor: Any?

    static func start(engine: Engine) {
        let mask: NSEvent.EventTypeMask = [.leftMouseDown, .rightMouseDown, .otherMouseDown, .keyDown, .scrollWheel]
        monitor = NSEvent.addGlobalMonitorForEvents(matching: mask) { e in
            let pid: pid_t?
            switch e.type {
            case .keyDown:
                pid = NSWorkspace.shared.frontmostApplication?.processIdentifier
            default:
                pid = owner(at: NSEvent.mouseLocation)
            }
            if let p = pid, p != getpid() { engine.userInput(pid: p) }
        }
    }

    /// The pid of the window under a Cocoa screen point (our click-through
    /// overlay is skipped: it is not ours to report).
    static func owner(at p: NSPoint) -> pid_t? {
        let h = NSScreen.screens.first?.frame.height ?? 0
        let cg = CGPoint(x: p.x, y: h - p.y)
        guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else { return nil }
        for w in list {
            guard (w[kCGWindowLayer as String] as? Int) == 0,
                  let pid = w[kCGWindowOwnerPID as String] as? pid_t, pid != getpid(),
                  let b = w[kCGWindowBounds as String] as? NSDictionary,
                  let r = CGRect(dictionaryRepresentation: b), r.contains(cg) else { continue }
            return pid
        }
        return nil
    }
}
