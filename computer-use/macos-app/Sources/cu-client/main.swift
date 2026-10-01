// A C5 client (the broker's side, for tests and by hand):
//   cu-client --socket <path> [--timeout <s>] '<json line>' ...
// prints the hello, then for each line sent its reply (requests with an
// "id" wait for the matching reply; control lines don't). Events that
// arrive meanwhile are printed too. --wait-event <name> <s>: after the
// lines, wait up to <s> seconds for that event.
import AppKit
import Foundation

// --probe: what the tests compare before and after (the front app and the
// real cursor must not change)
if CommandLine.arguments.dropFirst().first == "--probe" {
    let front = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? ""
    let p = CGEvent(source: nil)?.location ?? .zero
    print("{\"front\":\"\(front)\",\"cursor\":[\(Int(p.x)),\(Int(p.y))]}")
    exit(0)
}

var path = (NSHomeDirectory() as NSString).appendingPathComponent(".bise/run/computer-use-app.sock")
var timeout: TimeInterval = 30
var lines: [String] = []
var waitEvent: (String, TimeInterval)? = nil
var it = CommandLine.arguments.dropFirst().makeIterator()
while let a = it.next() {
    switch a {
    case "--socket": path = it.next() ?? path
    case "--timeout": timeout = Double(it.next() ?? "") ?? timeout
    case "--wait-event": if let n = it.next(), let s = Double(it.next() ?? "") { waitEvent = (n, s) }
    default: lines.append(a)
    }
}

func die(_ s: String) -> Never { FileHandle.standardError.write((s + "\n").data(using: .utf8)!); exit(2) }

var addr = sockaddr_un()
addr.sun_family = sa_family_t(AF_UNIX)
let pb = Array(path.utf8)
guard pb.count < MemoryLayout.size(ofValue: addr.sun_path) else { die("socket path too long") }
withUnsafeMutableBytes(of: &addr.sun_path) { p in for (i, b) in pb.enumerated() { p[i] = b }; p[pb.count] = 0 }
let fd = socket(AF_UNIX, SOCK_STREAM, 0)
let rc = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) } }
guard rc == 0 else { die("no helper on \(path)") }

var buf = Data()
var chunk = [UInt8](repeating: 0, count: 65536)
/// The next line, or nil at the deadline / EOF.
func readLine(until deadline: Date) -> String? {
    while true {
        if let nl = buf.firstIndex(of: 0x0A) {
            let l = buf.subdata(in: buf.startIndex..<nl)
            buf.removeSubrange(buf.startIndex...nl)
            return String(data: l, encoding: .utf8)
        }
        let left = deadline.timeIntervalSinceNow
        if left <= 0 { return nil }
        var pfd = pollfd(fd: fd, events: Int16(POLLIN), revents: 0)
        if poll(&pfd, 1, Int32(min(left, 1) * 1000)) <= 0 { continue }
        let n = read(fd, &chunk, chunk.count)
        if n <= 0 { return nil }
        buf.append(contentsOf: chunk[0..<n])
    }
}
func out(_ s: String) { print(s); fflush(stdout) }

guard let hello = readLine(until: Date().addingTimeInterval(5)) else { die("no hello") }
out(hello)
for l in lines {
    let data = (l + "\n").data(using: .utf8)!
    _ = data.withUnsafeBytes { write(fd, $0.baseAddress!, data.count) }
    guard let obj = (try? JSONSerialization.jsonObject(with: Data(l.utf8))) as? [String: Any], let id = obj["id"] else { continue }
    let deadline = Date().addingTimeInterval(timeout)
    while true {
        guard let r = readLine(until: deadline) else { die("no reply to \(l)") }
        out(r)
        if let o = (try? JSONSerialization.jsonObject(with: Data(r.utf8))) as? [String: Any], let rid = o["id"],
           "\(rid)" == "\(id)" { break }
    }
}
if let (name, s) = waitEvent {
    let deadline = Date().addingTimeInterval(s)
    while let r = readLine(until: deadline) {
        out(r)
        if r.contains("\"event\":\"\(name)\"") { exit(0) }
    }
    die("no \(name) event in \(s) s")
}
