//! `bise login [provider]`, `bise logout [provider]`, `bise auth list`
//! (BISE-143). API keys only (OAuth subscriptions later). A key is read
//! with the terminal's echo off (or from stdin when it is not a
//! terminal: `printf %s "$KEY" | bise login openai`), stored in
//! auth.json, and never printed.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;

use crate::auth::{loose_mode, tilde, EnvFile, Keys, Store};
use crate::{Catalog, Provider, Setup, CLI};

/// The files the commands read or write.
#[derive(Clone, Debug)]
pub struct Paths {
    /// auth.json
    pub auth_file: PathBuf,
    /// config.toml (custom providers)
    pub config: PathBuf,
    /// the old .env files, first wins
    pub env_files: Vec<PathBuf>,
    /// the home directory, to print paths as `~/...`
    pub home: Option<PathBuf>,
}

fn usage() -> String {
    format!(
        "usage: {cli} login [provider]    store a provider's API key (asked with the input hidden)
         --check                   first one tiny call to the model with it: saved only if it answers
         --model provider/model    the model of that call (default: the one in use, else the provider's pick)
         --from FILE               read the key from FILE's PROVIDER_API_KEY=... line (.env, shell rc)
       {cli} logout [provider]   remove it
       {cli} auth list           which providers have a key, and from where
       {cli} auth check [provider] [--model provider/model]
                                 one tiny call with the key bise finds; saves nothing
  A key is looked up in the environment first (the provider's variable,
  e.g. OPENAI_API_KEY), then in auth.json, then in the old .env files.
  Without a terminal, login reads the key from stdin.
  '{cli} models' lists the providers.",
        cli = CLI
    )
}

/// `bise auth <list|login|logout>`; returns the exit code.
pub fn auth_main(args: &[String], paths: &Paths, check: Checker) -> i32 {
    match args.first().map(|s| s.as_str()) {
        None | Some("list") | Some("ls") => list_main(paths),
        Some("login") => login_main(&args[1..], paths, check),
        Some("check") => check_main(&args[1..], paths, check),
        Some("logout") => logout_main(&args[1..], paths),
        Some("-h") | Some("--help") => {
            println!("{}", usage());
            0
        }
        _ => {
            eprintln!("{}", usage());
            2
        }
    }
}

/// The live key check (BISE-266's, the TUI's code): `(setup, model, key)`,
/// Err = why in plain words (never the key).
pub type Checker<'a> = &'a dyn Fn(&Setup, &str, &str) -> Result<(), String>;

/// `login`'s and `auth check`'s arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opts {
    pub provider: Option<String>,
    pub check: bool,
    pub model: Option<String>,
    pub from: Option<PathBuf>,
}

/// Parse `[provider] [--check] [--model M] [--from FILE]`; Err = exit code.
pub fn parse_opts(args: &[String]) -> Result<Opts, i32> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return Err(0);
            }
            "--check" => o.check = true,
            "--model" | "--from" => {
                let Some(v) = it.next().filter(|v| !v.starts_with('-')) else {
                    eprintln!("{} needs a value\n{}", a, usage());
                    return Err(2);
                };
                if a == "--model" {
                    o.model = Some(v.clone());
                } else {
                    o.from = Some(PathBuf::from(v));
                }
            }
            s if !s.starts_with('-') && o.provider.is_none() => o.provider = Some(s.to_string()),
            _ => {
                eprintln!("{}", usage());
                return Err(2);
            }
        }
    }
    Ok(o)
}

/// The model a check calls for provider `p`: `--model` (of that
/// provider), else the model in use when it is `p`'s, else `p`'s pick.
pub fn check_model_for(setup: &Setup, p: &Provider, asked: Option<&str>) -> Result<String, String> {
    if let Some(m) = asked {
        let r = setup.catalog.resolve(m);
        if r.provider != p.id {
            return Err(format!("{} is not a {} model", m, p.id));
        }
        return Ok(m.to_string());
    }
    let r = setup.catalog.resolve(&setup.model);
    if r.provider == p.id {
        return Ok(setup.model.clone());
    }
    if !p.model.is_empty() {
        return Ok(format!("{}/{}", p.id, p.model));
    }
    Err(format!("which {} model should the check call? give --model {}/<model>", p.id, p.id))
}

/// The key in `file` under `p`'s variable (or one of its aliases):
/// `.env` or shell lines (`export X=...`, quotes off). Err never holds
/// the key.
pub fn key_from_file(file: &std::path::Path, p: &Provider, home: Option<&std::path::Path>) -> Result<String, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("cannot read {}: {}", tilde(file, home), e))?;
    let f = EnvFile::parse(file.to_path_buf(), &text);
    crate::auth::env_names(&p.key_env)
        .iter()
        .find_map(|n| f.vars.get(*n).filter(|v| !v.trim().is_empty()).cloned())
        .ok_or_else(|| format!("{} has no {} line", tilde(file, home), p.key_env))
}

/// The provider argument, or None; Err = exit code.
fn one_arg(args: &[String]) -> Result<Option<String>, i32> {
    match args {
        [] => Ok(None),
        [a] if a == "-h" || a == "--help" => {
            println!("{}", usage());
            Err(0)
        }
        [a] if !a.starts_with('-') => Ok(Some(a.clone())),
        _ => {
            eprintln!("{}", usage());
            Err(2)
        }
    }
}

fn read_store(paths: &Paths) -> Result<Store, i32> {
    Store::read(&paths.auth_file).map_err(|e| {
        eprintln!("cannot read the key store: {} (fix it or delete it)", e);
        1
    })
}

fn real_env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

/// The providers a key can be stored for: they need one.
fn keyed(c: &Catalog) -> Vec<&Provider> {
    c.providers.iter().filter(|p| !p.key_env.is_empty()).collect()
}

/// Check a provider id for login/logout: Err = the message.
pub fn check_provider<'a>(c: &'a Catalog, id: &str) -> Result<&'a Provider, String> {
    match c.provider(id) {
        None => Err(format!(
            "unknown provider '{}': '{} models' lists them; a custom one goes in config.toml as [providers.{}]",
            id, CLI, id
        )),
        Some(p) if p.key_env.is_empty() => Err(format!("{} ({}) needs no key", p.id, p.name)),
        Some(p) => Ok(p),
    }
}

/// A key as typed or pasted: trimmed; Err (without the key) when it is
/// empty or holds a space or a control character.
pub fn clean_key(raw: &str) -> Result<String, String> {
    let k = raw.trim().trim_matches('"').trim_matches('\'');
    if k.is_empty() {
        return Err("no key given: nothing saved".into());
    }
    if k.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("the key holds a space or a control character: nothing saved".into());
    }
    Ok(k.to_string())
}

/// Store `key` for provider `p` in auth.json; the lines to print.
pub fn login(paths: &Paths, p: &Provider, key: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<Vec<String>, String> {
    let mut store = Store::read(&paths.auth_file)?;
    let key = clean_key(key)?;
    store.set(&p.id, &key);
    store
        .write(&paths.auth_file)
        .map_err(|e| format!("cannot write {}: {}", paths.auth_file.display(), e))?;
    let mut out = vec![format!(
        "saved the {} key in {}",
        p.name,
        tilde(&paths.auth_file, paths.home.as_deref())
    )];
    out.extend(env_note(p, &key, env));
    if !p.needs.is_empty() {
        out.push(format!("note: {} is not usable yet ({})", p.id, p.needs));
    }
    out.push("a running hub gives it to the agents it starts from now on (a running agent keeps the key it started with)".into());
    Ok(out)
}

/// "X in the environment holds another key: bise uses this one" (BISE-269:
/// auth.json wins).
fn env_note(p: &Provider, key: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let names = crate::auth::env_names(&p.key_env);
    let set = names.iter().find(|n| env(n).is_some_and(|v| !v.trim().is_empty() && v.trim() != key.trim()))?;
    Some(format!(
        "note: {} in the environment holds another key: bise uses this one (`{} logout {}` goes back to it)",
        set, CLI, p.id
    ))
}

/// Remove provider `id`'s key from auth.json; the lines to print.
pub fn logout(paths: &Paths, id: &str, env: &dyn Fn(&str) -> Option<String>, c: &Catalog) -> Result<Vec<String>, String> {
    let mut store = Store::read(&paths.auth_file)?;
    if !store.remove(id) {
        return Err(format!(
            "no key stored for '{}' in {}",
            id,
            tilde(&paths.auth_file, paths.home.as_deref())
        ));
    }
    store
        .write(&paths.auth_file)
        .map_err(|e| format!("cannot write {}: {}", paths.auth_file.display(), e))?;
    let mut out = vec![format!("removed the {} key", id)];
    if let Some(p) = c.provider(id) {
        let files = EnvFile::read_all(&paths.env_files);
        let keys = Keys { env, store: &store, files: &files };
        if let Some(f) = keys.for_provider(p) {
            out.push(format!("{} still has a key: {}", id, f.from.describe(paths.home.as_deref())));
        }
    }
    Ok(out)
}

/// `bise auth list`, pure: one line per provider that takes a key.
pub fn render_list(c: &Catalog, keys: &Keys, paths: &Paths) -> String {
    let home = paths.home.as_deref();
    let mut o = String::new();
    let ps = keyed(c);
    let w = ps.iter().map(|p| p.id.len()).max().unwrap_or(8);
    let we = ps.iter().map(|p| p.key_env.len()).max().unwrap_or(8);
    for p in &ps {
        let from = keys.source(p, home).unwrap_or_else(|| "-".into());
        o.push_str(&format!("{:<w$}  {:<we$}  {}\n", p.id, p.key_env, from, w = w, we = we));
    }
    for id in keys.store.providers() {
        match c.provider(id) {
            None => o.push_str(&format!("warning: auth.json has '{}', a provider bise does not know (ignored)\n", id)),
            Some(_) if keys.store.key(id).is_none() => {
                o.push_str(&format!("warning: auth.json's '{}' entry is not an API key (ignored)\n", id))
            }
            _ => {}
        }
    }
    o.push_str(&format!("\nkeys: {}\n", tilde(&paths.auth_file, home)));
    if let Some(m) = loose_mode(&paths.auth_file) {
        o.push_str(&format!(
            "warning: {} is readable by others (mode {:o}): chmod 600 it\n",
            tilde(&paths.auth_file, home),
            m
        ));
    }
    o
}

fn list_main(paths: &Paths) -> i32 {
    let setup = Setup::load(&paths.config);
    let store = match read_store(paths) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env: &real_env, store: &store, files: &files };
    print!("{}", render_list(&setup.catalog, &keys, paths));
    0
}

/// `bise login [provider]`; returns the exit code.
pub fn login_main(args: &[String], paths: &Paths, check: Checker) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let arg = opts.provider.clone();
    let setup = Setup::load(&paths.config);
    let c = &setup.catalog;
    if let Err(code) = read_store(paths) {
        return code;
    }
    let tty = std::io::stdin().is_terminal();
    let id = match arg {
        Some(id) => id,
        None if tty => match choose_provider(c) {
            Some(id) => id,
            None => return 1,
        },
        None => {
            eprintln!("{} login: give the provider when stdin is not a terminal", CLI);
            return 2;
        }
    };
    let p = match check_provider(c, &id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let model = match (opts.check || opts.model.is_some()).then(|| check_model_for(&setup, p, opts.model.as_deref())) {
        Some(Err(e)) => {
            eprintln!("{}", e);
            return 2;
        }
        Some(Ok(m)) => Some(m),
        None => None,
    };
    let raw = if let Some(file) = &opts.from {
        match key_from_file(file, p, paths.home.as_deref()) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("{}", e);
                return 1;
            }
        }
    } else if tty {
        match read_hidden(&format!("API key for {} ({}, input hidden): ", p.name, p.key_env)) {
            Ok(Some(k)) => k,
            Ok(None) => {
                eprintln!("cancelled: nothing saved");
                return 1;
            }
            Err(e) => {
                eprintln!("cannot read the key: {}", e);
                return 1;
            }
        }
    } else {
        let mut s = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut s) {
            eprintln!("cannot read the key from stdin: {}", e);
            return 1;
        }
        s
    };
    if let Some(m) = &model {
        let key = match clean_key(&raw) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("{}", e);
                return 1;
            }
        };
        if let Err(e) = check(&setup, m, &key) {
            eprintln!("{}: nothing saved", e);
            return 1;
        }
        println!("{} answered with this key", m);
    }
    match login(paths, p, &raw, &real_env) {
        Ok(lines) => {
            for l in lines {
                println!("{}", l);
            }
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

/// `bise auth check [provider] [--model M]`: the live check with the key
/// bise finds (env, auth.json, an old .env file), nothing saved; the exit
/// code (0: it answered).
pub fn check_main(args: &[String], paths: &Paths, check: Checker) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(code) => return code,
    };
    if opts.from.is_some() {
        eprintln!("{}", usage());
        return 2;
    }
    let setup = Setup::load(&paths.config);
    let c = &setup.catalog;
    let id = match (&opts.provider, &opts.model) {
        (Some(id), _) => id.clone(),
        (None, Some(m)) => c.resolve(m).provider,
        (None, None) => c.resolve(&setup.model).provider,
    };
    let p = match check_provider(c, &id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let model = match check_model_for(&setup, p, opts.model.as_deref()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let store = match read_store(paths) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env: &real_env, store: &store, files: &files };
    let Some(found) = keys.for_provider(p) else {
        eprintln!("no {} key: {} or '{} login {}'", p.id, p.key_env, CLI, p.id);
        return 1;
    };
    let from = found.from.describe(paths.home.as_deref());
    match check(&setup, &model, &found.key) {
        Ok(()) => {
            println!("{} answered with the key from {}", model, from);
            0
        }
        Err(e) => {
            eprintln!("{} (the key from {})", e, from);
            1
        }
    }
}

/// `bise logout [provider]`; returns the exit code.
pub fn logout_main(args: &[String], paths: &Paths) -> i32 {
    let arg = match one_arg(args) {
        Ok(a) => a,
        Err(code) => return code,
    };
    let setup = Setup::load(&paths.config);
    let store = match read_store(paths) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let id = match arg {
        Some(id) => id,
        None => {
            let stored = store.providers();
            match stored.as_slice() {
                [] => {
                    eprintln!("no key stored in {}", tilde(&paths.auth_file, paths.home.as_deref()));
                    return 1;
                }
                [one] => one.to_string(),
                many => {
                    eprintln!("keys stored for: {}; say which: {} logout <provider>", many.join(", "), CLI);
                    return 2;
                }
            }
        }
    };
    match logout(paths, &id, &real_env, &setup.catalog) {
        Ok(lines) => {
            for l in lines {
                println!("{}", l);
            }
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

/// Ask which provider (a number or an id); None = cancelled.
fn choose_provider(c: &Catalog) -> Option<String> {
    let ps = keyed(c);
    let store = Store::default();
    let keys = Keys { env: &real_env, store: &store, files: &[] };
    let mut err = std::io::stderr();
    for (i, p) in ps.iter().enumerate() {
        let mut tags = Vec::new();
        if keys.for_provider(p).is_some() {
            tags.push("key in the environment".to_string());
        }
        if !p.needs.is_empty() {
            tags.push(format!("not usable yet, {}", p.needs));
        }
        let tags = if tags.is_empty() { String::new() } else { format!("  ({})", tags.join("; ")) };
        let _ = writeln!(err, "{:>3}. {:<12} {}{}", i + 1, p.id, p.name, tags);
    }
    let _ = write!(err, "provider (number or id): ");
    let _ = err.flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).ok()? == 0 {
        let _ = writeln!(err);
        return None;
    }
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    match line.parse::<usize>() {
        Ok(n) if n >= 1 && n <= ps.len() => Some(ps[n - 1].id.clone()),
        Ok(_) => {
            let _ = writeln!(err, "no provider number {}", line);
            None
        }
        Err(_) => Some(line.to_string()),
    }
}

/// Read one line from the terminal with the echo off. Ctrl-C, Esc or
/// Ctrl-D on an empty line: None. The terminal is restored before
/// returning; nothing typed is ever shown.
#[cfg(unix)]
fn read_hidden(prompt: &str) -> std::io::Result<Option<String>> {
    let mut err = std::io::stderr();
    write!(err, "{}", prompt)?;
    err.flush()?;
    let fd = 0;
    // SAFETY: tcgetattr/tcsetattr on stdin with a zeroed termios they fill.
    let mut old: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut old) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut raw = old;
    raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
    raw.c_cc[libc::VMIN] = 1;
    raw.c_cc[libc::VTIME] = 0;
    if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &raw) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut stdin = std::io::stdin().lock();
    let res = loop {
        let mut b = [0u8; 1];
        match stdin.read(&mut b) {
            Ok(0) => break Ok(if buf.is_empty() { None } else { Some(()) }),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => break Err(e),
            Ok(_) => match b[0] {
                b'\r' | b'\n' => break Ok(Some(())),
                3 | 0x1b => break Ok(None),                   // Ctrl-C, Esc
                4 if buf.is_empty() => break Ok(None),         // Ctrl-D
                0x7f | 8 => {
                    // backspace: drop one UTF-8 char
                    while let Some(c) = buf.pop() {
                        if c & 0xC0 != 0x80 {
                            break;
                        }
                    }
                }
                0x15 => buf.clear(), // Ctrl-U
                c => buf.push(c),
            },
        }
    };
    unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &old) };
    let _ = writeln!(err);
    match res {
        Ok(Some(())) => Ok(Some(String::from_utf8_lossy(&buf).into_owned())),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(not(unix))]
fn read_hidden(prompt: &str) -> std::io::Result<Option<String>> {
    let _ = prompt;
    Err(std::io::Error::other("hidden input needs a Unix terminal; pipe the key on stdin"))
}

