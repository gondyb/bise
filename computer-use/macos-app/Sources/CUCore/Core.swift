// The helper's pure logic: no AX, no AppKit, so cu-check can test it on
// fake trees. Contracts: C1 (errors, locators, summary), C2 (snapshot text).
import Foundation

// MARK: errors (C1)

public struct CUError: Error {
    public var code: String
    public var message: String
    public var candidates: [String]? = nil
    /// act only: the user-facing failure line, no ✗ (C1, m_3630)
    public var summary: String? = nil
    public init(_ code: String, _ message: String, candidates: [String]? = nil, summary: String? = nil) {
        self.code = code; self.message = message; self.candidates = candidates; self.summary = summary
    }
    public var json: [String: Any] {
        var e: [String: Any] = ["code": code, "message": message]
        if let c = candidates { e["candidates"] = c }
        if let s = summary { e["summary"] = s }
        return e
    }
}

// MARK: roles (C2)

public enum Roles {
    static let map: [String: String] = [
        "AXButton": "button", "AXTextField": "textbox", "AXTextArea": "textbox",
        "AXSearchField": "searchbox", "AXCheckBox": "checkbox", "AXRadioButton": "radio",
        "AXPopUpButton": "combobox", "AXMenuItem": "menuitem", "AXLink": "link",
        "AXStaticText": "text", "AXImage": "img", "AXTabGroup": "tablist",
        "AXSlider": "slider", "AXWindow": "window", "AXGroup": "group",
    ]
    /// The ARIA name of an AX role (subrole first: a search field is an
    /// AXTextField with the AXSearchField subrole).
    public static func aria(role: String, subrole: String? = nil) -> String {
        if subrole == "AXSearchField" { return "searchbox" }
        if let r = map[role] { return r }
        return role.hasPrefix("AX") ? String(role.dropFirst(2)).lowercased() : role.lowercased()
    }
}

// MARK: the tree a snapshot prints

public struct Node {
    public var role: String          // ARIA name (C2)
    public var name: String
    public var value: String?
    public var label: String = ""    // the AXTitleUIElement / placeholder text
    public var depth: Int = 0
    public var ref: Int? = nil
    public var secure = false
    public var disabled = false, focused = false, checked = false, expanded = false
    public init(role: String, name: String = "", value: String? = nil, depth: Int = 0) {
        self.role = role; self.name = name; self.value = value; self.depth = depth
    }
    /// The text a `text` locator and `read` see: the name, else the value.
    public var text: String {
        if secure { return name }
        if role == "text" || name.isEmpty { return value ?? name }
        return name
    }
}

public enum Snapshot {
    public static func cut(_ s: String, _ n: Int) -> String {
        s.count <= n ? s : String(s.prefix(n - 1)) + "…"
    }
    static func clean(_ s: String) -> String {
        s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"")
            .replacingOccurrences(of: "\r\n", with: " ").replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\t", with: " ")
    }
    /// One C2 line, without the indentation.
    public static func line(_ n: Node) -> String {
        var s = "- " + n.role
        if !n.name.isEmpty { s += " \"" + clean(cut(n.name, 120)) + "\"" }
        if let r = n.ref { s += " [e\(r)]" }
        if n.secure { s += " value=\"•••\"" }
        else if let v = n.value, !v.isEmpty, n.role != "text" || !n.name.isEmpty { s += " value=\"" + clean(cut(v, 80)) + "\"" }
        if n.disabled { s += " (disabled)" }
        if n.focused { s += " (focused)" }
        if n.checked { s += " (checked)" }
        if n.expanded { s += " (expanded)" }
        return s
    }
    public static func text(header: String, nodes: [Node]) -> String {
        var out = "# " + header
        for n in nodes { out += "\n" + String(repeating: "  ", count: n.depth) + line(n) }
        return out
    }
    /// `changed` (C1): the lines that came or went, ≤ 20, "" when none.
    public static func diff(before: String, after: String, limit: Int = 20) -> String {
        func lines(_ s: String) -> [String] {
            s.split(separator: "\n", omittingEmptySubsequences: true).dropFirst().map { $0.trimmingCharacters(in: .whitespaces) }
        }
        var count: [String: Int] = [:]
        for l in lines(before) { count[l, default: 0] += 1 }
        var added: [String] = []
        for l in lines(after) {
            if let c = count[l], c > 0 { count[l] = c - 1 } else { added.append("+ " + l) }
        }
        var removed: [String] = []
        for l in lines(before) where (count[l] ?? 0) > 0 { removed.append("- " + l); count[l]! -= 1 }
        let all = removed + added
        if all.count <= limit { return all.joined(separator: "\n") }
        return (all.prefix(limit - 1) + ["… \(all.count - limit + 1) more lines"]).joined(separator: "\n")
    }
    /// `read` without a target: the window's texts, one per line, ≤ 4000 chars.
    public static func readText(_ nodes: [Node], limit: Int = 4000) -> String {
        var out: [String] = []
        var seen = Set<String>()
        for n in nodes where !n.secure {
            let t: String
            if n.role == "text" { t = n.text }
            else if let v = n.value, !v.isEmpty, ["textbox", "searchbox", "combobox"].contains(n.role) { t = v }
            else { continue }
            let tt = t.trimmingCharacters(in: .whitespacesAndNewlines)
            if tt.isEmpty || (n.role == "text" && seen.contains(tt)) { continue }
            seen.insert(tt); out.append(tt)
        }
        let s = out.joined(separator: "\n")
        return s.count <= limit ? s : String(s.prefix(limit))
    }
}

// MARK: locators (C1)

public struct Locator {
    public var role: String?, name: String?, nameRe: NSRegularExpression?
    public var text: String?, textRe: NSRegularExpression?, label: String?
    public var exact = false
    public var nth: Int?

    /// "Anker.*2 m/i": the flags after the last "/", when they are flags.
    public static func regex(_ src: String) throws -> NSRegularExpression {
        var body = src, flags = ""
        if let slash = src.lastIndex(of: "/") {
            let tail = src[src.index(after: slash)...]
            if tail.allSatisfy({ "gimsuy".contains($0) }) { body = String(src[..<slash]); flags = String(tail) }
        }
        var opts: NSRegularExpression.Options = []
        if flags.contains("i") { opts.insert(.caseInsensitive) }
        if flags.contains("s") { opts.insert(.dotMatchesLineSeparators) }
        if flags.contains("m") { opts.insert(.anchorsMatchLines) }
        do { return try NSRegularExpression(pattern: body, options: opts) }
        catch { throw CUError("bad_args", "the regex /\(body)/ does not parse; fix it and try again") }
    }

    public init(json: [String: Any]) throws {
        role = json["role"] as? String
        name = json["name"] as? String
        text = json["text"] as? String
        label = json["label"] as? String
        exact = json["exact"] as? Bool ?? false
        nth = (json["nth"] as? NSNumber)?.intValue
        if let r = json["name_re"] as? String { nameRe = try Locator.regex(r) }
        if let r = json["text_re"] as? String { textRe = try Locator.regex(r) }
        if role == nil && name == nil && nameRe == nil && text == nil && textRe == nil && label == nil {
            throw CUError("bad_args", "the locator is empty; give it a role, name, text or label")
        }
    }

    func same(_ have: String, _ want: String) -> Bool {
        if exact { return have == want }
        let norm = { (s: String) in s.lowercased().split(whereSeparator: \.isWhitespace).joined(separator: " ") }
        return norm(have).contains(norm(want))
    }
    static func search(_ re: NSRegularExpression, _ s: String) -> Bool {
        re.firstMatch(in: s, range: NSRange(s.startIndex..., in: s)) != nil
    }

    public func matches(_ n: Node) -> Bool {
        if let r = role, n.role != r { return false }
        if let w = name, !same(n.name, w) { return false }
        if let re = nameRe, !Locator.search(re, n.name) { return false }
        if let w = text, !(same(n.text, w)) { return false }
        if let re = textRe, !Locator.search(re, n.text) { return false }
        if let w = label, !(same(n.label, w) || (!n.label.isEmpty ? false : same(n.name, w))) { return false }
        return true
    }

    /// The node indexes that match; with `nth`, that one (negative from the end).
    public func resolve(_ nodes: [Node]) -> Result<Int, CUError> {
        let hits = nodes.indices.filter { matches(nodes[$0]) }
        let lines = hits.prefix(10).map { Snapshot.line(nodes[$0]) }
        if hits.isEmpty { return .failure(CUError("not_found", "nothing matches \(describe()); take a snapshot and pick a ref")) }
        if let k = nth {
            let i = k < 0 ? hits.count + k : k
            guard i >= 0 && i < hits.count else {
                return .failure(CUError("not_found", "\(describe()) matches \(hits.count), there is no nth \(k)", candidates: Array(lines)))
            }
            return .success(hits[i])
        }
        if hits.count > 1 {
            return .failure(CUError("ambiguous", "\(hits.count) elements match \(describe()); add nth or pick a ref", candidates: Array(lines)))
        }
        return .success(hits[0])
    }

    public func describe() -> String {
        var parts: [String] = []
        if let r = role { parts.append("role \(r)") }
        if let n = name { parts.append("name \"\(n)\"") }
        if let re = nameRe { parts.append("name /\(re.pattern)/") }
        if let t = text { parts.append("text \"\(t)\"") }
        if let re = textRe { parts.append("text /\(re.pattern)/") }
        if let l = label { parts.append("label \"\(l)\"") }
        return parts.joined(separator: ", ")
    }
    /// What the user-facing summary names: the name, else the text, else the label.
    public var shortName: String? { name ?? text ?? label }
}

// MARK: refusals (design §5.2; the broker checks first)

public enum Refusals {
    public static let bundles: [String: String] = [
        "com.apple.Terminal": "Terminal", "com.mitchellh.ghostty": "Ghostty",
        "com.googlecode.iterm2": "iTerm2", "com.github.wez.wezterm": "WezTerm",
        "net.kovidgoyal.kitty": "kitty", "org.alacritty": "Alacritty", "io.alacritty": "Alacritty",
        "dev.warp.Warp-Stable": "Warp", "dev.warp.Warp": "Warp",
        "com.apple.keychainaccess": "Keychain Access", "com.apple.Passwords": "Passwords",
        "com.1password.1password": "1Password", "com.agilebits.onepassword7": "1Password",
        "com.agilebits.onepassword-osx": "1Password", "com.bitwarden.desktop": "Bitwarden",
        "com.dashlane.dashlanephonefinal": "Dashlane", "com.lastpass.LastPass": "LastPass",
        "org.keepassxc.keepassxc": "KeePassXC", "in.sinew.Enpass-Desktop": "Enpass",
        "me.proton.pass.electron": "Proton Pass", "com.apple.loginwindow": "loginwindow",
        "com.apple.SecurityAgent": "SecurityAgent",
    ]
    /// System Settings' Privacy & Security pane, by its window title
    /// (English and the user's languages we know).
    public static let privacyTitles = ["Privacy & Security", "Confidentialité et sécurité",
        "Datenschutz & Sicherheit", "Privacidad y seguridad", "Privacy e sicurezza"]
    public static let systemSettings: Set<String> = ["com.apple.systempreferences", "com.apple.Settings"]

    /// nil = allowed; else the `refused` error.
    public static func check(bundle: String, windowTitle: String? = nil) -> CUError? {
        if bundle.hasPrefix("dev.bise.") || bundle == "dev.bise" {
            return CUError("refused", "bise does not drive itself; use sb or the tools you have")
        }
        if let name = bundles[bundle] {
            return CUError("refused", "\(name) is off limits to computer use; ask the user to do this step")
        }
        if systemSettings.contains(bundle), let t = windowTitle, privacyTitles.contains(where: { t.contains($0) }) {
            return CUError("refused", "Privacy & Security settings are off limits; ask the user to change them")
        }
        return nil
    }
}

// MARK: keys (Playwright names → macOS virtual key codes)

public struct KeyChord: Equatable {
    public var code: UInt16
    public var shift = false, control = false, option = false, command = false
    public init(code: UInt16, shift: Bool = false, control: Bool = false, option: Bool = false, command: Bool = false) {
        self.code = code; self.shift = shift; self.control = control; self.option = option; self.command = command
    }
}

public enum Keys {
    static let named: [String: UInt16] = [
        "enter": 0x24, "return": 0x24, "tab": 0x30, "space": 0x31, " ": 0x31, "backspace": 0x33,
        "escape": 0x35, "esc": 0x35, "delete": 0x75, "home": 0x73, "end": 0x77, "pageup": 0x74,
        "pagedown": 0x79, "arrowleft": 0x7B, "arrowright": 0x7C, "arrowdown": 0x7D, "arrowup": 0x7E,
        "f1": 0x7A, "f2": 0x78, "f3": 0x63, "f4": 0x76, "f5": 0x60, "f6": 0x61, "f7": 0x62,
        "f8": 0x64, "f9": 0x65, "f10": 0x6D, "f11": 0x67, "f12": 0x6F,
    ]
    static let chars: [Character: UInt16] = [
        "a": 0x00, "s": 0x01, "d": 0x02, "f": 0x03, "h": 0x04, "g": 0x05, "z": 0x06, "x": 0x07,
        "c": 0x08, "v": 0x09, "b": 0x0B, "q": 0x0C, "w": 0x0D, "e": 0x0E, "r": 0x0F, "y": 0x10,
        "t": 0x11, "1": 0x12, "2": 0x13, "3": 0x14, "4": 0x15, "6": 0x16, "5": 0x17, "=": 0x18,
        "9": 0x19, "7": 0x1A, "-": 0x1B, "8": 0x1C, "0": 0x1D, "]": 0x1E, "o": 0x1F, "u": 0x20,
        "[": 0x21, "i": 0x22, "p": 0x23, "l": 0x25, "j": 0x26, "'": 0x27, "k": 0x28, ";": 0x29,
        "\\": 0x2A, ",": 0x2B, "/": 0x2C, "n": 0x2D, "m": 0x2E, ".": 0x2F, "`": 0x32,
    ]
    /// "Control+A Tab Enter" → chords; "+" alone is a key ("Shift++").
    public static func parse(_ s: String) throws -> [KeyChord] {
        let chords = s.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
        if chords.isEmpty { throw CUError("bad_args", "press needs keys, e.g. \"Enter\" or \"Meta+A\"") }
        return try chords.map { chord in
            var parts = chord.components(separatedBy: "+")
            // "Shift++" → ["Shift", "", ""]: the key is "+"
            if chord.hasSuffix("++") { parts = Array(parts.dropLast(2)) + ["+"] }
            guard let key = parts.last, !key.isEmpty else { throw CUError("bad_args", "the key in \"\(chord)\" is missing") }
            var k = KeyChord(code: 0)
            for m in parts.dropLast() {
                switch m.lowercased() {
                case "shift": k.shift = true
                case "control", "ctrl": k.control = true
                case "alt", "option": k.option = true
                case "meta", "command", "cmd", "controlormeta": k.command = true
                default: throw CUError("bad_args", "unknown modifier \"\(m)\" in \"\(chord)\"; use Shift, Control, Alt or Meta")
                }
            }
            if let c = named[key.lowercased()] { k.code = c; return k }
            if key.count == 1, let ch = key.first {
                let lower = Character(ch.lowercased())
                // "A" alone types a capital; "Meta+A" is Cmd+A (Playwright)
                if let c = chars[lower] { k.code = c; if ch.isUppercase && parts.count == 1 { k.shift = true }; return k }
                let shifted: [Character: Character] = ["+": "=", "_": "-", "!": "1", "@": "2", "#": "3", "$": "4",
                    "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0", "?": "/", ":": ";", "\"": "'",
                    "<": ",", ">": ".", "{": "[", "}": "]", "|": "\\", "~": "`"]
                if let base = shifted[ch], let c = chars[base] { k.code = c; k.shift = true; return k }
            }
            throw CUError("bad_args", "unknown key \"\(key)\"; use Playwright names like Enter, Tab, ArrowDown, Meta+A")
        }
    }
}

// MARK: summaries (C1: the line the TUI shows after ↖)

public enum Summary {
    /// Keys as the user reads them (designer m_3774): "Meta+A Enter" →
    /// "cmd+a enter"; mac names cmd, opt, ctrl, shift; never Meta.
    public static func keys(_ s: String) -> String {
        s.split(separator: " ").map { chord in
            chord.split(separator: "+", omittingEmptySubsequences: false).map { part -> String in
                switch part.lowercased() {
                case "meta", "command", "cmd", "controlormeta": return "cmd"
                case "alt", "option": return "opt"
                case "control", "ctrl": return "ctrl"
                case "": return "+"
                default: return part.lowercased()
                }
            }.joined(separator: "+").replacingOccurrences(of: "++", with: "+")
        }.joined(separator: " ")
    }

    /// `"Save"`, else `the text area` style from the role.
    public static func what(name: String?, role: String?) -> String {
        if let n = name?.trimmingCharacters(in: .whitespacesAndNewlines), !n.isEmpty {
            return "\"" + Snapshot.cut(n, 32) + "\""
        }
        switch role ?? "" {
        case "textbox": return "the text field"
        case "searchbox": return "the search box"
        case "": return "the window"
        default: return "the " + (role ?? "element")
        }
    }
}
