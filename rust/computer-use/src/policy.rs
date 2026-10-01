//! A work or school browser: the organisation's policies can block
//! computer use (docs/computer-use-ship.md §3). The browser then only
//! fails quietly (the extension isn't allowed, its host never starts, the
//! debugger never attaches) and setup would wait forever. setup-check
//! reads the policies macOS gives the browser (`/Library/Managed
//! Preferences`, the per-user folder first) and says which one blocks.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::browsers::{Browser, EXTENSION_ID, HOST_NAME};

/// Where macOS puts a managed browser's mandatory policies: per user, then
/// for the machine (MDM profiles write both).
pub fn managed_dirs() -> Vec<PathBuf> {
    let root = PathBuf::from("/Library/Managed Preferences");
    let mut v = Vec::new();
    if let Ok(u) = std::env::var("USER") {
        if !u.is_empty() {
            v.push(root.join(u));
        }
    }
    v.push(root);
    v
}

/// The policies of `b` from the first plist found in `dirs`; Null when none.
pub fn read(b: &Browser, dirs: &[PathBuf]) -> Value {
    dirs.iter().map(|d| d.join(format!("{}.plist", b.bundle_id))).find(|p| p.is_file()).map(|p| plist_json(&p)).unwrap_or(Value::Null)
}

/// A plist as JSON (`plutil`, binary or XML); Null when it can't be read.
fn plist_json(p: &Path) -> Value {
    std::process::Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(p)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| serde_json::from_slice(&o.stdout).ok())
        .unwrap_or(Value::Null)
}

fn list_has(v: &Value, key: &str, item: &str) -> bool {
    v[key].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(item)))
}

/// What blocks computer use in these policies, as the setup row says it
/// ("your organisation blocks ... in Chrome"), with the policy's name; None
/// when nothing does.
pub fn blocks(p: &Value, browser: &str) -> Option<String> {
    let org = |what: &str, policy: &str| Some(format!("your organisation blocks {} in {} ({}); use a browser or profile it doesn't manage", what, browser, policy));
    // the extension: installs blocked unless allowed by id
    let allowed = list_has(p, "ExtensionInstallAllowlist", EXTENSION_ID) || list_has(p, "ExtensionInstallWhitelist", EXTENSION_ID);
    let settings = &p["ExtensionSettings"];
    let mode = |k: &str| settings[k]["installation_mode"].as_str().unwrap_or("");
    let ours = mode(EXTENSION_ID);
    if ours == "blocked" || ours == "removed" {
        return org("the bise extension", "ExtensionSettings");
    }
    let ours_ok = matches!(ours, "allowed" | "force_installed" | "normal_installed");
    if !allowed && !ours_ok {
        for list in ["ExtensionInstallBlocklist", "ExtensionInstallBlacklist"] {
            if list_has(p, list, "*") || list_has(p, list, EXTENSION_ID) {
                return org("extensions it hasn't approved", list);
            }
        }
        if mode("*") == "blocked" {
            return org("extensions it hasn't approved", "ExtensionSettings");
        }
    }
    // the native host: the extension's line to bise
    let host_ok = list_has(p, "NativeMessagingAllowlist", HOST_NAME) || list_has(p, "NativeMessagingWhitelist", HOST_NAME);
    if !host_ok {
        for list in ["NativeMessagingBlocklist", "NativeMessagingBlacklist"] {
            if list_has(p, list, "*") || list_has(p, list, HOST_NAME) {
                return org("extensions from talking to apps on this Mac", list);
            }
        }
    }
    if p["NativeMessagingUserLevelHosts"] == Value::Bool(false) {
        return org("apps installed for one user from talking to extensions", "NativeMessagingUserLevelHosts");
    }
    // the debugger: how the extension drives a tab
    if p["DeveloperToolsAvailability"].as_i64() == Some(2) {
        return org("the debugger extensions use to drive tabs", "DeveloperToolsAvailability");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn what_blocks_and_what_does_not() {
        let b = |p: Value| blocks(&p, "Chrome");
        assert_eq!(b(Value::Null), None);
        assert_eq!(b(json!({"HomepageLocation": "https://intranet"})), None);
        // installs
        assert!(b(json!({"ExtensionInstallBlocklist": ["*"]})).unwrap().contains("extensions it hasn't approved in Chrome (ExtensionInstallBlocklist)"));
        assert_eq!(b(json!({"ExtensionInstallBlocklist": ["*"], "ExtensionInstallAllowlist": [EXTENSION_ID]})), None);
        assert!(b(json!({"ExtensionInstallBlacklist": [EXTENSION_ID]})).is_some());
        assert!(b(json!({"ExtensionSettings": {"*": {"installation_mode": "blocked"}}})).unwrap().contains("ExtensionSettings"));
        assert_eq!(b(json!({"ExtensionSettings": {"*": {"installation_mode": "blocked"}, EXTENSION_ID: {"installation_mode": "allowed"}}})), None);
        assert!(b(json!({"ExtensionSettings": {EXTENSION_ID: {"installation_mode": "blocked"}}})).unwrap().contains("the bise extension"));
        // native messaging
        assert!(b(json!({"NativeMessagingBlocklist": ["*"]})).unwrap().contains("talking to apps"));
        assert_eq!(b(json!({"NativeMessagingBlocklist": ["*"], "NativeMessagingAllowlist": [HOST_NAME]})), None);
        assert!(b(json!({"NativeMessagingUserLevelHosts": false})).unwrap().contains("NativeMessagingUserLevelHosts"));
        assert_eq!(b(json!({"NativeMessagingUserLevelHosts": true})), None);
        // the debugger
        assert!(b(json!({"DeveloperToolsAvailability": 2})).unwrap().contains("debugger"));
        assert_eq!(b(json!({"DeveloperToolsAvailability": 1})), None);
    }

    #[test]
    fn reads_a_managed_plist() {
        let d = std::env::temp_dir().join(format!("cu-policy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (user, machine) = (d.join("user"), d.join("machine"));
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&machine).unwrap();
        let chrome = &crate::browsers::ALL[0];
        let file = machine.join("com.google.Chrome.plist");
        std::fs::write(&file, r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>DeveloperToolsAvailability</key><integer>2</integer>
<key>ExtensionInstallBlocklist</key><array><string>*</string></array>
<key>ExtensionInstallAllowlist</key><array><string>bogffepmbkbmbfejcadaipgphgkocgob</string></array>
</dict></plist>"#)
            .unwrap();
        // binary, like MDM writes them
        assert!(std::process::Command::new("plutil").args(["-convert", "binary1"]).arg(&file).status().unwrap().success());
        let p = read(chrome, &[user.clone(), machine.clone()]);
        assert_eq!(p["DeveloperToolsAvailability"], 2);
        assert!(blocks(&p, "Chrome").unwrap().contains("DeveloperToolsAvailability"));
        // the per-user file wins
        std::fs::write(user.join("com.google.Chrome.plist"), r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>BookmarkBarEnabled</key><true/></dict></plist>"#)
            .unwrap();
        assert_eq!(blocks(&read(chrome, &[user, machine]), "Chrome"), None);
        assert_eq!(read(chrome, &[d.join("none")]), Value::Null);
        let _ = std::fs::remove_dir_all(&d);
    }
}
