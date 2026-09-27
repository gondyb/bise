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
const __skip = ['then', 'toString', 'valueOf', 'inspect', 'constructor', 'prototype'];
const __gh = {
  has: () => true,
  get: (t, name) => {
    if (typeof name !== 'string') return undefined;
    if (name in globalThis) return globalThis[name];
    return new Proxy(function () {}, {
      get: (t2, k) => {
        if (typeof k !== 'string' || __skip.includes(k)) return undefined;
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
const __finish = (p) => Promise.resolve(p).then(
  (v) => __done(typeof v === 'string' ? v : JSON.stringify(v === undefined ? '' : v)),
  (e) => __fail(JSON.stringify(String((e && e.message) || e)))
);
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
        _ => {
            let line = to_rust_string(&mut scope, args.get(0));
            eprintln!("[program] {line}");
        }
    }
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
    let source = format!(
        "{}{}{}{}",
        PRELUDE.replace("__RESULTS_PLACEHOLDER__", &results_lit),
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
