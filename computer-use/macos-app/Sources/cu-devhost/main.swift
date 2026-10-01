// Dev only: "bise Computer Use dev.app" (dev.bise.computer-use.dev). TCC
// keys an ad-hoc signed app on its code hash, so every rebuild of the real
// helper loses its Accessibility and Screen Recording grants. This host
// never changes: it loads the helper's code from libCUHelperDylib.dylib
// (next to the bundle; `bundle.sh --devhost` copies it there) and runs it,
// so the grants given once to the dev host hold across rebuilds. Never shipped.
import Foundation

let path = (Bundle.main.bundlePath as NSString).deletingLastPathComponent + "/libCUHelperDylib.dylib"
guard let h = dlopen(path, RTLD_NOW) else {
    FileHandle.standardError.write("cu-devhost: dlopen \(path): \(String(cString: dlerror()))\n".data(using: .utf8)!)
    exit(1)
}
guard let sym = dlsym(h, "cu_helper_main") else {
    FileHandle.standardError.write("cu-devhost: no cu_helper_main in \(path)\n".data(using: .utf8)!)
    exit(1)
}
typealias Main = @convention(c) () -> Void
unsafeBitCast(sym, to: Main.self)()
