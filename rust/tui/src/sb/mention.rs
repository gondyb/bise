//! The `@` popup: the live agents a `@name` at the start of the
//! composer can name.

use super::*;

/// The `@` popup: the name being typed when the composer holds `@prefix`
/// and nothing else yet (routing only reads `@name` at line start).
fn mention_query(input: &str) -> Option<&str> {
    let rest = input.strip_prefix('@')?;
    if rest.contains(char::is_whitespace) {
        return None;
    }
    Some(rest)
}

/// The live agents a `@query` can name, the one in focus excluded:
/// case-insensitive prefix matches first, then substring matches, each
/// group in panel order.
pub(super) fn filter_mentions<'a>(agents: &'a [Agent], focus: &str, query: &str) -> Vec<&'a Agent> {
    let q = query.to_lowercase();
    let live = agents
        .iter()
        .filter(|a| !a.archived() && a.name != focus && !a.name.is_empty());
    let (mut prefix, mut inner) = (Vec::new(), Vec::new());
    for a in live {
        let n = a.name.to_lowercase();
        if n.starts_with(&q) {
            prefix.push(a);
        } else if n.contains(&q) {
            inner.push(a);
        }
    }
    prefix.extend(inner);
    prefix
}

/// One entry of the `@` popup.
pub(crate) struct Mention {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) objective: String,
}

impl Mention {
    /// What the composer holds once the entry is picked.
    pub(crate) fn completion(&self) -> String {
        format!("@{} ", self.name)
    }

    pub(crate) fn glyph(&self, tick: u32) -> (&'static str, Color) {
        glyph(&self.status, tick)
    }
}

/// The `@` popup entries for the composer's current text (empty: closed).
pub(crate) fn mentions(app: &App) -> Vec<Mention> {
    let Some(sb) = app.sb.as_ref() else {
        return Vec::new();
    };
    // a recalled history line is not a completion request
    if app.ed.browsing() || app.popup_dismissed.as_deref() == Some(app.ed.text.as_str()) {
        return Vec::new();
    }
    let Some(q) = mention_query(&app.ed.text) else {
        return Vec::new();
    };
    filter_mentions(&sb.agents, &sb.focus, q)
        .into_iter()
        .map(|a| Mention {
            name: a.name.clone(),
            status: a.status.clone(),
            objective: a.objective.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(name: &str, status: &str) -> Agent {
        Agent {
            name: name.into(),
            status: status.into(),
            ..Agent::default()
        }
    }

    fn names(v: Vec<&Agent>) -> Vec<&str> {
        v.into_iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn query_only_while_the_name_is_typed() {
        assert_eq!(mention_query("@"), Some(""));
        assert_eq!(mention_query("@be"), Some("be"));
        assert_eq!(mention_query("@bend-hub salut"), None);
        assert_eq!(mention_query("salut @be"), None);
        assert_eq!(mention_query("/new"), None);
        assert_eq!(mention_query(""), None);
    }

    #[test]
    fn filters_live_agents_prefix_first() {
        let agents = vec![
            agent("main", "idle"),
            agent("bend-hub", "working"),
            agent("parent-history", "working"),
            agent("at-complete", "working"),
            agent("old-bend", "archived"),
        ];
        // everything live but the focus
        assert_eq!(
            names(filter_mentions(&agents, "main", "")),
            ["bend-hub", "parent-history", "at-complete"]
        );
        // from a task, main is a candidate
        assert_eq!(
            names(filter_mentions(&agents, "bend-hub", "")),
            ["main", "parent-history", "at-complete"]
        );
        // prefix before substring, archived out, case-insensitive
        assert_eq!(names(filter_mentions(&agents, "main", "B")), ["bend-hub"]);
        assert_eq!(
            names(filter_mentions(&agents, "main", "a")),
            ["at-complete", "parent-history"]
        );
        assert_eq!(names(filter_mentions(&agents, "main", "his")), ["parent-history"]);
        assert!(filter_mentions(&agents, "main", "zzz").is_empty());
    }

    #[test]
    fn completion_inserts_the_name_and_a_space() {
        let m = Mention {
            name: "bend-hub".into(),
            status: "working".into(),
            objective: String::new(),
        };
        assert_eq!(m.completion(), "@bend-hub ");
        assert_eq!(mention_query(&m.completion()), None); // popup closes
    }
}
