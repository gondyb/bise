//! AGENTS.md: the instructions a user or a repo gives every agent
//! (BISE-232), read the way Codex reads them (codex-rs
//! `core/src/agents_md.rs`, `codex-home/src/instructions`):
//!
//! 1. global: `$BISE_HOME/AGENTS.override.md`, else `$BISE_HOME/AGENTS.md`
//!    (the first one with text);
//! 2. project: the root is the nearest folder, from the agent's working
//!    folder up, that holds a `.git` (file or folder: a worktree has a
//!    `.git` file); no root: the working folder alone. In each folder from
//!    the root down to the working folder, the first file of
//!    `AGENTS.override.md`, `AGENTS.md`, then `project_doc_fallback_filenames`;
//! 3. the project files, root first, share a budget of
//!    `project_doc_max_bytes` (32 KiB): the file that crosses it is cut,
//!    the next ones are dropped. The global file is outside the budget.
//!
//! The result is one prompt section ([`section`]), written per REPL start
//! to a file the REPL reads (`BEND_AGENTS_MD`, bend/runtime/repl-live.bend).
//! docs/agents-md.md compares Codex and Vibe.

use std::path::{Path, PathBuf};

pub const DEFAULT_FILENAME: &str = "AGENTS.md";
pub const OVERRIDE_FILENAME: &str = "AGENTS.override.md";
/// Codex's `project_doc_max_bytes` default.
pub const DEFAULT_MAX_BYTES: usize = 32 * 1024;
/// Where the REPL finds the section (a file path; empty file: nothing).
pub const ENV: &str = "BEND_AGENTS_MD";

/// The top-level keys of config.toml that change the discovery (Codex's
/// names).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Budget of the project files, in bytes; 0: no project file.
    pub max_bytes: usize,
    /// Names tried after AGENTS.override.md and AGENTS.md (e.g. CLAUDE.md).
    pub fallback_filenames: Vec<String>,
    /// What marks the project root; empty: the working folder alone.
    pub root_markers: Vec<String>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            max_bytes: DEFAULT_MAX_BYTES,
            fallback_filenames: Vec::new(),
            root_markers: vec![".git".to_string()],
        }
    }
}

fn toml_list(v: &str) -> Option<Vec<String>> {
    let v = v.trim();
    let inner = v.strip_prefix('[')?.split(']').next()?;
    Some(
        inner
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

impl Config {
    /// The keys before the first `[table]` of config.toml; a bad value
    /// keeps the default.
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for raw in text.lines() {
            let l = raw.trim();
            if l.starts_with('[') {
                break;
            }
            let Some((k, v)) = l.split_once('=') else {
                continue;
            };
            let v = v.split(" #").next().unwrap_or(v);
            match k.trim() {
                "project_doc_max_bytes" => {
                    if let Ok(n) = v.trim().replace('_', "").parse() {
                        c.max_bytes = n;
                    }
                }
                "project_doc_fallback_filenames" => {
                    if let Some(l) = toml_list(v) {
                        c.fallback_filenames = l;
                    }
                }
                "project_root_markers" => {
                    if let Some(l) = toml_list(v) {
                        c.root_markers = l;
                    }
                }
                _ => {}
            }
        }
        c
    }

    /// The names tried in each folder, in order. A fallback that is not a
    /// plain file name is ignored (Codex does the same).
    fn candidates(&self) -> Vec<&str> {
        let mut names = vec![OVERRIDE_FILENAME, DEFAULT_FILENAME];
        for n in &self.fallback_filenames {
            let n = n.as_str();
            if n.is_empty() || n == "." || n == ".." || n.contains(['/', '\0']) {
                continue;
            }
            if !names.contains(&n) {
                names.push(n);
            }
        }
        names
    }
}

/// One file that made it into the prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doc {
    pub path: PathBuf,
    pub text: String,
}

/// The global file of bise's home folder.
pub fn global(home: &Path) -> Option<Doc> {
    [OVERRIDE_FILENAME, DEFAULT_FILENAME].iter().find_map(|n| {
        let path = home.join(n);
        if !path.is_file() {
            return None;
        }
        let bytes = std::fs::read(&path).ok()?;
        let text = String::from_utf8_lossy(&bytes).trim().to_string();
        (!text.is_empty()).then_some(Doc { path, text })
    })
}

/// The folders searched: the project root down to `cwd`, root first.
pub fn search_dirs(cwd: &Path, markers: &[String]) -> Vec<PathBuf> {
    let root = cwd
        .ancestors()
        .find(|d| markers.iter().any(|m| d.join(m).exists()));
    let Some(root) = root else {
        return vec![cwd.to_path_buf()];
    };
    let mut dirs: Vec<PathBuf> = cwd
        .ancestors()
        .take_while(|d| *d != root)
        .map(Path::to_path_buf)
        .collect();
    dirs.push(root.to_path_buf());
    dirs.reverse();
    dirs
}

/// `s` cut to at most `n` bytes, on a character boundary.
fn cut(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut i = n;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    &s[..i]
}

/// The project files for an agent working in `cwd`, root first, within
/// the budget.
pub fn project(cwd: &Path, cfg: &Config) -> Vec<Doc> {
    let mut remaining = cfg.max_bytes;
    let mut docs = Vec::new();
    if remaining == 0 {
        return docs;
    }
    let names = cfg.candidates();
    for dir in search_dirs(cwd, &cfg.root_markers) {
        if remaining == 0 {
            break;
        }
        let Some(path) = names.iter().map(|n| dir.join(n)).find(|p| p.is_file()) else {
            continue;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let all = String::from_utf8_lossy(&bytes);
        let text = cut(&all, remaining);
        if text.trim().is_empty() {
            continue;
        }
        remaining -= text.len();
        docs.push(Doc {
            path,
            text: text.trim_end().to_string(),
        });
    }
    docs
}

/// What the model reads: Codex's block (`# AGENTS.md instructions for
/// <cwd>`, the files inside `<INSTRUCTIONS>`, the global one first and
/// `--- project-doc ---` before the project ones), each file under the
/// line that names it (Vibe does that), and Codex's rules of scope and
/// precedence first. Empty when there is no file.
pub fn section(cwd: &Path, global: Option<&Doc>, project: &[Doc]) -> String {
    if global.is_none() && project.is_empty() {
        return String::new();
    }
    let mut body: Vec<String> = Vec::new();
    if let Some(g) = global {
        body.push(format!(
            "Contents of {} (the user's own, for every project):\n\n{}",
            g.path.display(),
            g.text
        ));
    }
    for (i, d) in project.iter().enumerate() {
        let file = format!("Contents of {}:\n\n{}", d.path.display(), d.text);
        if i == 0 && global.is_some() {
            body.push(format!("--- project-doc ---\n\n{}", file));
        } else {
            body.push(file);
        }
    }
    format!(
        "# AGENTS.md instructions for {}\n\n\
The user and this repository give you the instructions below (AGENTS.md files). \
An AGENTS.md applies to the whole folder tree that holds it. \
When they conflict, a file deeper in the tree wins over one above it, and the project's files win over the user's own. \
The direct instructions of your prompt and of the user win over any AGENTS.md. \
These files, from the repository root down to your working folder, are already here: do not read them again. \
When you work in a subfolder or outside your working folder, look for an AGENTS.md there and follow it too.\n\n\
<INSTRUCTIONS>\n{}\n</INSTRUCTIONS>",
        cwd.display(),
        body.join("\n\n")
    )
}

/// The section for an agent working in `cwd`, with bise's home `home`
/// (config.toml read from `config`).
pub fn section_for(cwd: &Path, home: &Path, config: &Path) -> String {
    let cfg = std::fs::read_to_string(config)
        .map(|t| Config::parse(&t))
        .unwrap_or_default();
    section(cwd, global(home).as_ref(), &project(cwd, &cfg))
}

/// Writes the section for `cwd` to `out` (an empty file when there is
/// none) and gives `out` back, for [`ENV`].
pub fn write_for(cwd: &Path, home: &bise_home::Home, out: &Path) -> PathBuf {
    let text = section_for(cwd, home.root(), &home.config_file());
    let _ = std::fs::write(out, text);
    out.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-agents-md-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    fn put(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn the_root_and_the_subfolder_both_come_root_first() {
        let d = tmp("chain");
        let repo = d.join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        put(&repo.join("AGENTS.md"), "root rules\n");
        put(&repo.join("a/b/AGENTS.md"), "sub rules");
        // above the root: never read
        put(&d.join("AGENTS.md"), "outside");
        let docs = project(&repo.join("a/b"), &Config::default());
        let texts: Vec<_> = docs.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, ["root rules", "sub rules"]);
        assert_eq!(docs[1].path, repo.join("a/b/AGENTS.md"));
        // from the root: the subfolder's file is not in the chain
        let texts: Vec<_> = project(&repo, &Config::default())
            .into_iter()
            .map(|x| x.text)
            .collect();
        assert_eq!(texts, ["root rules"]);
    }

    #[test]
    fn a_worktree_git_file_marks_the_root() {
        let d = tmp("wt");
        put(&d.join("AGENTS.md"), "outside");
        put(&d.join("wt/.git"), "gitdir: /elsewhere\n");
        put(&d.join("wt/AGENTS.md"), "worktree rules");
        let texts: Vec<_> = project(&d.join("wt"), &Config::default())
            .into_iter()
            .map(|x| x.text)
            .collect();
        assert_eq!(texts, ["worktree rules"]);
    }

    #[test]
    fn no_root_marker_reads_the_working_folder_alone() {
        let d = tmp("nomarker");
        put(&d.join("AGENTS.md"), "parent");
        put(&d.join("w/AGENTS.md"), "here");
        let texts: Vec<_> = project(&d.join("w"), &Config::default())
            .into_iter()
            .map(|x| x.text)
            .collect();
        assert_eq!(texts, ["here"]);
        assert_eq!(search_dirs(&d.join("w"), &[]), vec![d.join("w")]);
    }

    #[test]
    fn the_override_wins_in_its_folder_and_fallbacks_come_last() {
        let d = tmp("override");
        std::fs::create_dir_all(d.join(".git")).unwrap();
        put(&d.join("AGENTS.md"), "plain");
        put(&d.join("AGENTS.override.md"), "override");
        put(&d.join("s/CLAUDE.md"), "claude");
        let cfg = Config::parse("project_doc_fallback_filenames = [\"CLAUDE.md\", \"../x\"]\n");
        assert_eq!(
            cfg.candidates(),
            [OVERRIDE_FILENAME, DEFAULT_FILENAME, "CLAUDE.md"]
        );
        let texts: Vec<_> = project(&d.join("s"), &cfg)
            .into_iter()
            .map(|x| x.text)
            .collect();
        assert_eq!(texts, ["override", "claude"]);
        // without the fallback, CLAUDE.md is not read
        let texts: Vec<_> = project(&d.join("s"), &Config::default())
            .into_iter()
            .map(|x| x.text)
            .collect();
        assert_eq!(texts, ["override"]);
    }

    #[test]
    fn the_budget_cuts_the_file_that_crosses_it_and_drops_the_rest() {
        let d = tmp("budget");
        std::fs::create_dir_all(d.join(".git")).unwrap();
        put(&d.join("AGENTS.md"), "0123456789");
        put(&d.join("s/AGENTS.md"), "abcdefghij");
        put(&d.join("s/t/AGENTS.md"), "never");
        let cfg =
            Config::parse("project_doc_max_bytes = 15\n[models.x]\nproject_doc_max_bytes = 1\n");
        assert_eq!(cfg.max_bytes, 15);
        let texts: Vec<_> = project(&d.join("s/t"), &cfg)
            .into_iter()
            .map(|x| x.text)
            .collect();
        assert_eq!(texts, ["0123456789", "abcde"]);
        // 0: no project file
        let cfg = Config::parse("project_doc_max_bytes = 0");
        assert!(project(&d.join("s"), &cfg).is_empty());
        // a cut never splits a character
        assert_eq!(cut("aé", 2), "a");
    }

    #[test]
    fn the_global_file_override_first_blank_skipped() {
        let home = tmp("home");
        assert_eq!(global(&home), None);
        put(&home.join("AGENTS.md"), "  global  \n");
        assert_eq!(global(&home).unwrap().text, "global");
        put(&home.join("AGENTS.override.md"), "   \n");
        assert_eq!(global(&home).unwrap().text, "global");
        put(&home.join("AGENTS.override.md"), "local global");
        assert_eq!(global(&home).unwrap().text, "local global");
    }

    #[test]
    fn the_section_is_codex_block_with_each_file_named() {
        let cwd = Path::new("/w/repo/sub");
        assert_eq!(section(cwd, None, &[]), "");
        let g = Doc {
            path: "/h/AGENTS.md".into(),
            text: "G".into(),
        };
        let p = [
            Doc {
                path: "/w/repo/AGENTS.md".into(),
                text: "R".into(),
            },
            Doc {
                path: "/w/repo/sub/AGENTS.md".into(),
                text: "S".into(),
            },
        ];
        let s = section(cwd, Some(&g), &p);
        assert!(
            s.starts_with("# AGENTS.md instructions for /w/repo/sub\n\n"),
            "{s}"
        );
        assert!(s.ends_with(
            "<INSTRUCTIONS>\nContents of /h/AGENTS.md (the user's own, for every project):\n\nG\n\n\
--- project-doc ---\n\nContents of /w/repo/AGENTS.md:\n\nR\n\n\
Contents of /w/repo/sub/AGENTS.md:\n\nS\n</INSTRUCTIONS>"
        ), "{s}");
        // no global file: no project-doc marker
        let s = section(cwd, None, &p[..1]);
        assert!(
            s.contains("<INSTRUCTIONS>\nContents of /w/repo/AGENTS.md:\n\nR\n</INSTRUCTIONS>"),
            "{s}"
        );
    }

    #[test]
    fn write_for_leaves_an_empty_file_when_there_is_nothing() {
        let d = tmp("write");
        let home = bise_home::Home::at(d.join("home"));
        let out = write_for(&d, &home, &d.join("agents-md.md"));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "");
        put(&d.join("home/AGENTS.md"), "g");
        put(&d.join("home/config.toml"), "project_doc_max_bytes = 0\n");
        put(&d.join("AGENTS.md"), "p");
        let s = std::fs::read_to_string(write_for(&d, &home, &d.join("agents-md.md"))).unwrap();
        assert!(s.contains("\n\ng\n</INSTRUCTIONS>"), "{s}");
    }
}
