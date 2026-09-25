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
const __out = (function () {
  with (new Proxy({}, __gh)) {
"#;

const EPILOGUE: &str = r#"
  }
})();
__done(typeof __out === 'string' ? __out : JSON.stringify(__out === undefined ? '' : __out));
"#;

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
            eprintln!("SyntaxError: {error}");
            std::process::exit(1);
        }
    };
    let results_lit = if results.trim().is_empty() {
        "[]".to_string()
    } else {
        results.trim().to_string()
    };
    let source = format!(
        "{}{}{}",
        PRELUDE.replace("__RESULTS_PLACEHOLDER__", &results_lit),
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
    set_global(tc, &context, "__log");

    let code = match v8::String::new(tc, &source) {
        Some(code) => code,
        None => {
            eprintln!("Error: could not allocate the program source");
            std::process::exit(1);
        }
    };
    let script = match v8::Script::compile(tc, code, None) {
        Some(script) => script,
        None => {
            eprintln!("SyntaxError: could not compile the program");
            std::process::exit(1);
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
            eprintln!("Error: program exceeded the {ROUND_TIMEOUT:?} deadline");
        } else {
            eprintln!("ExecutionError: {message}");
        }
        std::process::exit(1);
    }
    let _ = stop_tx.send(());
    // __done exits before this point on success
    eprintln!("Error: the program ended without returning");
    std::process::exit(1);
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
