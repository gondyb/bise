// swift-tools-version:5.9
// bise Computer Use.app: the macOS helper of computer use (design
// docs/computer-use-design.md §5.2, contracts C1/C2/C5 in
// docs/computer-use-briefs.md). `scripts/bundle.sh` makes the .app,
// `scripts/test.sh` runs the checks.
import PackageDescription

let package = Package(
    name: "BiseComputerUse",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "BiseComputerUse", targets: ["BiseComputerUse"]),
        // dev only: the helper's code for cu-devhost (see its main.swift)
        .library(name: "CUHelperDylib", type: .dynamic, targets: ["CUHelper"]),
        .executable(name: "cu-devhost", targets: ["cu-devhost"]),
        .executable(name: "cu-client", targets: ["cu-client"]),
        .executable(name: "cu-testapp", targets: ["cu-testapp"]),
        .executable(name: "cu-check", targets: ["cu-check"]),
    ],
    targets: [
        // pure logic, no macOS frameworks: roles, snapshot text, locators,
        // diff, refusals, key names (checked by cu-check)
        .target(name: "CUCore"),
        // the helper: C5 server, AX, ScreenCaptureKit, overlay, takeover
        .target(name: "CUHelper", dependencies: ["CUCore"]),
        // the app's executable (calls CUHelper's main)
        .executableTarget(name: "BiseComputerUse", dependencies: ["CUHelper"]),
        // dev only: a stable host that loads CUHelper from the dylib
        .executableTarget(name: "cu-devhost"),
        // a C5 client for tests and by hand
        .executableTarget(name: "cu-client"),
        // a tiny AppKit app the tests drive
        .executableTarget(name: "cu-testapp"),
        // CUCore's unit checks (no XCTest with the Command Line Tools)
        .executableTarget(name: "cu-check", dependencies: ["CUCore"]),
    ]
)
