// The tests' own app: one window "cu test" with a button, a text field,
// a secure field, a checkbox, a pop-up and a long list. Every change is
// written to the label "status: …" so a test can read it back through AX.
// Started in the background; it never activates itself.
import AppKit

final class Delegate: NSObject, NSApplicationDelegate, NSTableViewDataSource {
    var window: NSWindow!
    let status = NSTextField(labelWithString: "status: ready")
    var clicks = 0

    func applicationDidFinishLaunching(_ n: Notification) {
        window = NSWindow(contentRect: NSRect(x: 120, y: 160, width: 420, height: 380),
                          styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.title = "cu test"
        window.isReleasedWhenClosed = false
        let v = NSView(frame: window.contentView!.bounds)
        window.contentView = v

        status.frame = NSRect(x: 20, y: 340, width: 380, height: 20)
        status.setAccessibilityIdentifier("status")
        v.addSubview(status)

        let button = NSButton(title: "Add one", target: self, action: #selector(add))
        button.frame = NSRect(x: 20, y: 300, width: 120, height: 30)
        v.addSubview(button)

        let field = NSTextField(frame: NSRect(x: 20, y: 262, width: 250, height: 24))
        field.placeholderString = "Your name"
        field.target = self; field.action = #selector(entered(_:))
        v.addSubview(field)

        let secret = NSSecureTextField(frame: NSRect(x: 20, y: 228, width: 250, height: 24))
        secret.placeholderString = "Password"
        secret.stringValue = "hunter2"
        v.addSubview(secret)

        let check = NSButton(checkboxWithTitle: "Remember me", target: self, action: #selector(toggled(_:)))
        check.frame = NSRect(x: 20, y: 196, width: 200, height: 24)
        v.addSubview(check)

        let pop = NSPopUpButton(frame: NSRect(x: 20, y: 160, width: 160, height: 26), pullsDown: false)
        pop.addItems(withTitles: ["Small", "Medium", "Large"])
        pop.target = self; pop.action = #selector(picked(_:))
        v.addSubview(pop)

        let scroll = NSScrollView(frame: NSRect(x: 20, y: 10, width: 380, height: 140))
        let table = NSTableView()
        let col = NSTableColumn(identifier: .init("row")); col.title = "Rows"; col.width = 340
        table.addTableColumn(col)
        table.dataSource = self
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        v.addSubview(scroll)

        // shown behind the user's windows, never made key or active (the
        // tests also check a covered window can be captured)
        window.orderBack(nil)
    }

    func numberOfRows(in t: NSTableView) -> Int { 200 }
    func tableView(_ t: NSTableView, objectValueFor c: NSTableColumn?, row: Int) -> Any? { "row \(row)" }

    func say(_ s: String) { status.stringValue = "status: " + s }
    @objc func add() { clicks += 1; say("clicked \(clicks)") }
    @objc func entered(_ f: NSTextField) { say("name \(f.stringValue)") }
    @objc func toggled(_ b: NSButton) { say(b.state == .on ? "remember on" : "remember off") }
    @objc func picked(_ p: NSPopUpButton) { say("size \(p.titleOfSelectedItem ?? "")") }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let d = Delegate()
app.delegate = d
app.run()
