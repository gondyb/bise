// Thin helpers over AXUIElement, and the walk that turns a window into
// C2 nodes with refs.
import AppKit
import ApplicationServices
import CUCore

/// The window id behind an AX window (private but stable since 10.x;
/// every window tool uses it).
@_silgen_name("_AXUIElementGetWindow")
func _AXUIElementGetWindow(_ element: AXUIElement, _ id: UnsafeMutablePointer<CGWindowID>) -> AXError

extension AXUIElement {
    func raw(_ name: String) -> (AXError, CFTypeRef?) {
        var v: CFTypeRef?
        let e = AXUIElementCopyAttributeValue(self, name as CFString, &v)
        return (e, v)
    }
    func attr(_ name: String) -> CFTypeRef? {
        let (e, v) = raw(name); return e == .success ? v : nil
    }
    func string(_ name: String) -> String? {
        guard let v = attr(name) else { return nil }
        if let s = v as? String { return s }
        if let a = v as? NSAttributedString { return a.string }
        return nil
    }
    func bool(_ name: String) -> Bool? { (attr(name) as? NSNumber)?.boolValue }
    func element(_ name: String) -> AXUIElement? {
        guard let v = attr(name), CFGetTypeID(v) == AXUIElementGetTypeID() else { return nil }
        return (v as! AXUIElement)
    }
    func elements(_ name: String) -> [AXUIElement] {
        guard let v = attr(name), let a = v as? [AnyObject] else { return [] }
        return a.compactMap { CFGetTypeID($0) == AXUIElementGetTypeID() ? ($0 as! AXUIElement) : nil }
    }
    var children: [AXUIElement] { elements(kAXChildrenAttribute) }
    var role: String? { string(kAXRoleAttribute) }
    var title: String? { string(kAXTitleAttribute) }
    func point(_ name: String) -> CGPoint? {
        guard let v = attr(name), CFGetTypeID(v) == AXValueGetTypeID() else { return nil }
        var p = CGPoint.zero
        return AXValueGetValue(v as! AXValue, .cgPoint, &p) ? p : nil
    }
    func size(_ name: String) -> CGSize? {
        guard let v = attr(name), CFGetTypeID(v) == AXValueGetTypeID() else { return nil }
        var s = CGSize.zero
        return AXValueGetValue(v as! AXValue, .cgSize, &s) ? s : nil
    }
    /// Screen frame, top-left origin (AX / CG global coordinates).
    var frame: CGRect? {
        guard let p = point(kAXPositionAttribute), let s = size(kAXSizeAttribute) else { return nil }
        return CGRect(origin: p, size: s)
    }
    var actions: [String] {
        var names: CFArray?
        guard AXUIElementCopyActionNames(self, &names) == .success, let a = names as? [String] else { return [] }
        return a
    }
    @discardableResult func perform(_ action: String) -> AXError { AXUIElementPerformAction(self, action as CFString) }
    @discardableResult func set(_ name: String, _ value: CFTypeRef) -> AXError {
        AXUIElementSetAttributeValue(self, name as CFString, value)
    }
    func settable(_ name: String) -> Bool {
        var b: DarwinBoolean = false
        return AXUIElementIsAttributeSettable(self, name as CFString, &b) == .success && b.boolValue
    }
    /// Gone (window closed, view replaced): its refs are stale.
    var isAlive: Bool {
        let (e, _) = raw(kAXRoleAttribute)
        return e != .invalidUIElement
    }
    var windowID: CGWindowID? {
        var id: CGWindowID = 0
        return _AXUIElementGetWindow(self, &id) == .success && id != 0 ? id : nil
    }
    var pid: pid_t {
        var p: pid_t = 0
        AXUIElementGetPid(self, &p)
        return p
    }
}

/// An AXUIElement as a dictionary key (CFEqual / CFHash).
struct AXKey: Hashable {
    let e: AXUIElement
    static func == (a: AXKey, b: AXKey) -> Bool { CFEqual(a.e, b.e) }
    func hash(into h: inout Hasher) { h.combine(CFHash(e)) }
}

/// What one walk of a window gives: the C2 nodes and their elements.
struct Walk {
    var nodes: [Node] = []
    var elements: [AXUIElement] = []
    var truncated = false
}

enum Walker {
    static let batch: [String] = [kAXRoleAttribute, kAXSubroleAttribute, kAXTitleAttribute,
        kAXDescriptionAttribute, kAXValueAttribute, kAXEnabledAttribute, kAXFocusedAttribute,
        kAXChildrenAttribute, kAXExpandedAttribute, kAXPlaceholderValueAttribute,
        kAXTitleUIElementAttribute, kAXSelectedAttribute]
    /// Containers that say nothing alone: their children take their place.
    static let transparent: Set<String> = ["AXGroup", "AXScrollArea", "AXSplitGroup", "AXLayoutArea",
        "AXUnknown", "AXLayoutItem", "AXMatte", "AXRuler", "AXGrowArea"]
    static let skipped: Set<String> = ["AXSplitter", "AXGrowArea"]
    /// Shown, but their parts (arrows, thumb) are noise.
    static let leaves: Set<String> = ["AXScrollBar"]

    static func str(_ v: Any?) -> String? {
        guard let v = v else { return nil }
        if let s = v as? String { return s }
        if let a = v as? NSAttributedString { return a.string }
        return nil
    }

    /// The window's nodes in document order, at most `budget` printed;
    /// `ref` gives each node its ref (the caller's registry keeps them stable).
    static func walk(_ window: AXUIElement, budget: Int, ref: (AXUIElement) -> Int) -> Walk {
        var w = Walk()
        var visited = 0
        func visit(_ e: AXUIElement, depth: Int) {
            if w.nodes.count >= budget { w.truncated = true; return }
            visited += 1
            if visited > budget * 20 { w.truncated = true; return }
            var values: CFArray?
            guard AXUIElementCopyMultipleAttributeValues(e, batch as CFArray, AXCopyMultipleAttributeOptions(rawValue: 0), &values) == .success,
                  let a = values as? [Any], a.count == batch.count else { return }
            func at(_ i: Int) -> Any? {
                let v = a[i] as AnyObject
                if CFGetTypeID(v) == AXValueGetTypeID(), AXValueGetType(v as! AXValue) == .axError { return nil }
                return v
            }
            let axRole = str(at(0)) ?? "AXUnknown"
            if skipped.contains(axRole) { return }
            let subrole = str(at(1))
            let title = str(at(2)) ?? ""
            let desc = str(at(3)) ?? ""
            let rawValue = at(4)
            let children: [AXUIElement] = ((at(7) as? [AnyObject]) ?? []).compactMap {
                CFGetTypeID($0) == AXUIElementGetTypeID() ? ($0 as! AXUIElement) : nil
            }
            let role = Roles.aria(role: axRole, subrole: subrole)
            var name = title.isEmpty ? desc : title
            var label = ""
            if let tu = at(10), CFGetTypeID(tu as AnyObject) == AXUIElementGetTypeID() {
                let t = tu as! AXUIElement
                label = t.string(kAXValueAttribute) ?? t.title ?? ""
            }
            let placeholder = str(at(9)) ?? ""
            if name.isEmpty { name = label.isEmpty ? placeholder : label }
            var value: String? = nil
            var checked = false
            if let s = str(rawValue) {
                if role == "text" && name.isEmpty { name = s } else if role != "text" { value = s }
            } else if let n = rawValue as? NSNumber {
                if role == "checkbox" || role == "radio" || axRole == "AXMenuItem" { checked = n.intValue == 1 }
                else if role != "text" {
                    // a scroll bar's 0…1 position, a slider's value: 2 decimals at most
                    let d = n.doubleValue
                    value = d == d.rounded() ? String(Int(d)) : String(format: "%.2f", d)
                }
            }
            if role == "checkbox" || role == "radio" { value = nil }
            let isTransparent = transparent.contains(axRole) && name.isEmpty && value == nil
            let isEmptyText = role == "text" && name.isEmpty
            if isTransparent || isEmptyText {
                for c in children { visit(c, depth: depth) }
                return
            }
            var n = Node(role: role, name: name, value: value, depth: depth)
            n.label = label.isEmpty ? placeholder : label
            n.secure = subrole == "AXSecureTextField"
            if n.secure { n.value = nil }
            n.disabled = (at(5) as? NSNumber).map { !$0.boolValue } ?? false
            n.focused = (at(6) as? NSNumber)?.boolValue ?? false
            n.expanded = (at(8) as? NSNumber)?.boolValue ?? false
            n.checked = checked || ((at(11) as? NSNumber)?.boolValue ?? false && ["row", "cell", "outlinerow", "tab"].contains(role))
            n.ref = ref(e)
            w.nodes.append(n)
            w.elements.append(e)
            if leaves.contains(axRole) { return }
            // a long table or outline: its visible rows only (what a scroll
            // changes; a mail box can hold 100k rows), no column objects
            if axRole == "AXTable" || axRole == "AXOutline" {
                let visible = Set(e.elements(kAXVisibleRowsAttribute).map { AXKey(e: $0) })
                for c in children {
                    let r = c.role ?? ""
                    if r == "AXColumn" { continue }
                    if r == "AXRow" && !visible.isEmpty && !visible.contains(AXKey(e: c)) { continue }
                    visit(c, depth: depth + 1)
                }
                return
            }
            for c in children { visit(c, depth: depth + 1) }
        }
        visit(window, depth: 0)
        return w
    }
}
