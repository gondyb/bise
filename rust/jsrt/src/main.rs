// jsrt.rs — bend-jsrt: run_program in a fresh V8 isolate (the Vibe SDK
// core pattern).
//
// `bend-jsrt <program-file> <results-file>`:
//   - the program is TypeScript/JavaScript; deno_ast transpiles it (types
//     stripped in-process — no tsgo, no external stripper)
//   - a fresh isolate runs it with heap limits and a watchdog
//   - free identifiers become tool calls through a `with`-Proxy; dotted
//     names compose (github_app.get_me)
//   - the FIRST missing tool result makes the runner print the request
//     to stdout and exit 42; the host (the Bend runtime) executes the
//     tool and re-runs with the extended results — replay by
//     re-execution, the same durable-boundary semantics as the in-house
//     interpreter
//   - the final value prints to stdout and exits 0; errors go to stderr
//     and exit 1 so the model can read them and retry

use std::sync::mpsc;
use std::sync::Once;
use std::thread;
use std::time::Duration;

use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseParams;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileOptions;
use deno_ast::parse_program;
use deno_core::v8;

static INITIALIZE_V8: Once = Once::new();
const INITIAL_HEAP_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEAP_BYTES: usize = 64 * 1024 * 1024;
const ROUND_TIMEOUT: Duration = Duration::from_secs(10);

// TS -> JS in-process (types, interfaces, enums stripped)
fn transpile(code: &str) -> Result<String, String> {
    let parsed = parse_program(ParseParams {
        specifier: ModuleSpecifier::parse("file:///run_program.ts")
            .map_err(|error| error.to_string())?,
        text: code.to_string().into(),
        media_type: MediaType::TypeScript,
        capture_tokens: false,
        maybe_syntax: None,
        scope_analysis: false,
    })
    .map_err(|error| error.to_string())?;
    parsed
        .transpile(
            &TranspileOptions::default(),
            &TranspileModuleOptions::default(),
            &EmitOptions::default(),
        )
        .map(|result| result.into_source().text)
        .map_err(|error| error.to_string())
}

const PRELUDE: &str = r#"
const __RESULTS = __RESULTS_PLACEHOLDER__;
let __i = 0;
function __tool(name, args) {
  if (name.startsWith('tools.')) name = name.slice(6);
  if (__i < __RESULTS.length) {
    const r = __RESULTS[__i]; __i++;
    try { return JSON.parse(r); } catch (e) { return r; }
  }
  const payload = args === undefined || args === null
    ? '{}'
    : JSON.stringify(args);
  __request_tool(name, payload);
}
// self.readImages(paths) (alias self.readImage): the image blocks of local
// files, to put in the program's returned array. Each file is checked here
// (it exists, it is a PNG, JPEG, GIF or WebP); a relative path is the
// agent's working dir's (BEND_WORKDIR), ~/ is HOME. Local: no tool call,
// no replay position; a bad path throws (the program fails with the reason).
const __readImages = (paths) => {
  const list = Array.isArray(paths) ? paths : [paths];
  if (list.length === 0 || !list.every((p) => typeof p === 'string' && p.trim() !== '')) {
    throw new Error('self.readImages: give a file path or an array of file paths (strings)');
  }
  return list.map((p) => {
    const r = __image_check(p);
    if (r.startsWith('error:')) throw new Error('self.readImages: ' + r.slice(6));
    return { type: 'image', path: r.slice(3) };
  });
};
const __selfLocal = { readImages: __readImages, readImage: __readImages };
const __skip = ['then', 'toString', 'valueOf', 'inspect', 'constructor', 'prototype'];
const __gh = {
  has: () => true,
  get: (t, name) => {
    if (typeof name !== 'string') return undefined;
    if (name in globalThis) return globalThis[name];
    return new Proxy(function () {}, {
      get: (t2, k) => {
        if (typeof k !== 'string' || __skip.includes(k)) return undefined;
        if (name === 'self' && Object.hasOwn(__selfLocal, k)) return __selfLocal[k];
        const sub = name + '.' + k;
        return new Proxy(function (...a) { return __tool(sub, a[0]); }, {
          get: (t3, k2) => {
            if (typeof k2 !== 'string' || __skip.includes(k2)) return undefined;
            return new Proxy(function (...a2) { return __tool(sub + '.' + k2, a2[0]); }, {});
          }
        });
      },
      apply: (t2, thisArg, a) => __tool(name, a[0]),
    });
  },
};
const console = {
  log: (...a) => __log(a.map(String).join(' ')),
  error: (...a) => __log(a.map(String).join(' ')),
  warn: (...a) => __log(a.map(String).join(' ')),
};
// the Vibe rule: a non-empty array of content blocks REPLACES the result;
// text blocks stay text, image blocks ({type:'image', data, mimeType} or
// our {type:'image', path}) become image markers (docs/images.md)
const __isBlock = (b) => b !== null && typeof b === 'object' && (
  (b.type === 'text' && typeof b.text === 'string') ||
  (b.type === 'image' && (typeof b.data === 'string' || typeof b.path === 'string')));
const __blocks = (v) => {
  let n = 0;
  return v.map((b) => b.type === 'text' ? b.text : __image(
    typeof b.data === 'string' ? b.data : '',
    String(b.mimeType || b.mime_type || ''),
    typeof b.path === 'string' ? b.path : '',
    '[Image #' + (++n) + ']')).join('\n');
};
const __finish = (p) => Promise.resolve(p).then(
  (v) => __done(typeof v === 'string' ? v
    : (Array.isArray(v) && v.length > 0 && v.every(__isBlock)) ? __blocks(v)
    : JSON.stringify(v === undefined ? '' : v)),
  (e) => __fail(JSON.stringify(String((e && e.message) || e)))
);
__COMPUTER_SDK__
const __out = (async function () {
  with (new Proxy({}, __gh)) {
"#;

// the fallback closure: assigned on globalThis (an identifier assignment
// inside the `with` scope would land on the proxy target, and `main` is
// only resolvable from inside that scope - the closure captures it).
// Injected only when the program declares `function main`.
const PRELUDE_MAIN_FB: &str = r#"
globalThis.__fb = () => Promise.resolve(main());
"#;

// one epilogue for every program: the body is an async function body, so
// top-level await always works and a top-level `return` gives the result;
// when the body produced no value and `function main` was declared, the
// resolved value of main() is the result (the promise chain drains through
// the microtask checkpoint; __done/__fail exit from inside)
const EPILOGUE: &str = r#"
  }
})();
__finish(__out.then((v) => {
  if (v === undefined && typeof globalThis.__fb === 'function') {
    return globalThis.__fb();
  }
  return v;
}));
"#;

// the `computer` object of code mode (docs/computer-use-design.md §6),
// in the prelude when the session loaded the built-in `computer` plugin:
// its tools are in this session's plugin index (rust/plugins bridge,
// bend/runtime/plugins.bend: $BEND_RUN_DIR/<repl port>/plugins/), lines
// `<cid> computer <tool> : …`
const COMPUTER_TS: &str = include_str!("computer.ts");

fn computer_loaded() -> bool {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let run = var("BEND_RUN_DIR")
        .or_else(|| var("HOME").map(|h| format!("{h}/.bend-harness/run")))
        .unwrap_or_default();
    let port = var("BEND_REPL_PORT").unwrap_or_else(|| "0".to_string());
    let index = std::fs::read_to_string(format!("{run}/{port}/plugins/mcp-index.txt")).unwrap_or_default();
    index.lines().any(|l| {
        let mut f = l.split_whitespace().skip(1);
        f.next() == Some("computer") && f.next() == Some("act")
    })
}

// does the (transpiled) program declare a `main` function?
// `function main` followed (after spaces) by `(` or `<` — covers
// `function main`, `async function main`, generic forms included
fn declares_main(js: &str) -> bool {
    let bytes = js.as_bytes();
    let mut i = 0;
    while let Some(at) = js[i..].find("function main") {
        let start = i + at + "function main".len();
        let mut j = start;
        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
            j += 1;
        }
        if j < bytes.len() && (bytes[j] == b'(' || bytes[j] == b'<') {
            return true;
        }
        i = start;
    }
    false
}

fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// the program failed: print a machine-readable error (exit 43) so the
// host can hand the message back to the model, which can read and retry
fn fail(message: &str) -> ! {
    println!("{{\"error\": \"{}\"}}", json_escape(message));
    std::process::exit(43);
}

fn to_rust_string(scope: &mut v8::PinScope, value: v8::Local<v8::Value>) -> String {
    value
        .to_string(scope)
        .map(|s| s.to_rust_string_lossy(&**scope))
        .unwrap_or_default()
}

unsafe extern "C" fn native_call(
    info: *const v8::FunctionCallbackInfo,
) {
    let info = unsafe { &*info };
    let scope = std::pin::pin!(unsafe { v8::CallbackScope::new(info) });
    let mut scope = scope.init();
    let args =
        v8::FunctionCallbackArguments::from_function_callback_info(info);
    let data = args.data();
    let name = data
        .to_string(&mut scope)
        .map(|s| s.to_rust_string_lossy(&scope))
        .unwrap_or_default();
    match name.as_str() {
        "__request_tool" => {
            let tool = to_rust_string(&mut scope, args.get(0));
            let payload = to_rust_string(&mut scope, args.get(1));
            println!("{{\"tool\": \"{tool}\", \"args\": {payload}}}");
            std::process::exit(42);
        }
        "__done" => {
            let result = to_rust_string(&mut scope, args.get(0));
            println!("{result}");
            std::process::exit(0);
        }
        "__fail" => {
            let message = to_rust_string(&mut scope, args.get(0));
            // the payload is already JSON-encoded by the epilogue
            println!("{{\"error\": {message}}}");
            std::process::exit(43);
        }
        "__image" => {
            let data = to_rust_string(&mut scope, args.get(0));
            let mime = to_rust_string(&mut scope, args.get(1));
            let path = to_rust_string(&mut scope, args.get(2));
            let name = to_rust_string(&mut scope, args.get(3));
            let text = image_marker(&data, &mime, &path, &name);
            if let Some(v) = v8::String::new(&scope, &text) {
                let mut rv = v8::ReturnValue::from_function_callback_info(info);
                rv.set(v.into());
            }
        }
        "__image_check" => {
            let path = to_rust_string(&mut scope, args.get(0));
            let text = match image_check(&path) {
                Ok(abs) => format!("ok:{}", abs.display()),
                Err(e) => format!("error:{e}"),
            };
            if let Some(v) = v8::String::new(&scope, &text) {
                let mut rv = v8::ReturnValue::from_function_callback_info(info);
                rv.set(v.into());
            }
        }
        _ => {
            let line = to_rust_string(&mut scope, args.get(0));
            eprintln!("[program] {line}");
        }
    }
}

// one image block of a returned content array -> its marker (the image
// goes to the store), or a text note the model can read
fn image_marker(data: &str, mime: &str, path: &str, name: &str) -> String {
    let stored = if !path.is_empty() {
        bend_images::store_file(std::path::Path::new(path))
    } else {
        match bend_images::base64_decode(data) {
            Some(bytes) if bend_images::sniff(&bytes).is_some() => bend_images::store_bytes(bytes),
            Some(_) => Err(format!("the data is not a PNG, JPEG, GIF or WebP image (mimeType {mime:?})")),
            None => Err("the data is not base64".to_string()),
        }
    };
    match stored {
        Ok(s) => {
            let source = if path.is_empty() { s.file.to_string_lossy().to_string() } else { path.to_string() };
            bend_images::marker(name, &source, &s)
        }
        Err(e) => format!("[image not attached: {e}]"),
    }
}

// a path of self.readImages -> absolute: ~/ is HOME, a relative path is
// the agent's working dir's (BEND_WORKDIR, where its bash runs), else
// this process's cwd
fn resolve_path(path: &str) -> std::path::PathBuf {
    let env_dir = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = env_dir("HOME") {
            return home.join(rest);
        }
    }
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let base = env_dir("BEND_WORKDIR")
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    base.join(p)
}

// self.readImages's check of one file: it exists, it is a file, not too
// large, and its first bytes say PNG, JPEG, GIF or WebP (the content, not
// the extension). Ok: the absolute path.
fn image_check(path: &str) -> Result<std::path::PathBuf, String> {
    use std::io::Read;
    let abs = resolve_path(path.trim());
    let meta = match std::fs::metadata(&abs) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("no file at {}", abs.display()));
        }
        Err(e) => return Err(format!("{}: {e}", abs.display())),
    };
    if !meta.is_file() {
        return Err(format!("{} is not a file", abs.display()));
    }
    if meta.len() > bend_images::MAX_INPUT_BYTES as u64 {
        return Err(format!("{} is too large ({} bytes)", abs.display(), meta.len()));
    }
    let mut head = Vec::with_capacity(16);
    std::fs::File::open(&abs)
        .and_then(|f| f.take(16).read_to_end(&mut head))
        .map_err(|e| format!("{}: {e}", abs.display()))?;
    if bend_images::sniff(&head).is_none() {
        return Err(format!("{} is not a PNG, JPEG, GIF or WebP image", abs.display()));
    }
    Ok(abs)
}

fn set_global(
    scope: &mut v8::PinScope,
    context: &v8::Local<v8::Context>,
    name: &str,
) {
    let key = v8::String::new(scope, name).unwrap();
    let tag = v8::String::new(scope, name).unwrap();
    let builder: v8::FunctionBuilder<v8::Function> =
        v8::FunctionBuilder::new_raw(native_call).data(tag.into());
    let val = builder.build(scope).unwrap();
    let global = context.global(scope);
    let _ = global.set(scope, key.into(), val.into());
}

fn run(program: &str, results: &str) -> ! {
    let js = match transpile(program) {
        Ok(js) => js,
        Err(error) => {
            fail(&format!("SyntaxError: {error}"));
        }
    };
    let results_lit = if results.trim().is_empty() {
        "[]".to_string()
    } else {
        results.trim().to_string()
    };
    let fb = if declares_main(&js) {
        PRELUDE_MAIN_FB
    } else {
        ""
    };
    let sdk = if computer_loaded() {
        transpile(COMPUTER_TS).unwrap_or_else(|e| fail(&format!("computer.ts: {e}")))
    } else {
        String::new()
    };
    let source = format!(
        "{}{}{}{}",
        PRELUDE
            .replacen("__COMPUTER_SDK__", &sdk, 1)
            .replace("__RESULTS_PLACEHOLDER__", &results_lit),
        fb,
        js,
        EPILOGUE
    );

    INITIALIZE_V8.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });

    let params = v8::CreateParams::default().heap_limits(INITIAL_HEAP_BYTES, MAX_HEAP_BYTES);
    let mut isolate = v8::Isolate::new(params);
    let isolate_handle = isolate.thread_safe_handle();

    // watchdog: terminate the round if it exceeds the deadline
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        if stop_rx.recv_timeout(ROUND_TIMEOUT).is_err() {
            isolate_handle.terminate_execution();
        }
    });

    v8::scope!(let handle_scope, &mut isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    v8::tc_scope!(let tc, scope);

    set_global(tc, &context, "__request_tool");
    set_global(tc, &context, "__done");
    set_global(tc, &context, "__fail");
    set_global(tc, &context, "__log");
    set_global(tc, &context, "__image");
    set_global(tc, &context, "__image_check");

    let code = match v8::String::new(tc, &source) {
        Some(code) => code,
        None => {
            fail("could not allocate the program source");
        }
    };
    let script = match v8::Script::compile(tc, code, None) {
        Some(script) => script,
        None => {
            let message = tc
                .exception()
                .and_then(|e| e.to_string(tc))
                .map(|s| s.to_rust_string_lossy(tc))
                .unwrap_or_else(|| "could not compile the program".to_string());
            tc.reset();
            fail(&message);
        }
    };
    if script.run(tc).is_none() {
        let message = tc
            .exception()
            .and_then(|e| e.to_string(tc))
            .map(|s| s.to_rust_string_lossy(tc))
            .unwrap_or_else(|| "unknown error".to_string());
        let deadline = tc.is_execution_terminating();
        let _ = stop_tx.send(());
        if deadline {
            fail(&format!("program exceeded the {ROUND_TIMEOUT:?} deadline"));
        } else {
            fail(&format!("ExecutionError: {message}"));
        }
    }
    // the vibe format resolves `async function main()` through the
    // microtask queue — drain it (__done/__fail exit from inside)
    tc.perform_microtask_checkpoint();
    let _ = stop_tx.send(());
    // __done exits before this point on success
    fail("the program ended without returning");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: bend-jsrt <program-file> <results-file>");
        std::process::exit(1);
    }
    let program = std::fs::read_to_string(&args[1]).unwrap_or_default();
    let results = std::fs::read_to_string(&args[2]).unwrap_or_default();
    run(&program, &results);
}
