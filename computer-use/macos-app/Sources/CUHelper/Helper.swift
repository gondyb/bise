// bise Computer Use.app (dev.bise.computer-use, LSUIElement): holds the
// Accessibility and Screen Recording grants and serves C5 on a unix socket.
// The broker starts it with `open -g -a "bise Computer Use" --args --socket <path>`
// so it is its own responsible process (TCC lists "bise Computer Use").
import AppKit
import CUCore

var socketPath = (NSHomeDirectory() as NSString).appendingPathComponent(".bise/run/computer-use-app.sock")
/// --no-takeover (tests): the user's own clicks don't pause the agents
/// (the tests use the simulate_input op instead)
var takeover = true

final class AppDelegate: NSObject, NSApplicationDelegate {
    var server: Server?
    func applicationDidFinishLaunching(_ n: Notification) {
        let s = Server(path: socketPath)
        do { try s.start() } catch {
            FileHandle.standardError.write("bise Computer Use: \(error)\n".data(using: .utf8)!)
            exit(1)
        }
        server = s
        if takeover { Takeover.start(engine: Engine.shared) }
    }
    func applicationWillTerminate(_ n: Notification) { unlink(socketPath) }
}

/// The helper's main. Also exported for the dev host (cu-devhost), which
/// loads this code from a dylib so the TCC grants survive rebuilds.
@_cdecl("cu_helper_main")
public func helperMain() {
    var args = CommandLine.arguments.dropFirst().makeIterator()
    while let a = args.next() {
        switch a {
        case "--socket": if let p = args.next() { socketPath = (p as NSString).expandingTildeInPath }
        case "--no-takeover": takeover = false
        case "--version": print(helperVersion); exit(0)
        case "--write-iconset":
            // the placeholder icon (bundle.sh): the agents' cursor on paper
            if let dir = args.next() { Icon.writeIconset(dir) }
            exit(0)
        case "--write-cursor":
            // the overlay's look as a PNG (for designer)
            if let p = args.next() { Icon.writeCursor(p, agent: args.next() ?? "api-v2") }
            exit(0)
        case "--diag":
            // who TCC charges for this process when started this way
            let d = try! JSONSerialization.data(withJSONObject: Diag.info(), options: [.sortedKeys])
            print(String(data: d, encoding: .utf8)!); exit(0)
        default: break
        }
    }
    if Server.alive(socketPath) { exit(0) }   // one helper per socket
    signal(SIGTERM) { _ in unlink(socketPath); exit(0) }
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    let delegate = AppDelegate()
    app.delegate = delegate
    app.run()
}
