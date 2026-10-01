// CUCore's unit checks (no XCTest with the Command Line Tools): roles,
// C2 lines, locators on a fake tree, diff, refusals, key names.
import CUCore
import Foundation

var failures = 0
var count = 0
func expect(_ ok: Bool, _ what: String, file: String = #file, line: Int = #line) {
    count += 1
    if !ok { failures += 1; print("FAIL \(what) (line \(line))") }
}
func eq<T: Equatable>(_ a: T, _ b: T, _ what: String, line: Int = #line) {
    count += 1
    if a != b { failures += 1; print("FAIL \(what) (line \(line)): got \(a), want \(b)") }
}

// roles (C2)
eq(Roles.aria(role: "AXButton"), "button", "AXButton")
eq(Roles.aria(role: "AXTextArea"), "textbox", "AXTextArea")
eq(Roles.aria(role: "AXTextField", subrole: "AXSearchField"), "searchbox", "search subrole")
eq(Roles.aria(role: "AXPopUpButton"), "combobox", "popup")
eq(Roles.aria(role: "AXStaticText"), "text", "static text")
eq(Roles.aria(role: "AXScrollArea"), "scrollarea", "other roles lowercased")
eq(Roles.aria(role: "AXTabGroup"), "tablist", "tab group")

// C2 lines
var b = Node(role: "button", name: "Save")
b.ref = 3
eq(Snapshot.line(b), "- button \"Save\" [e3]", "button line")
var tf = Node(role: "textbox", name: "Email", value: "a@b.c")
tf.ref = 4; tf.focused = true
eq(Snapshot.line(tf), "- textbox \"Email\" [e4] value=\"a@b.c\" (focused)", "textbox line")
var pw = Node(role: "textbox", name: "Password", value: "hunter2")
pw.ref = 5; pw.secure = true
eq(Snapshot.line(pw), "- textbox \"Password\" [e5] value=\"•••\"", "password never shown")
expect(!Snapshot.line(pw).contains("hunter2"), "no password in the line")
var cb = Node(role: "checkbox", name: "Bold"); cb.ref = 6; cb.checked = true; cb.disabled = true
eq(Snapshot.line(cb), "- checkbox \"Bold\" [e6] (disabled) (checked)", "flags order")
let long = Node(role: "textbox", name: "x", value: String(repeating: "a", count: 200))
expect(Snapshot.line(long).contains(String(repeating: "a", count: 79) + "…\""), "value cut at 80")
eq(Snapshot.line(Node(role: "text", name: "say \"hi\"\nnow")), "- text \"say \\\"hi\\\" now\"", "quotes and newlines")
var w = Node(role: "window", name: "Doc"); w.ref = 1
var t1 = Node(role: "text", name: "hello", depth: 1); t1.ref = 2
eq(Snapshot.text(header: "Doc · TextEdit", nodes: [w, t1]), "# Doc · TextEdit\n- window \"Doc\" [e1]\n  - text \"hello\" [e2]", "indent 2 per depth")

// locators
func node(_ role: String, _ name: String, _ ref: Int, value: String? = nil, label: String = "") -> Node {
    var n = Node(role: role, name: name, value: value); n.ref = ref; n.label = label; return n
}
let tree = [node("window", "Shop", 1), node("button", "Add to cart", 2), node("button", "Add to list", 3),
            node("textbox", "Search", 4, value: "cable"), node("text", "Anker USB-C 2 m", 5),
            node("textbox", "", 6, label: "Email"), node("button", "Buy now", 7)]
func loc(_ j: [String: Any]) -> Locator { try! Locator(json: j) }
if case .success(let i) = loc(["role": "button", "name": "add to cart"]).resolve(tree) { eq(i, 1, "name substring, case") } else { expect(false, "add to cart") }
if case .failure(let e) = loc(["role": "button", "name": "Add"]).resolve(tree) {
    eq(e.code, "ambiguous", "two matches")
    eq(e.candidates?.count ?? 0, 2, "candidates listed")
    eq(e.candidates?.first ?? "", "- button \"Add to cart\" [e2]", "candidates are snapshot lines")
} else { expect(false, "ambiguous") }
if case .success(let i) = loc(["role": "button", "name": "Add", "nth": 1]).resolve(tree) { eq(i, 2, "nth") } else { expect(false, "nth") }
if case .success(let i) = loc(["role": "button", "name": "Add", "nth": -1]).resolve(tree) { eq(i, 2, "nth -1") } else { expect(false, "nth -1") }
if case .failure(let e) = loc(["role": "button", "name": "Add to", "exact": true]).resolve(tree) { eq(e.code, "not_found", "exact") } else { expect(false, "exact") }
if case .success(let i) = loc(["text_re": "Anker.*2 m/i"]).resolve(tree) { eq(i, 4, "text_re with flags") } else { expect(false, "text_re") }
if case .success(let i) = loc(["name_re": "^buy"]).resolve(tree) { expect(false, "regex is case-sensitive without /i: \(i)") }
if case .success(let i) = loc(["name_re": "^buy/i"]).resolve(tree) { eq(i, 6, "name_re /i") } else { expect(false, "name_re /i") }
if case .success(let i) = loc(["label": "email"]).resolve(tree) { eq(i, 5, "label") } else { expect(false, "label") }
if case .success(let i) = loc(["text": "cable"]).resolve(tree) { expect(false, "text sees the name of a textbox, not its value: \(i)") }
if case .failure(let e) = loc(["role": "slider"]).resolve(tree) { eq(e.code, "not_found", "not found") } else { expect(false, "slider") }
do { _ = try Locator(json: [:]); expect(false, "empty locator") } catch let e as CUError { eq(e.code, "bad_args", "empty locator") } catch {}
do { _ = try Locator(json: ["name_re": "(/i"]); expect(false, "bad regex") } catch let e as CUError { eq(e.code, "bad_args", "bad regex") } catch {}
eq(try! Locator.regex("a/b").pattern, "a/b", "a slash that is not flags")

// diff
let before = "# T · A\n- button \"A\" [e1]\n- text \"0\" [e2]"
let after = "# T · A\n- button \"A\" [e1]\n- text \"1\" [e2]"
eq(Snapshot.diff(before: before, after: after), "- - text \"0\" [e2]\n+ - text \"1\" [e2]", "diff")
eq(Snapshot.diff(before: before, after: before), "", "no change")
let many = "# x\n" + (0..<40).map { "- text \"\($0)\" [e\($0)]" }.joined(separator: "\n")
let d = Snapshot.diff(before: "# x", after: many)
eq(d.split(separator: "\n").count, 20, "≤ 20 lines")
expect(d.hasSuffix("… 21 more lines"), "says how many more")

// read
eq(Snapshot.readText([node("text", "a", 1), node("textbox", "q", 2, value: "v"), pw, node("text", "a", 3)]), "a\nv", "read: texts and values, no password, no dup")

// refusals
eq(Refusals.check(bundle: "com.mitchellh.ghostty")?.code ?? "", "refused", "Ghostty")
eq(Refusals.check(bundle: "com.apple.Terminal")?.code ?? "", "refused", "Terminal")
eq(Refusals.check(bundle: "dev.bise.computer-use")?.code ?? "", "refused", "bise itself")
eq(Refusals.check(bundle: "com.1password.1password")?.code ?? "", "refused", "1Password")
eq(Refusals.check(bundle: "com.apple.loginwindow")?.code ?? "", "refused", "loginwindow")
expect(Refusals.check(bundle: "com.apple.systempreferences", windowTitle: "Wi-Fi") == nil, "System Settings, other pane")
eq(Refusals.check(bundle: "com.apple.systempreferences", windowTitle: "Privacy & Security")?.code ?? "", "refused", "Privacy pane")
eq(Refusals.check(bundle: "com.apple.systempreferences", windowTitle: "Confidentialité et sécurité")?.code ?? "", "refused", "Privacy pane (fr)")
expect(Refusals.check(bundle: "com.apple.TextEdit") == nil, "TextEdit allowed")

// keys
eq(try! Keys.parse("Enter"), [KeyChord(code: 0x24)], "Enter")
eq(try! Keys.parse("Meta+A"), [KeyChord(code: 0x00, command: true)], "Meta+A")
eq(try! Keys.parse("Control+Shift+ArrowDown"), [KeyChord(code: 0x7D, shift: true, control: true)], "chord")
eq(try! Keys.parse("Tab Tab Enter").count, 3, "space-separated chords")
eq(try! Keys.parse("A"), [KeyChord(code: 0x00, shift: true)], "uppercase = shift")
eq(try! Keys.parse("Shift++"), [KeyChord(code: 0x18, shift: true)], "the + key")
do { _ = try Keys.parse("Hyper+A"); expect(false, "bad modifier") } catch let e as CUError { eq(e.code, "bad_args", "bad modifier") } catch {}
do { _ = try Keys.parse("NoSuchKey"); expect(false, "bad key") } catch let e as CUError { eq(e.code, "bad_args", "bad key") } catch {}

// summaries
eq(Summary.what(name: "Save", role: "button"), "\"Save\"", "named")
eq(Summary.what(name: "", role: "searchbox"), "the search box", "unnamed searchbox")
eq(Summary.what(name: String(repeating: "x", count: 40), role: "button"), "\"" + String(repeating: "x", count: 31) + "…\"", "labels cut at 32")
eq(Summary.keys("Meta+A"), "cmd+a", "keys: cmd, lowercase")
eq(Summary.keys("Control+Alt+Shift+ArrowDown Enter"), "ctrl+opt+shift+arrowdown enter", "keys: mac names")
eq(Summary.keys("Shift++"), "shift++", "keys: the + key")

print(failures == 0 ? "cu-check: \(count) checks ok" : "cu-check: \(failures) of \(count) failed")
exit(failures == 0 ? 0 : 1)
