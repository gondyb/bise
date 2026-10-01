//! GitHub through `gh api graphql` (pr-design §7, §8): one query per repo
//! per tick, an alias per branch (`b0`, `b1`…), gh's own login. bise
//! never reads, stores or prints a token: gh does.

use super::{Forge, ForgeError, RepoRef};
use crate::place::{Checks, PrSnapshot, PrState, Review};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// What one alias asks: the 3 latest PRs of the branch (a fork's PR on a
/// branch of the same name is skipped by its owner), the last commit's
/// checks with the names of the failing ones.
const FIELDS: &str = "number url state isDraft reviewDecision headRefName headRefOid updatedAt \
headRepositoryOwner { login } \
commits(last: 1) { nodes { commit { statusCheckRollup { state \
contexts(first: 50) { nodes { __typename \
... on CheckRun { name status conclusion } \
... on StatusContext { context state } } } } } } }";

/// A GraphQL string literal.
fn lit(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// The query for `branches` of `repo` (measured: 25 aliases cost 1 point).
pub fn query(repo: &RepoRef, branches: &[String]) -> String {
    let aliases: Vec<String> = branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            format!(
                "b{}: pullRequests(headRefName: {}, first: 3, orderBy: {{field: UPDATED_AT, direction: DESC}}) {{ nodes {{ {} }} }}",
                i,
                lit(b),
                FIELDS
            )
        })
        .collect();
    format!(
        "query {{ repository(owner: {}, name: {}) {{ {} }} }}",
        lit(&repo.owner),
        lit(&repo.name),
        aliases.join(" ")
    )
}

fn checks_of(rollup: &Value) -> Checks {
    if rollup.is_null() {
        return Checks::None;
    }
    let mut failing: Vec<String> = Vec::new();
    for c in rollup["contexts"]["nodes"].as_array().into_iter().flatten() {
        let (name, bad) = match c["__typename"].as_str() {
            Some("CheckRun") => (
                c["name"].as_str(),
                matches!(
                    c["conclusion"].as_str(),
                    Some("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE")
                ),
            ),
            Some("StatusContext") => (c["context"].as_str(), matches!(c["state"].as_str(), Some("FAILURE" | "ERROR"))),
            _ => (None, false),
        };
        if let (Some(n), true) = (name, bad) {
            if !failing.iter().any(|f| f == n) {
                failing.push(n.to_string());
            }
        }
    }
    match rollup["state"].as_str() {
        _ if !failing.is_empty() => Checks::Fail { failing },
        Some("FAILURE" | "ERROR") => Checks::Fail { failing },
        Some("SUCCESS") => Checks::Pass,
        Some("PENDING" | "EXPECTED") => Checks::Running,
        _ => Checks::None,
    }
}

fn snapshot(n: &Value, branch: &str) -> Option<PrSnapshot> {
    let state = match (n["state"].as_str()?, n["isDraft"].as_bool() == Some(true)) {
        ("OPEN", true) => PrState::Draft,
        ("OPEN", false) => PrState::Open,
        ("MERGED", _) => PrState::Merged,
        ("CLOSED", _) => PrState::Closed,
        _ => return None,
    };
    let review = match n["reviewDecision"].as_str() {
        Some("APPROVED") => Review::Approved,
        Some("CHANGES_REQUESTED") => Review::ChangesRequested,
        Some("REVIEW_REQUIRED") => Review::Pending,
        _ => Review::None,
    };
    let rollup = &n["commits"]["nodes"][0]["commit"]["statusCheckRollup"];
    Some(PrSnapshot {
        number: n["number"].as_u64()?,
        url: n["url"].as_str()?.to_string(),
        branch: branch.to_string(),
        head_oid: n["headRefOid"].as_str().unwrap_or_default().to_string(),
        state,
        review,
        checks: checks_of(rollup),
        updated_at: n["updatedAt"].as_str().unwrap_or_default().to_string(),
    })
}

/// The PRs in gh's answer to [`query`]: per alias, the latest PR from the
/// repo's owner (not a fork's).
pub fn parse(out: &str, repo: &RepoRef, branches: &[String]) -> Result<Vec<PrSnapshot>, ForgeError> {
    let v: Value = serde_json::from_str(out).map_err(|e| ForgeError::Other(format!("gh answered no JSON: {}", e)))?;
    if let Some(errs) = v["errors"].as_array().filter(|e| !e.is_empty()) {
        let e = &errs[0];
        let msg = e["message"].as_str().unwrap_or("error").to_string();
        return Err(match e["type"].as_str() {
            Some("RATE_LIMITED") => ForgeError::RateLimited(msg),
            _ => ForgeError::Other(msg),
        });
    }
    let r = &v["data"]["repository"];
    if r.is_null() {
        return Err(ForgeError::Other(format!("no repository {}/{} on {}", repo.owner, repo.name, repo.host)));
    }
    let mut prs = Vec::new();
    for (i, b) in branches.iter().enumerate() {
        let ours = r[format!("b{}", i)]["nodes"].as_array().into_iter().flatten().find(|n| {
            n["headRepositoryOwner"]["login"].as_str().is_none_or(|o| o.eq_ignore_ascii_case(&repo.owner))
        });
        if let Some(pr) = ours.and_then(|n| snapshot(n, b)) {
            prs.push(pr);
        }
    }
    Ok(prs)
}

/// What gh's failure says (its stderr, never printed whole: a short
/// reason for hub.log).
pub fn error_of(stderr: &str) -> ForgeError {
    let s = stderr.to_lowercase();
    let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("gh failed").trim();
    let short: String = first.chars().take(120).collect();
    if s.contains("http 401") || s.contains("bad credentials") || s.contains("gh auth login") || s.contains("not logged") {
        ForgeError::Auth(short)
    } else if s.contains("rate limit") {
        ForgeError::RateLimited(short)
    } else if s.contains("could not resolve")
        || s.contains("no such host")
        || s.contains("connection refused")
        || s.contains("network is unreachable")
        || s.contains("timeout")
        || s.contains("error connecting")
        || s.contains("http 502")
        || s.contains("http 503")
    {
        ForgeError::Offline(short)
    } else {
        ForgeError::Other(short)
    }
}

/// GitHub through the `gh` at `gh` (the hub's PATH lookup; a fake one in
/// the tests).
pub struct GitHub {
    pub gh: PathBuf,
}

impl GitHub {
    /// The `gh` on `path` (a PATH-like list), if any.
    pub fn find(path: &str) -> Option<GitHub> {
        crate::tools_env::which("gh", path).map(|gh| GitHub { gh })
    }

    fn gh(&self, args: &[&str]) -> Result<String, ForgeError> {
        let out = Command::new(&self.gh)
            .args(args)
            // gh asks nothing, pages nothing, prints no color
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("NO_COLOR", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => ForgeError::Missing,
                _ => ForgeError::Other(format!("gh: {}", e)),
            })?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            // a GraphQL error comes with a JSON body on stdout
            let body = String::from_utf8_lossy(&out.stdout);
            if body.trim_start().starts_with('{') && body.contains("\"errors\"") {
                return Ok(body.into_owned());
            }
            Err(error_of(&String::from_utf8_lossy(&out.stderr)))
        }
    }

    /// Whether gh has a login for `host` (`gh auth status`'s exit code;
    /// its output, which names the token's scopes, is never read).
    pub fn logged_in(&self, host: &str) -> Result<bool, ForgeError> {
        let st = Command::new(&self.gh)
            .args(["auth", "status", "--hostname", host])
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => ForgeError::Missing,
                _ => ForgeError::Other(format!("gh: {}", e)),
            })?;
        Ok(st.success())
    }
}

impl Forge for GitHub {
    fn fetch(&self, repo: &RepoRef, branches: &[String]) -> Result<Vec<PrSnapshot>, ForgeError> {
        if branches.is_empty() {
            return Ok(Vec::new());
        }
        let q = format!("query={}", query(repo, branches));
        let mut args = vec!["api", "graphql", "-f", q.as_str()];
        if repo.host != "github.com" {
            args.extend(["--hostname", repo.host.as_str()]);
        }
        let out = self.gh(&args)?;
        parse(&out, repo, branches)
    }
}

/// The forge of a workspace: its `origin`'s repo, when it is on GitHub
/// (`github.com`, or a host gh has a login for: GitHub Enterprise).
/// None: not a GitHub repo (no forge; GitLab later).
pub fn detect(remote_url: &str, gh: Option<&GitHub>) -> Option<RepoRef> {
    let repo = super::repo_of_url(remote_url)?;
    if repo.host == "github.com" || gh.is_some_and(|g| g.logged_in(&repo.host) == Ok(true)) {
        Some(repo)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> RepoRef {
        RepoRef { host: "github.com".into(), owner: "o".into(), name: "r".into() }
    }

    #[test]
    fn the_query_has_an_alias_per_branch() {
        let q = query(&repo(), &["sb/a".into(), "sb/\"b".into()]);
        assert!(q.starts_with("query { repository(owner: \"o\", name: \"r\") {"));
        assert!(q.contains("b0: pullRequests(headRefName: \"sb/a\", first: 3"));
        assert!(q.contains("b1: pullRequests(headRefName: \"sb/\\\"b\""));
        assert!(!q.contains("b2:"));
    }

    #[test]
    fn an_answer_reads_into_snapshots() {
        let out = serde_json::json!({"data": {"repository": {
            "b0": {"nodes": [
                {"number": 9, "url": "u9", "state": "OPEN", "isDraft": false, "reviewDecision": null,
                 "headRefOid": "f", "updatedAt": "t", "headRepositoryOwner": {"login": "fork"}, "commits": {"nodes": []}},
                {"number": 7, "url": "u7", "state": "OPEN", "isDraft": true, "reviewDecision": "CHANGES_REQUESTED",
                 "headRefOid": "h", "updatedAt": "t7", "headRepositoryOwner": {"login": "O"},
                 "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "FAILURE", "contexts": {"nodes": [
                    {"__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "FAILURE"},
                    {"__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "SUCCESS"},
                    {"__typename": "StatusContext", "context": "ci/legacy", "state": "ERROR"}]}}}}]}}]},
            "b1": {"nodes": []},
            "b2": {"nodes": [{"number": 3, "url": "u3", "state": "MERGED", "isDraft": false, "reviewDecision": "APPROVED",
                 "headRefOid": "m", "updatedAt": "t3", "headRepositoryOwner": {"login": "o"},
                 "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS", "contexts": {"nodes": []}}}}]}}]}
        }}});
        let bs = ["sb/a".to_string(), "sb/b".into(), "sb/c".into()];
        let prs = parse(&out.to_string(), &repo(), &bs).unwrap();
        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].number, 7);
        assert_eq!(prs[0].branch, "sb/a");
        assert_eq!(prs[0].state, PrState::Draft);
        assert_eq!(prs[0].review, Review::ChangesRequested);
        assert_eq!(prs[0].checks, Checks::Fail { failing: vec!["test".into(), "ci/legacy".into()] });
        assert_eq!(prs[1].branch, "sb/c");
        assert_eq!(prs[1].state, PrState::Merged);
        assert_eq!(prs[1].checks, Checks::Pass);
    }

    #[test]
    fn checks_running_and_none() {
        assert_eq!(checks_of(&Value::Null), Checks::None);
        let r = serde_json::json!({"state": "PENDING", "contexts": {"nodes": [
            {"__typename": "CheckRun", "name": "t", "status": "IN_PROGRESS", "conclusion": null}]}});
        assert_eq!(checks_of(&r), Checks::Running);
        // one failed while others run: red already
        let r = serde_json::json!({"state": "PENDING", "contexts": {"nodes": [
            {"__typename": "CheckRun", "name": "t", "status": "COMPLETED", "conclusion": "TIMED_OUT"}]}});
        assert_eq!(checks_of(&r), Checks::Fail { failing: vec!["t".into()] });
    }

    #[test]
    fn errors() {
        let e = parse(r#"{"errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded"}]}"#, &repo(), &[]);
        assert!(matches!(e, Err(ForgeError::RateLimited(_))));
        assert!(matches!(error_of("HTTP 401: Bad credentials (https://api.github.com/graphql)"), ForgeError::Auth(_)));
        assert!(matches!(error_of("To get started with GitHub CLI, please run:  gh auth login"), ForgeError::Auth(_)));
        assert!(matches!(error_of("error connecting to api.github.com"), ForgeError::Offline(_)));
        assert!(matches!(error_of("HTTP 403: API rate limit exceeded for user"), ForgeError::RateLimited(_)));
        assert!(matches!(error_of("something else"), ForgeError::Other(_)));
        assert!(matches!(parse(r#"{"data": {"repository": null}}"#, &repo(), &[]), Err(ForgeError::Other(_))));
    }

    #[test]
    fn detection() {
        assert_eq!(detect("https://github.com/o/r.git", None), Some(repo()));
        // another host: only when gh knows it
        assert_eq!(detect("https://gitlab.com/o/r.git", None), None);
        assert_eq!(detect("not a url", None), None);
    }
}
