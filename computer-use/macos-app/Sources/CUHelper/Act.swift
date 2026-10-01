// One `act` on an app target (C1): find the element (ref or locator,
// auto-wait), act without raising the app or moving the cursor, diff.
import AppKit
import ApplicationServices
import CUCore

struct Act {
    let engine: Engine
    let agent: String
    let target: AppTarget
    let args: [String: Any]
    let action: String
    let verb: String

    var timeout: TimeInterval { Double((args["timeout_ms"] as? NSNumber)?.intValue ?? 5000) / 1000 }
    var hasElement: Bool { args["ref"] != nil || args["locator"] != nil }

    /// What the summary names (the user-facing line, C1).
    func what(_ n: Node?) -> String {
        if let n = n { return Summary.what(name: n.name.isEmpty ? n.label : n.name, role: n.role) }
        if let l = args["locator"] as? [String: Any] {
            return Summary.what(name: (l["name"] ?? l["text"] ?? l["label"]) as? String, role: l["role"] as? String)
        }
        if let r = args["ref"] as? String { return r }
        return "the window"
    }

    func fail(_ e: CUError, _ n: Node? = nil) -> CUError {
        var e = e
        guard e.summary == nil else { return e }
        let w = what(n)
        switch e.code {
        case "not_found", "ambiguous": e.summary = "couldn't find \(w)"
        case "stale_ref": e.summary = "couldn't \(verb) \(w): the window changed"
        case "needs_front": e.summary = "couldn't \(verb) \(w): \(target.name) needs to be in front"
        case "timeout": e.summary = "couldn't \(verb) \(w): timed out"
        case "stopped": e.summary = "stopped by you"
        case "refused": e.summary = "couldn't use \(target.name): off limits"
        case "no_permission": e.summary = "couldn't \(verb) \(w): no permission"
        default: e.summary = "couldn't \(verb) \(w)"
        }
        return e
    }

    func run() throws -> [String: Any] {
        let window: AXUIElement
        do { window = try engine.window(target, args["window"] as? String) } catch let e as CUError { throw fail(e) }
        let before = Snapshot.text(header: engine.header(target, window), nodes: engine.walk(target, window, budget: 400).nodes)
        var el: AXUIElement? = nil
        var node: Node? = nil
        if hasElement {
            do { (el, node) = try find(window, enabled: !["read", "wait", "hover", "scroll"].contains(action)) }
            catch let e as CUError { throw fail(e) }
        }
        if engine.isStopped(agent) { throw fail(engine.stoppedError(), node) }
        if let el = el, let f = el.frame { point(window, at: CGPoint(x: f.midX, y: f.midY), click: ["click", "check", "select"].contains(action)) }

        var summary = ""
        var changed: String? = nil
        do {
            switch action {
            case "click": try click(el, node); summary = "clicked \(what(node))"
            case "fill": try fill(el ?? focused(window), node); summary = "filled \(what(node))"
            case "type": let n = try type(el ?? focused(window), node); summary = "typed in \(what(n))"
            case "press":
                let keys = try Keys.parse(args["keys"] as? String ?? args["text"] as? String ?? "")
                if let el = el { el.set(kAXFocusedAttribute, kCFBooleanTrue) }
                press(keys)
                summary = "pressed \(args["keys"] as? String ?? args["text"] as? String ?? "")"
            case "select": let v = try select(el, node); summary = "selected \"\(Snapshot.cut(v, 40))\" in \(what(node))"
            case "check": let on = try check(el, node); summary = "\(on ? "checked" : "unchecked") \(what(node))"
            case "hover": try hover(el); summary = "hovered \(what(node))"
            case "scroll": let d = try scroll(el, window); summary = "scrolled \(d)"
            case "close":
                let title = window.title
                try close(window); summary = "closed \(Summary.what(name: title, role: "window"))"
            case "wait": summary = try wait(window, node)
            case "read": changed = read(el, window); summary = "read \(el == nil ? "the window" : what(node))"
            default: throw CUError("bad_args", "unknown action \(action)")
            }
        } catch let e as CUError { throw fail(e, node) }
        summary += " · \(target.name)"

        // let the app redraw, then diff (C1 `changed`)
        if changed == nil {
            Thread.sleep(forTimeInterval: action == "wait" || action == "read" ? 0 : 0.2)
            if window.isAlive, let w2 = try? engine.window(target, args["window"] as? String), CFEqual(w2, window) {
                let after = Snapshot.text(header: engine.header(target, w2), nodes: engine.walk(target, w2, budget: 400).nodes)
                changed = Snapshot.diff(before: before, after: after)
            } else {
                changed = "- window \"\(window.title ?? "")\" (closed)"
            }
        }
        let title = (window.isAlive ? window.title : nil) ?? ""
        return ["ok": true, "title": title, "changed": changed ?? "", "summary": summary]
    }

    // MARK: finding

    /// The element of args.ref or args.locator; a locator waits (≤ timeout)
    /// for exactly one match, enabled when the action needs it.
    func find(_ window: AXUIElement, enabled: Bool) throws -> (AXUIElement, Node?) {
        if let r = args["ref"] as? String {
            let n = Int(r.hasPrefix("e") ? String(r.dropFirst()) : r) ?? -1
            guard let el = engine.registry(target.target).element(n) else {
                throw CUError("stale_ref", "\(r) is not a ref of \(target.name); take a snapshot first")
            }
            guard el.isAlive else { throw CUError("stale_ref", "\(r) is gone (the window changed); take a new snapshot") }
            let w = Walker.walk(el, budget: 1) { engine.registry(target.target).ref($0) }
            return (el, w.nodes.first)
        }
        guard let lj = args["locator"] as? [String: Any] else { throw CUError("bad_args", "give a ref or a locator") }
        let loc = try Locator(json: lj)
        let deadline = Date().addingTimeInterval(timeout)
        while true {
            if engine.isStopped(agent) { throw engine.stoppedError() }
            let w = engine.walk(target, window, budget: 5000)
            var last: CUError
            switch loc.resolve(w.nodes) {
            case .success(let i):
                if !enabled || !w.nodes[i].disabled { return (w.elements[i], w.nodes[i]) }
                last = CUError("timeout", "\(Snapshot.line(w.nodes[i])) stayed disabled for \(Int(timeout * 1000)) ms")
            case .failure(let e):
                last = e
                if e.code == "ambiguous" { throw e }   // waiting will not make it unique
            }
            if Date() >= deadline { throw last }
            Thread.sleep(forTimeInterval: 0.1)
        }
    }

    func focused(_ window: AXUIElement) throws -> AXUIElement {
        if let f = target.element.element(kAXFocusedUIElementAttribute) { return f }
        throw CUError("not_found", "nothing has the focus in \(target.name); give a ref or a locator")
    }

    /// The overlay cursor glides there (only drawn when the window shows).
    func point(_ window: AXUIElement, at p: CGPoint, click: Bool) {
        guard let wid = window.windowID, let wf = window.frame else { return }
        let agent = self.agent
        DispatchQueue.main.async { Overlay.shared.show(agent: agent, windowID: wid, windowFrame: wf, point: p, click: click) }
        Thread.sleep(forTimeInterval: click ? 0.18 : 0.15)
    }

    // MARK: actions

    static let pressable = ["AXPress", "AXConfirm", "AXPick"]

    func click(_ el: AXUIElement?, _ n: Node?) throws {
        guard let el = el else { throw CUError("bad_args", "click needs a ref or a locator") }
        let acts = el.actions
        if ["textbox", "searchbox"].contains(n?.role ?? ""), el.settable(kAXFocusedAttribute) {
            el.set(kAXFocusedAttribute, kCFBooleanTrue); return
        }
        for a in Act.pressable where acts.contains(a) {
            let e = el.perform(a)
            // a press that opens a modal returns cannotComplete after the fact
            if e == .success || e == .cannotComplete { return }
        }
        if el.settable(kAXSelectedAttribute) { el.set(kAXSelectedAttribute, kCFBooleanTrue); return }
        throw CUError("needs_front", "\(target.name) takes no click on this element without being in front; ask the user to bring it forward")
    }

    func fill(_ el: AXUIElement, _ n: Node?) throws {
        let text = args["value"] as? String ?? args["text"] as? String ?? ""
        guard el.settable(kAXValueAttribute) else {
            throw CUError("needs_front", "this element of \(target.name) can't be filled in the background; ask the user to bring it forward")
        }
        el.set(kAXFocusedAttribute, kCFBooleanTrue)
        let e = el.set(kAXValueAttribute, text as CFString)
        if e != .success { throw CUError("needs_front", "\(target.name) refused the new value (\(e.rawValue)); ask the user to bring it forward") }
    }

    /// Text at the caret: AXSelectedText (no events), else keys posted to the pid.
    func type(_ el: AXUIElement, _ n: Node?) throws -> Node? {
        guard let text = args["text"] as? String, !text.isEmpty else { throw CUError("bad_args", "type needs text") }
        let node = n ?? Walker.walk(el, budget: 1) { engine.registry(target.target).ref($0) }.nodes.first
        if hasElement { el.set(kAXFocusedAttribute, kCFBooleanTrue) }
        let before = el.string(kAXValueAttribute)
        if el.settable(kAXSelectedTextAttribute), el.set(kAXSelectedTextAttribute, text as CFString) == .success {
            if before == nil || el.string(kAXValueAttribute) != before { return node }
        }
        postText(text)
        Thread.sleep(forTimeInterval: 0.1)
        if let b = before, el.string(kAXValueAttribute) == b {
            throw CUError("needs_front", "\(target.name) ignores typing in the background; ask the user to bring it forward")
        }
        return node
    }

    func check(_ el: AXUIElement?, _ n: Node?) throws -> Bool {
        guard let el = el else { throw CUError("bad_args", "check needs a ref or a locator") }
        let want: Bool
        switch args["value"] {
        case let b as Bool: want = b
        case let s as String: want = !["false", "0", "off", "no"].contains(s.lowercased())
        case let n as NSNumber: want = n.boolValue
        default: want = true
        }
        let state = { (el.attr(kAXValueAttribute) as? NSNumber)?.intValue == 1 }
        if state() == want { return want }
        try click(el, n)
        Thread.sleep(forTimeInterval: 0.1)
        if state() != want { throw CUError("needs_front", "the checkbox did not change; \(target.name) may need to be in front") }
        return want
    }

    /// combobox (pop-up): open its menu and press the item; list/table: select the row.
    func select(_ el: AXUIElement?, _ n: Node?) throws -> String {
        guard let el = el else { throw CUError("bad_args", "select needs a ref or a locator") }
        guard let want = args["value"] as? String ?? args["text"] as? String else { throw CUError("bad_args", "select needs value") }
        let role = el.role ?? ""
        if role == "AXComboBox", el.settable(kAXValueAttribute) {
            el.set(kAXValueAttribute, want as CFString); return want
        }
        if role == "AXPopUpButton" || el.actions.contains("AXShowMenu") {
            el.perform(role == "AXPopUpButton" ? "AXPress" : "AXShowMenu")
            Thread.sleep(forTimeInterval: 0.15)
            let items = menuItems(el)
            let titles = items.map { $0.title ?? "" }
            let i = titles.firstIndex(of: want) ?? titles.firstIndex { $0.localizedCaseInsensitiveContains(want) }
            defer {
                // never leave a menu open on the user's screen
                Thread.sleep(forTimeInterval: 0.1)
                for m in el.children where m.role == "AXMenu" { m.perform("AXCancel") }
            }
            if let i = i { items[i].perform("AXPress"); return titles[i] }
            throw CUError("not_found", "no option \"\(want)\"", candidates: titles.filter { !$0.isEmpty }.prefix(10).map { "- menuitem \"\($0)\"" })
        }
        // a list, table or outline: the row whose text matches
        let rows = el.elements(kAXRowsAttribute)
        for r in rows {
            let w = Walker.walk(r, budget: 50) { engine.registry(target.target).ref($0) }
            if w.nodes.contains(where: { $0.text == want || $0.name == want }) || Snapshot.readText(w.nodes).contains(want) {
                if r.set(kAXSelectedAttribute, kCFBooleanTrue) == .success { return want }
                if el.set(kAXSelectedRowsAttribute, [r] as CFArray) == .success { return want }
            }
        }
        throw CUError("not_found", "no option \"\(want)\" in this element")
    }

    func menuItems(_ el: AXUIElement) -> [AXUIElement] {
        var out: [AXUIElement] = []
        for m in el.children where m.role == "AXMenu" {
            out += m.children.filter { $0.role == "AXMenuItem" }
        }
        return out
    }

    func hover(_ el: AXUIElement?) throws {
        guard let f = el?.frame else { throw CUError("bad_args", "hover needs a ref or a locator") }
        let p = CGPoint(x: f.midX, y: f.midY)
        engine.lastPost = Date()
        CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left)?.postToPid(target.pid)
    }

    /// AX scroll bars first (no events); else a wheel event posted to the pid.
    func scroll(_ el: AXUIElement?, _ window: AXUIElement) throws -> String {
        let dir = (args["direction"] as? String ?? "down").lowercased()
        guard ["up", "down", "left", "right"].contains(dir) else { throw CUError("bad_args", "direction is up, down, left or right") }
        let vertical = dir == "up" || dir == "down"
        guard let area = scrollArea(from: el, window) else {
            throw CUError("not_found", "nothing scrolls in this window of \(target.name)")
        }
        let visible = area.frame ?? .zero
        let amount = (args["amount"] as? NSNumber)?.doubleValue ?? Double(vertical ? visible.height : visible.width) * 0.8
        let sign: Double = (dir == "down" || dir == "right") ? 1 : -1
        let content = area.elements(kAXContentsAttribute).compactMap { $0.frame }.reduce(CGRect.null) { $0.union($1) }
        let bar = area.element(vertical ? kAXVerticalScrollBarAttribute : kAXHorizontalScrollBarAttribute)
        let range = vertical ? content.height - visible.height : content.width - visible.width
        if let bar = bar, !content.isNull, range > 0, bar.settable(kAXValueAttribute),
           let cur = (bar.attr(kAXValueAttribute) as? NSNumber)?.doubleValue {
            let next = min(1, max(0, cur + sign * amount / Double(range)))
            if bar.set(kAXValueAttribute, NSNumber(value: next)) == .success { return dir }
        }
        let dy = vertical ? Int32(-sign * amount) : 0, dx = vertical ? 0 : Int32(-sign * amount)
        guard let ev = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0) else {
            throw CUError("needs_front", "\(target.name) can't be scrolled in the background")
        }
        ev.location = CGPoint(x: visible.midX, y: visible.midY)
        engine.lastPost = Date()
        ev.postToPid(target.pid)
        return dir
    }

    func scrollArea(from el: AXUIElement?, _ window: AXUIElement) -> AXUIElement? {
        var cur = el
        while let c = cur {
            if c.role == "AXScrollArea" { return c }
            cur = c.element(kAXParentAttribute)
        }
        // the window's first scroll area, breadth first
        var queue = [window]
        var seen = 0
        while !queue.isEmpty && seen < 2000 {
            let e = queue.removeFirst(); seen += 1
            if e.role == "AXScrollArea" { return e }
            queue += e.children
        }
        return nil
    }

    func close(_ window: AXUIElement) throws {
        guard let b = window.element(kAXCloseButtonAttribute), b.perform("AXPress") == .success else {
            throw CUError("needs_front", "this window of \(target.name) has no close button the helper can press")
        }
    }

    func wait(_ window: AXUIElement, _ n: Node?) throws -> String {
        if hasElement { return "waited for \(what(n))" }   // find() already waited
        if let text = args["text"] as? String, !text.isEmpty {
            let deadline = Date().addingTimeInterval(timeout)
            while true {
                if engine.isStopped(agent) { throw engine.stoppedError() }
                let all = Snapshot.readText(engine.walk(target, window, budget: 5000).nodes, limit: 1_000_000)
                if all.localizedCaseInsensitiveContains(text) { return "waited for \"\(Snapshot.cut(text, 40))\"" }
                if Date() >= deadline { throw CUError("timeout", "\"\(text)\" did not show in \(Int(timeout * 1000)) ms") }
                Thread.sleep(forTimeInterval: 0.1)
            }
        }
        let ms = min((args["amount"] as? NSNumber)?.doubleValue ?? 1000, timeout * 1000)
        let end = Date().addingTimeInterval(ms / 1000)
        while Date() < end {
            if engine.isStopped(agent) { throw engine.stoppedError() }
            Thread.sleep(forTimeInterval: min(0.1, max(0, end.timeIntervalSinceNow)))
        }
        return "waited \(Int(ms)) ms"
    }

    func read(_ el: AXUIElement?, _ window: AXUIElement) -> String {
        let root = el ?? window
        let w = Walker.walk(root, budget: 5000) { engine.registry(target.target).ref($0) }
        if let el = el, w.nodes.count <= 1 {
            if let n = w.nodes.first, !n.secure { return Snapshot.cut(n.value ?? n.text, 4000) }
            _ = el
            return ""
        }
        return Snapshot.readText(w.nodes)
    }

    // MARK: events to the pid (never global: the cursor and the front app stay)

    func press(_ chords: [KeyChord]) {
        engine.lastPost = Date()
        for k in chords {
            var flags: CGEventFlags = []
            if k.shift { flags.insert(.maskShift) }
            if k.control { flags.insert(.maskControl) }
            if k.option { flags.insert(.maskAlternate) }
            if k.command { flags.insert(.maskCommand) }
            for down in [true, false] {
                guard let e = CGEvent(keyboardEventSource: nil, virtualKey: k.code, keyDown: down) else { continue }
                e.flags = flags
                e.postToPid(target.pid)
            }
            Thread.sleep(forTimeInterval: 0.02)
        }
    }

    func postText(_ text: String) {
        engine.lastPost = Date()
        let units = Array(text.utf16)
        var i = 0
        while i < units.count {
            let chunk = Array(units[i..<min(i + 16, units.count)])
            for down in [true, false] {
                guard let e = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down) else { continue }
                chunk.withUnsafeBufferPointer { e.keyboardSetUnicodeString(stringLength: chunk.count, unicodeString: $0.baseAddress) }
                e.postToPid(target.pid)
            }
            i += 16
            Thread.sleep(forTimeInterval: 0.01)
        }
    }
}
