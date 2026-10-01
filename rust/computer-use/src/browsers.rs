//! The Chromium family (design §4.1b): where each browser lives, where its
//! native host manifests go, and the shim they point at (C4).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::paths::Paths;

/// C4: the native host's name.
pub const HOST_NAME: &str = "dev.bise.computer_use";
/// The extension id, pinned with `key` in computer-use/extension/manifest.json.
pub const EXTENSION_ID: &str = "bogffepmbkbmbfejcadaipgphgkocgob";
/// The oldest Chrome major the extension supports (MV3, tab groups,
/// `chrome.debugger` focus emulation, a native port keeping the worker alive).
pub const MIN_MAJOR: u32 = 116;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Browser {
    /// the C4 `hello.browser` key
    pub key: &'static str,
    /// the name bise shows ("↖ driving Edge · github.com")
    pub name: &'static str,
    /// its folder under `~/Library/Application Support`
    pub support: &'static str,
    /// the app bundle under /Applications
    pub app: &'static str,
    pub bundle_id: &'static str,
}

impl Browser {
    /// `~/Library/Application Support/<support>/NativeMessagingHosts`
    pub fn hosts_dir(&self, paths: &Paths) -> PathBuf {
        paths.app_support().join(self.support).join("NativeMessagingHosts")
    }

    pub fn manifest_path(&self, paths: &Paths) -> PathBuf {
        self.hosts_dir(paths).join(format!("{}.json", HOST_NAME))
    }

    /// Installed: its profile folder exists (it ran once for this user).
    pub fn installed(&self, paths: &Paths) -> bool {
        paths.app_support().join(self.support).is_dir()
    }
}

/// In the order setup proposes them. Arc reads `Arc/User Data` (checked on
/// a real install); its tabs open without a group (no real tab groups).
pub const ALL: [Browser; 6] = [
    Browser { key: "chrome", name: "Chrome", support: "Google/Chrome", app: "Google Chrome.app", bundle_id: "com.google.Chrome" },
    Browser { key: "edge", name: "Edge", support: "Microsoft Edge", app: "Microsoft Edge.app", bundle_id: "com.microsoft.edgemac" },
    Browser { key: "brave", name: "Brave", support: "BraveSoftware/Brave-Browser", app: "Brave Browser.app", bundle_id: "com.brave.Browser" },
    Browser { key: "vivaldi", name: "Vivaldi", support: "Vivaldi", app: "Vivaldi.app", bundle_id: "com.vivaldi.Vivaldi" },
    Browser { key: "opera", name: "Opera", support: "com.operasoftware.Opera", app: "Opera.app", bundle_id: "com.operasoftware.Opera" },
    Browser { key: "arc", name: "Arc", support: "Arc/User Data", app: "Arc.app", bundle_id: "company.thebrowser.Browser" },
];

pub fn by_key(key: &str) -> Option<Browser> {
    ALL.iter().copied().find(|b| b.key.eq_ignore_ascii_case(key) || b.name.eq_ignore_ascii_case(key))
}

/// The browser whose app bundle holds `exe` (the native host's parent
/// process): it names Vivaldi and Arc, which say `chrome` (C4).
pub fn from_exe(exe: &str) -> Option<Browser> {
    ALL.iter().copied().find(|b| exe.contains(&format!("/{}/", b.app)))
}

/// The C4 manifest for one browser.
pub fn manifest(shim: &Path) -> Value {
    json!({
        "name": HOST_NAME,
        "description": "bise computer use",
        "path": shim,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{}/", EXTENSION_ID)],
    })
}

/// `ok`, `missing`, or `stale` (another path or extension id).
pub fn manifest_state(b: &Browser, paths: &Paths) -> &'static str {
    match std::fs::read_to_string(b.manifest_path(paths)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        None => "missing",
        Some(v) if v == manifest(&paths.shim()) && paths.shim().exists() => "ok",
        Some(_) => "stale",
    }
}

/// The shim: `~/.bise/bin/bise-chrome-host` runs the install's `current`
/// bise (it follows version switches), else the bise that wrote it.
pub fn shim_text(current: Option<&Path>, fallback: &Path) -> String {
    let q = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"));
    let mut s = String::from("#!/bin/sh\n# bise computer use: the browsers' native host (C4). Written by `bise computer-use repair`.\n");
    if let Some(c) = current {
        s.push_str(&format!("exe={}/bise\n[ -x \"$exe\" ] || exe={}\n", q(c), q(fallback)));
    } else {
        s.push_str(&format!("exe={}\n", q(fallback)));
    }
    s.push_str("exec \"$exe\" computer-use chrome-host \"$@\"\n");
    s
}

/// The bise to run: `<prefix>/current` of an install, and this executable.
pub fn exe_and_current() -> (PathBuf, Option<PathBuf>) {
    let exe = std::env::current_exe().ok().map(|p| p.canonicalize().unwrap_or(p)).unwrap_or_else(|| PathBuf::from("bise"));
    let current = exe
        .parent()
        .and_then(bise_home::release::Install::of_root)
        .map(|i| i.current_link());
    (exe, current)
}

/// Write the shim and every installed browser's manifest. Returns what it wrote.
pub fn repair(paths: &Paths, exe: &Path, current: Option<&Path>) -> Result<Value, String> {
    let shim = paths.shim();
    if let Some(d) = shim.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {}", d.display(), e))?;
    }
    std::fs::write(&shim, shim_text(current, exe)).map_err(|e| format!("{}: {}", shim.display(), e))?;
    crate::paths::private(&shim, 0o755).map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for b in ALL.iter().filter(|b| b.installed(paths)) {
        let dir = b.hosts_dir(paths);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
        let f = b.manifest_path(paths);
        let text = serde_json::to_string_pretty(&manifest(&shim)).unwrap_or_default() + "\n";
        std::fs::write(&f, text).map_err(|e| format!("{}: {}", f.display(), e))?;
        written.push(json!({"browser": b.name, "manifest": f}));
    }
    Ok(json!({"shim": shim, "manifests": written}))
}

/// `CFBundleShortVersionString` of an app bundle.
pub fn app_version(app: &Path) -> Option<String> {
    let out = std::process::Command::new("plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// Where the app is: /Applications, then ~/Applications.
pub fn app_path(b: &Browser, paths: &Paths) -> Option<PathBuf> {
    [PathBuf::from("/Applications"), paths.home.join("Applications")]
        .into_iter()
        .map(|d| d.join(b.app))
        .find(|p| p.is_dir())
}

/// Whether the browser runs (its main executable, by bundle path).
pub fn running(b: &Browser) -> bool {
    let out = std::process::Command::new("pgrep").args(["-f", &format!("/{}/Contents/MacOS/", b.app)]).output();
    out.map(|o| o.status.success()).unwrap_or(false)
}

pub fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_and_shim() {
        let d = std::env::temp_dir().join(format!("cu-br-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = Paths::new(d.join("run"), d.join("bise"), d.join("home"));
        for s in ["Google/Chrome", "Arc/User Data"] {
            std::fs::create_dir_all(p.app_support().join(s)).unwrap();
        }
        let chrome = by_key("chrome").unwrap();
        assert_eq!(manifest_state(&chrome, &p), "missing");
        let out = repair(&p, Path::new("/opt/bise/bise"), Some(Path::new("/opt/x/current"))).unwrap();
        assert_eq!(out["manifests"].as_array().unwrap().len(), 2);
        assert_eq!(manifest_state(&chrome, &p), "ok");
        assert_eq!(manifest_state(&by_key("edge").unwrap(), &p), "missing");
        let arc = by_key("Arc").unwrap();
        assert!(arc.manifest_path(&p).ends_with("Arc/User Data/NativeMessagingHosts/dev.bise.computer_use.json"));
        let m: Value = serde_json::from_str(&std::fs::read_to_string(chrome.manifest_path(&p)).unwrap()).unwrap();
        assert_eq!(m["allowed_origins"][0], "chrome-extension://bogffepmbkbmbfejcadaipgphgkocgob/");
        assert_eq!(m["path"], p.shim().to_string_lossy().as_ref());
        let shim = std::fs::read_to_string(p.shim()).unwrap();
        assert!(shim.contains("exe='/opt/x/current'/bise"), "{}", shim);
        assert!(shim.contains("computer-use chrome-host"));
        std::fs::write(chrome.manifest_path(&p), "{}").unwrap();
        assert_eq!(manifest_state(&chrome, &p), "stale");
        assert_eq!(from_exe("/Applications/Vivaldi.app/Contents/MacOS/Vivaldi").map(|b| b.name), Some("Vivaldi"));
        assert_eq!(from_exe("/usr/bin/true"), None);
        assert_eq!(major("154.0.7000.1"), Some(154));
        let _ = std::fs::remove_dir_all(&d);
    }
}
