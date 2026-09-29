//! The enable state: `~/.bend-harness/plugins.json`,
//! `{"disabled": ["name", ...]}`. A missing or unreadable file means
//! every plugin is enabled.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// `$BEND_PLUGINS_STATE`, else bise's `plugins.json` (`bise_home`).
pub fn state_path() -> PathBuf {
    bise_home::Home::from_env().plugins_state()
}

pub fn disabled(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    v.get("disabled")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Enable (`on`) or disable a plugin by name. Returns whether the file
/// changed. Other keys in the file are kept.
pub fn set_enabled(path: &Path, name: &str, on: bool) -> std::io::Result<bool> {
    let mut doc: Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let mut list = disabled(path);
    let had = list.iter().any(|n| n == name);
    if on == !had {
        return Ok(false);
    }
    if on {
        list.retain(|n| n != name);
    } else {
        list.push(name.to_string());
        list.sort();
    }
    doc["disabled"] = json!(list);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n")?;
    std::fs::rename(&tmp, path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_and_keeps_other_keys() {
        let dir = std::env::temp_dir().join(format!("bp-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("plugins.json");
        assert!(disabled(&p).is_empty());
        assert!(set_enabled(&p, "b", false).unwrap());
        assert!(set_enabled(&p, "a", false).unwrap());
        assert!(!set_enabled(&p, "a", false).unwrap());
        assert_eq!(disabled(&p), vec!["a", "b"]);
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        v["other"] = json!(1);
        std::fs::write(&p, v.to_string()).unwrap();
        assert!(set_enabled(&p, "a", true).unwrap());
        assert_eq!(disabled(&p), vec!["b"]);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["other"], json!(1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
