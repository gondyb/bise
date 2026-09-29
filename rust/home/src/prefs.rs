//! The user's preferences: one `prefs.json` in the bise layout, the
//! old files in the legacy one (`tui.json`, `hints.json`, `tip`,
//! `onboarded`). A [`Slot`] hides which: a JSON value in a file, the
//! whole file or one key of its object.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pref {
    /// `voice_mode_enabled`: bool.
    Voice,
    /// `theme`: "auto" | "light" | "dark".
    Theme,
    /// `hints`: `{hint key: true}`, the one-time hints seen.
    Hints,
    /// `tip`: the index of the last tip shown.
    Tip,
    /// `onboarded`: present once the onboarding was seen.
    Onboarded,
}

impl Pref {
    /// Its key in `prefs.json` (and `tui.json` for voice and theme).
    pub fn key(self) -> &'static str {
        match self {
            Pref::Voice => "voice_mode_enabled",
            Pref::Theme => "theme",
            Pref::Hints => "hints",
            Pref::Tip => "tip",
            Pref::Onboarded => "onboarded",
        }
    }
}

/// A value kept in `file`: the whole file, or its object's `key`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub file: PathBuf,
    pub key: Option<&'static str>,
}

impl Slot {
    /// The whole file is the value.
    pub fn file(file: impl Into<PathBuf>) -> Slot {
        Slot { file: file.into(), key: None }
    }

    /// One key of the file's JSON object.
    pub fn key(file: impl Into<PathBuf>, key: &'static str) -> Slot {
        Slot { file: file.into(), key: Some(key) }
    }

    /// The value, if set and readable. A whole-file slot whose text is
    /// not JSON (an old flag file) reads as that text.
    pub fn get(&self) -> Option<Value> {
        let text = std::fs::read_to_string(&self.file).ok()?;
        match self.key {
            Some(k) => serde_json::from_str::<Value>(&text).ok()?.get(k).cloned().filter(|v| !v.is_null()),
            None => Some(serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.trim().to_string()))),
        }
    }

    /// Set the value (a keyed slot keeps the object's other keys). The
    /// file is replaced whole (tmp + rename): a reader never sees half.
    pub fn set(&self, v: Value) -> io::Result<()> {
        let text = match self.key {
            Some(k) => {
                let mut obj = std::fs::read_to_string(&self.file)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .filter(|o| o.is_object())
                    .unwrap_or_else(|| Value::Object(Default::default()));
                obj[k] = v;
                serde_json::to_string_pretty(&obj)?
            }
            None => serde_json::to_string_pretty(&v)?,
        };
        write_atomic(&self.file, &(text + "\n"))
    }
}

fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
