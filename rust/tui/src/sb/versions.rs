//! The `/version` picker: the versions the hub offers (its `versions`
//! event), filtered by what follows `/version ` in the composer.

use super::*;

/// One entry of the `/version` picker.
#[derive(Clone, Debug, Default)]
pub(crate) struct VersionItem {
    pub(super) rev: String,
    subject: String,
    pub(super) marks: Vec<String>,
}

pub(super) fn parse_versions(v: &Value) -> Vec<VersionItem> {
    let s = str_of;
    v.get("items")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| VersionItem {
                    rev: s(x, "rev"),
                    subject: s(x, "subject"),
                    marks: x
                        .get("marks")
                        .and_then(|m| m.as_array())
                        .map(|m| m.iter().filter_map(|y| y.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The text after `/version ` when the composer holds a version query.
fn version_query(input: &str) -> Option<&str> {
    input.strip_prefix("/version ").filter(|q| !q.contains('\n'))
}

/// The items matching `q` (in the revision or the subject).
fn filter_versions<'a>(items: &'a [VersionItem], q: &str) -> Vec<&'a VersionItem> {
    let q = q.trim().to_lowercase();
    items
        .iter()
        .filter(|i| {
            q.is_empty() || i.rev.to_lowercase().contains(&q) || i.subject.to_lowercase().contains(&q)
        })
        .collect()
}

/// The marks as words, and the glyph of the most telling one.
fn version_marks(marks: &[String]) -> (String, (&'static str, Color)) {
    let has = |m: &str| marks.iter().any(|x| x == m);
    let glyph = if has("building") {
        ("…", theme::accent())
    } else if has("failed") && !has("current") {
        ("✗", theme::error())
    } else if has("current") {
        ("◉", theme::accent())
    } else if has("good") {
        ("✓", theme::dim())
    } else if has("built") {
        ("●", theme::text())
    } else {
        ("○", theme::dim())
    };
    let words: Vec<&str> = marks
        .iter()
        .map(|m| match m.as_str() {
            "current" => "current",
            "trial" => "on trial",
            "good" => "last good",
            "built" => "built",
            "building" => "building…",
            "failed" => "failed",
            other => other,
        })
        .collect();
    (words.join(", "), glyph)
}

/// `/version <query>`: the picker of versions (commits, tree, back).
/// Enter builds if needed, then switches; Tab fills the composer.
pub(crate) fn version_items(app: &App) -> Vec<PopItem> {
    let Some(sb) = app.sb.as_ref() else {
        return Vec::new();
    };
    if !popup_open(app) {
        return Vec::new();
    }
    let Some(q) = version_query(&app.ed.text) else {
        return Vec::new();
    };
    // ask the hub for a fresh list (at most every 3 s while it is open)
    let stale = sb
        .versions_asked
        .get()
        .is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(3));
    if stale {
        sb.versions_asked.set(Some(std::time::Instant::now()));
        if let Ok(mut w) = sb.writer.lock() {
            let _ = w.write_all(b"{\"op\":\"version\",\"do\":\"items\"}\n");
        }
    }
    if sb.versions.is_empty() {
        return vec![PopItem {
            label: "…".into(),
            desc: "loading the versions".into(),
            mark: None,
            fill: app.ed.text.clone(),
            fill_cursor: app.ed.cursor,
            run: None,
            closable: true,
            path: None,
            folder: false,
        }];
    }
    filter_versions(&sb.versions, q)
        .into_iter()
        .map(|i| {
            let (words, glyph) = version_marks(&i.marks);
            let line = format!("/version {}", i.rev);
            PopItem {
                label: i.rev.clone(),
                desc: if words.is_empty() {
                    i.subject.clone()
                } else {
                    format!("[{}] {}", words, i.subject)
                },
                mark: Some(glyph),
                fill_cursor: line.chars().count(),
                fill: line.clone(),
                run: Some(line),
                closable: true,
                path: None,
                folder: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(rev: &str, subject: &str, marks: &[&str]) -> VersionItem {
        VersionItem {
            rev: rev.into(),
            subject: subject.into(),
            marks: marks.iter().map(|m| m.to_string()).collect(),
        }
    }

    #[test]
    fn the_query_is_the_text_after_version() {
        assert_eq!(version_query("/version "), Some(""));
        assert_eq!(version_query("/version ab1"), Some("ab1"));
        assert_eq!(version_query("/version"), None);
        assert_eq!(version_query("/versions x"), None);
    }

    #[test]
    fn the_filter_matches_the_revision_or_the_subject() {
        let items = vec![
            item("back", "roll back to 1234567", &[]),
            item("tree", "the working tree", &["current"]),
            item("abc1234", "tui: faster feed", &["built"]),
            item("def5678", "hub: journal", &["good", "built"]),
        ];
        let revs = |q: &str| -> Vec<String> {
            filter_versions(&items, q).iter().map(|i| i.rev.clone()).collect()
        };
        assert_eq!(revs("").len(), 4);
        assert_eq!(revs("abc"), vec!["abc1234"]);
        assert_eq!(revs("JOURNAL"), vec!["def5678"]);
        assert_eq!(revs("back"), vec!["back"]);
    }

    #[test]
    fn marks_read_as_words_and_one_glyph() {
        let (w, g) = version_marks(&["current".into(), "trial".into()]);
        assert_eq!(w, "current, on trial");
        assert_eq!(g.0, "◉");
        assert_eq!(version_marks(&["building".into()]).1 .0, "…");
        assert_eq!(version_marks(&["failed".into()]).1 .0, "✗");
        assert_eq!(version_marks(&[]).1 .0, "○");
    }

    #[test]
    fn the_hub_event_parses() {
        let v = json!({"ev": "versions", "items": [
            {"rev": "abc1234", "subject": "s", "marks": ["built", "good"]}]});
        let items = parse_versions(&v);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].marks, vec!["built", "good"]);
    }
}
