//! Releases (BISE-170..172): where an installed bise lives, where its
//! updates come from (the dist URL) and the manifest it reads
//! (`latest.json`). Shared by `bise update` (the harness) and the hub
//! (`/version`, `/restart latest`, the "update ready" notice).
//!
//! An install (install.sh):
//!   <prefix>/versions/<id>/   immutable app roots (VERSION has no `repo=`)
//!   <prefix>/current -> versions/<id>
//!   <prefix>/bin/bise         the launcher: exec current/bise
//!   <prefix>/install.sh       the installer (uninstall, reinstall)
//!   <prefix>/dist-url         the release channel it was installed from
//!
//! The release host is not decided yet: [`DIST_URL`] is the one place
//! to set it; `BISE_DIST_URL` (env) and `<prefix>/dist-url` win over it,
//! and a `file://` URL works the same (the tests).

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The default release channel: a base URL that serves `install.sh`,
/// `latest.json` and the tarballs. Empty until the host is decided:
/// then only an install made with `BISE_DIST_URL` has a channel.
pub const DIST_URL: &str = "";
/// Overrides the channel (and the one an install recorded).
pub const DIST_URL_ENV: &str = "BISE_DIST_URL";
/// `1`: no background update check.
pub const NO_UPDATE_ENV: &str = "BISE_NO_UPDATE";
/// The manifest's name at the channel's base URL.
pub const MANIFEST: &str = "latest.json";

/// The `key=value` lines of a VERSION file.
pub fn version_fields(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// The VERSION fields of an app root (empty when it has none).
pub fn read_version(root: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(root.join("VERSION")).map(|t| version_fields(&t)).unwrap_or_default()
}

/// One installed version.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    pub dir: PathBuf,
    pub subject: String,
    pub built: String,
}

/// An installed bise: the prefix install.sh wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Install {
    pub prefix: PathBuf,
}

impl Install {
    /// The install an app root belongs to: a `versions/<id>` dir of a
    /// prefix that has `current`, whose VERSION names no dev repo (a
    /// `versions.sh` build has `repo=`: dev mode, BISE-172).
    pub fn of_root(root: &Path) -> Option<Install> {
        let root = root.canonicalize().ok()?;
        let v = read_version(&root);
        if v.is_empty() || v.contains_key("repo") {
            return None;
        }
        let versions = root.parent()?;
        if versions.file_name()? != "versions" {
            return None;
        }
        let prefix = versions.parent()?;
        prefix.join("current").symlink_metadata().ok()?;
        Some(Install { prefix: prefix.to_path_buf() })
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.prefix.join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.prefix.join("current")
    }

    /// The version `current` points at, resolved.
    pub fn current(&self) -> Option<PathBuf> {
        self.current_link().canonicalize().ok()
    }

    /// The installer kept in the prefix (uninstall).
    pub fn installer(&self) -> PathBuf {
        self.prefix.join("install.sh")
    }

    /// Where install.sh recorded the channel it installed from.
    pub fn dist_url_file(&self) -> PathBuf {
        self.prefix.join("dist-url")
    }

    /// The release channel: `BISE_DIST_URL`, else the one recorded at
    /// install, else [`DIST_URL`]; None when none is set.
    pub fn dist_url(&self, env: crate::Lookup) -> Option<String> {
        let recorded = std::fs::read_to_string(self.dist_url_file()).ok();
        [env(DIST_URL_ENV), recorded, Some(DIST_URL.to_string())]
            .into_iter()
            .flatten()
            .map(|u| u.trim().trim_end_matches('/').to_string())
            .find(|u| !u.is_empty())
    }

    /// The installed versions, newest build first.
    pub fn installed(&self) -> Vec<Installed> {
        let mut out: Vec<Installed> = std::fs::read_dir(self.versions_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| {
                let dir = e.path();
                let v = read_version(&dir);
                let id = v.get("id").cloned()?;
                Some(Installed {
                    id,
                    subject: v.get("subject").cloned().unwrap_or_default(),
                    built: v.get("built").cloned().unwrap_or_default(),
                    dir: dir.canonicalize().unwrap_or(dir),
                })
            })
            .collect();
        out.sort_by(|a, b| b.built.cmp(&a.built).then(a.id.cmp(&b.id)));
        out
    }

    /// The installed version of this id.
    pub fn find(&self, id: &str) -> Option<Installed> {
        self.installed().into_iter().find(|i| i.id == id)
    }
}

/// A release, as `latest.json` gives it for one target.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// the release's name (`0.1.0`), else its id
    pub version: String,
    pub id: String,
    /// the tarball: absolute, or relative to the channel's base URL
    pub url: String,
    pub sha256: String,
    /// when it was built (VERSION `built=`), when the manifest says
    pub built: Option<String>,
}

/// `latest.json` (the CI workflow's and `make-release.sh`'s format):
/// `{version, id, targets: {<os-arch>: {url, sha256, id, built?}}}`.
pub fn parse_manifest(text: &str, target: &str) -> Result<Release, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("{} is not valid JSON: {}", MANIFEST, e))?;
    let t = v
        .pointer(&format!("/targets/{}", target))
        .ok_or_else(|| format!("the release has no build for {}", target))?;
    let s = |x: &Value, k: &str| x.get(k).and_then(|y| y.as_str()).map(str::trim).filter(|y| !y.is_empty()).map(str::to_string);
    let id = s(t, "id").or_else(|| s(&v, "id")).ok_or("the release has no id")?;
    let url = s(t, "url").or_else(|| s(t, "file")).ok_or("the release has no tarball url")?;
    let sha256 = s(t, "sha256").ok_or("the release has no sha256")?;
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("bad sha256 in the release: {}", sha256));
    }
    Ok(Release {
        version: s(&v, "version").unwrap_or_else(|| id.clone()),
        id,
        url,
        sha256: sha256.to_ascii_lowercase(),
        built: s(t, "built").or_else(|| s(&v, "built")),
    })
}

/// A tarball URL: absolute as is, else relative to the channel.
pub fn resolve_url(base: &str, url: &str) -> String {
    if url.contains("://") {
        url.to_string()
    } else {
        format!("{}/{}", base.trim_end_matches('/'), url.trim_start_matches('/'))
    }
}

/// Whether `release` replaces the version `current` holds: another id,
/// and not an older build (a local build of a newer commit stays).
pub fn is_update(release: &Release, current_id: &str, current_built: Option<&str>) -> bool {
    if release.id == current_id {
        return false;
    }
    match (release.built.as_deref(), current_built) {
        (Some(r), Some(c)) if !r.is_empty() && !c.is_empty() => r > c,
        _ => true,
    }
}

impl crate::Home {
    /// The last `latest.json` read by `bise update` (the hub reads it).
    pub fn release_manifest(&self) -> PathBuf {
        self.cache_dir().join(MANIFEST)
    }

    /// Touched at each update check: the daily check runs when it is old.
    pub fn update_stamp(&self) -> PathBuf {
        self.cache_dir().join("update-check")
    }

    /// The background check's log.
    pub fn update_log(&self) -> PathBuf {
        self.cache_dir().join("update.log")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest(target: &str, url: &str) -> String {
        format!(
            r#"{{"version":"0.2.0","id":"top","targets":{{"{}":{{"url":"{}","sha256":"{}","id":"abc1234","built":"2026-10-02T10:00:00Z"}}}}}}"#,
            target, url, SHA
        )
    }

    #[test]
    fn the_manifest_gives_this_targets_release() {
        let r = parse_manifest(&manifest("darwin-arm64", "bise-abc1234-darwin-arm64.tar.gz"), "darwin-arm64").unwrap();
        assert_eq!(r.id, "abc1234");
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.sha256, SHA);
        assert_eq!(r.built.as_deref(), Some("2026-10-02T10:00:00Z"));
        let e = parse_manifest(&manifest("darwin-arm64", "x"), "darwin-x86_64").unwrap_err();
        assert!(e.contains("no build for darwin-x86_64"), "{e}");
        assert!(parse_manifest("{", "darwin-arm64").unwrap_err().contains("not valid JSON"));
        let bad = manifest("darwin-arm64", "x").replace(SHA, "zz");
        assert!(parse_manifest(&bad, "darwin-arm64").unwrap_err().contains("bad sha256"));
    }

    #[test]
    fn urls_are_relative_to_the_channel_unless_absolute() {
        assert_eq!(resolve_url("file:///tmp/rel/", "a.tar.gz"), "file:///tmp/rel/a.tar.gz");
        assert_eq!(resolve_url("https://x.dev", "https://gh/a.tar.gz"), "https://gh/a.tar.gz");
    }

    #[test]
    fn an_update_is_another_id_and_not_an_older_build() {
        let r = Release {
            version: "v".into(),
            id: "new".into(),
            url: "u".into(),
            sha256: SHA.into(),
            built: Some("2026-10-02T10:00:00Z".into()),
        };
        assert!(!is_update(&r, "new", None));
        assert!(is_update(&r, "old", Some("2026-10-01T10:00:00Z")));
        assert!(!is_update(&r, "local", Some("2026-10-03T10:00:00Z")));
        assert!(is_update(&r, "old", None));
        assert!(is_update(&Release { built: None, ..r }, "old", Some("2026-10-03T10:00:00Z")));
    }

    #[test]
    fn an_install_is_a_version_dir_of_a_prefix_without_repo() {
        let t = std::env::temp_dir().join(format!("bise-release-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let v = t.join("versions");
        for (id, built) in [("a1", "2026-10-01"), ("b2", "2026-10-02")] {
            std::fs::create_dir_all(v.join(id)).unwrap();
            std::fs::write(v.join(id).join("VERSION"), format!("id={}\nbuilt={}\nsubject=s {}\n", id, built, id)).unwrap();
        }
        std::fs::create_dir_all(v.join(".tmp")).unwrap();
        // no `current` yet: not an install
        assert_eq!(Install::of_root(&v.join("a1")), None);
        std::os::unix::fs::symlink("versions/a1", t.join("current")).unwrap();
        let i = Install::of_root(&v.join("b2")).unwrap();
        assert_eq!(i.prefix, t.canonicalize().unwrap());
        assert_eq!(i.current(), Some(v.join("a1").canonicalize().unwrap()));
        let ids: Vec<String> = i.installed().into_iter().map(|x| x.id).collect();
        assert_eq!(ids, ["b2", "a1"]);
        assert_eq!(i.find("a1").unwrap().subject, "s a1");
        // a dev version (repo=) is never an install
        std::fs::write(v.join("a1").join("VERSION"), "id=a1\nrepo=/src\n").unwrap();
        assert_eq!(Install::of_root(&v.join("a1")), None);
        // the channel: env, else recorded, else the const (empty today)
        let none = |_: &str| None;
        assert_eq!(i.dist_url(&none), (!DIST_URL.is_empty()).then(|| DIST_URL.to_string()));
        std::fs::write(i.dist_url_file(), "file:///rel/\n").unwrap();
        assert_eq!(i.dist_url(&none).as_deref(), Some("file:///rel"));
        let env = |k: &str| (k == DIST_URL_ENV).then(|| "https://x".to_string());
        assert_eq!(i.dist_url(&env).as_deref(), Some("https://x"));
        let _ = std::fs::remove_dir_all(&t);
    }
}
