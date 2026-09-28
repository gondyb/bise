//! Where a workspace's hub keeps its state (RFC 0001 §5):
//! `$SB_STATE_DIR`, else `$XDG_STATE_HOME/switchboard/<name>-<hash>`,
//! else `~/.local/state/switchboard/<name>-<hash>`.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub workspace: PathBuf,
    pub state: PathBuf,
}

/// FNV-1a, 64 bits: a stable id for a workspace path.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

pub fn workspace_id(workspace: &Path) -> String {
    let base = workspace
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "root".to_string());
    let base: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .take(32)
        .collect();
    format!("{}-{:08x}", base, fnv1a(&workspace.to_string_lossy()) as u32)
}

impl Paths {
    pub fn for_workspace(workspace: &Path) -> Paths {
        let workspace = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
        let state = match std::env::var("SB_STATE_DIR") {
            Ok(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                let root = match std::env::var("XDG_STATE_HOME") {
                    Ok(d) if !d.is_empty() => PathBuf::from(d),
                    _ => PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
                        .join(".local/state"),
                };
                root.join("switchboard").join(workspace_id(&workspace))
            }
        };
        Paths { workspace, state }
    }

    pub fn socket(&self) -> PathBuf {
        self.state.join("hub.sock")
    }
    pub fn journal(&self) -> PathBuf {
        self.state.join("journal.jsonl")
    }
    pub fn pid_file(&self) -> PathBuf {
        self.state.join("hub.pid")
    }
    pub fn log(&self) -> PathBuf {
        self.state.join("hub.log")
    }
    pub fn bin_dir(&self) -> PathBuf {
        self.state.join("bin")
    }
    pub fn agent_dir(&self, name: &str) -> PathBuf {
        self.state.join("agents").join(name)
    }
    pub fn worktree(&self, name: &str) -> PathBuf {
        self.state.join("worktrees").join(name)
    }
    pub fn config(&self) -> PathBuf {
        self.workspace.join(".switchboard").join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_readable() {
        let a = workspace_id(Path::new("/Users/me/my repo"));
        assert_eq!(a, workspace_id(Path::new("/Users/me/my repo")));
        assert!(a.starts_with("my-repo-"));
        assert_ne!(a, workspace_id(Path::new("/Users/you/my repo")));
    }
}
