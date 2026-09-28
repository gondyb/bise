//! What one line typed by the user asks for (RFC 0001 §7.1, RFC 0002 §3,
//! §5). Pure parsing: the core decides what happens.

use crate::model::MAIN;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserCmd {
    /// Plain text for the agent in focus.
    Say(String),
    /// `@name text`: an explicit route, no main turn (RFC 0001 §7.1).
    To {
        target: String,
        text: String,
    },
    /// `/new [-w] [--with-changes] [name:] brief`.
    New {
        name: Option<String>,
        brief: String,
        worktree: bool,
        with_changes: bool,
    },
    /// `/drop [name] [--force]` (no name: the task in focus).
    Drop {
        name: Option<String>,
        force: bool,
    },
    Restore {
        name: String,
    },
    Isolate {
        name: String,
    },
    Rename {
        name: String,
        new_name: String,
    },
    /// `/answer N text`: the answer to attention card N.
    Answer {
        card: u64,
        text: String,
    },
    /// `/close N`: close attention card N without answering it.
    Close {
        card: u64,
    },
    /// `/cancel`: undo the last route if it is not delivered yet.
    Cancel,
    /// `/tasks`: the board, printed locally.
    Tasks,
    /// `/interrupt`: the agent in focus stops its turn.
    Interrupt,
    /// Any other `/command`: for the REPL of the agent in focus
    /// (`/compact`, `/status`...).
    Passthrough(String),
    Help,
    Invalid(String),
}

/// A task name (RFC 0001 §9.3): `[a-z0-9-]{1,24}`.
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 24
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
}

/// A task name from free text: the first words, lowercased and dashed.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        let w: String = word
            .chars()
            .map(fold_accent)
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if w.is_empty() || w.len() < 2 && out.is_empty() {
            continue;
        }
        let next = if out.is_empty() {
            w
        } else {
            format!("{}-{}", out, w)
        };
        if next.len() > 24 {
            break;
        }
        out = next;
        if out.matches('-').count() >= 2 {
            break;
        }
    }
    if out.is_empty() {
        "task".to_string()
    } else {
        out
    }
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ç' => 'c',
        'À' | 'Â' => 'a',
        'É' | 'È' | 'Ê' => 'e',
        _ => c,
    }
}

/// `name: rest` when `name` is a valid task name.
fn split_named(s: &str) -> (Option<String>, String) {
    if let Some((head, rest)) = s.split_once(':') {
        let head = head.trim();
        if valid_name(head) && !head.contains(' ') {
            return (Some(head.to_string()), rest.trim().to_string());
        }
    }
    (None, s.trim().to_string())
}

/// Parse one line typed while `focus` has the focus.
pub fn parse(line: &str, focus: &str) -> UserCmd {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix('@') {
        let (target, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let text = text.trim();
        if text.is_empty() {
            return UserCmd::Invalid(format!("empty message for @{}", target));
        }
        return UserCmd::To {
            target: target.to_string(),
            text: text.to_string(),
        };
    }
    if !line.starts_with('/') {
        return UserCmd::Say(line.to_string());
    }
    let (cmd, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    let words: Vec<&str> = rest.split_whitespace().collect();
    match cmd {
        "/new" => {
            let mut worktree = false;
            let mut with_changes = false;
            let mut body = rest;
            loop {
                let b = body.trim_start();
                if let Some(r) = b
                    .strip_prefix("-w ")
                    .or_else(|| b.strip_prefix("--worktree "))
                {
                    worktree = true;
                    body = r;
                } else if let Some(r) = b.strip_prefix("--with-changes ") {
                    with_changes = true;
                    body = r;
                } else if let Some(r) = b.strip_prefix("-s ") {
                    body = r;
                } else {
                    body = b;
                    break;
                }
            }
            let (name, brief) = split_named(body);
            if brief.is_empty() {
                return UserCmd::Invalid("usage: /new [-w] [name:] objective".into());
            }
            if with_changes && !worktree {
                return UserCmd::Invalid("--with-changes only works with -w".into());
            }
            UserCmd::New {
                name,
                brief,
                worktree,
                with_changes,
            }
        }
        "/drop" => {
            let force = words.contains(&"--force");
            let name = words
                .iter()
                .find(|w| !w.starts_with("--"))
                .map(|w| w.trim_start_matches('@').to_string());
            let name = name.or_else(|| (focus != MAIN).then(|| focus.to_string()));
            UserCmd::Drop { name, force }
        }
        "/restore" | "/isolate" => {
            let name = words
                .first()
                .map(|w| w.trim_start_matches('@').to_string())
                .or_else(|| (focus != MAIN).then(|| focus.to_string()));
            match name {
                Some(name) if cmd == "/restore" => UserCmd::Restore { name },
                Some(name) => UserCmd::Isolate { name },
                None => UserCmd::Invalid(format!("usage: {} <task>", cmd)),
            }
        }
        "/rename" => match words.as_slice() {
            [a, b] => UserCmd::Rename {
                name: a.trim_start_matches('@').to_string(),
                new_name: b.trim_start_matches('@').to_string(),
            },
            [b] if focus != MAIN => UserCmd::Rename {
                name: focus.to_string(),
                new_name: b.trim_start_matches('@').to_string(),
            },
            _ => UserCmd::Invalid("usage: /rename <task> <new-name>".into()),
        },
        "/answer" | "/reply" => {
            let (n, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            match n.trim_start_matches('#').parse::<u64>() {
                Ok(card) if !text.trim().is_empty() => UserCmd::Answer {
                    card,
                    text: text.trim().to_string(),
                },
                _ => UserCmd::Invalid("usage: /answer <card> <answer>".into()),
            }
        }
        "/close" => match rest.trim().trim_start_matches('#').parse::<u64>() {
            Ok(card) => UserCmd::Close { card },
            _ => UserCmd::Invalid("usage: /close <card>".into()),
        },
        "/cancel" => UserCmd::Cancel,
        "/tasks" => UserCmd::Tasks,
        "/interrupt" => UserCmd::Interrupt,
        "/help" => UserCmd::Help,
        _ => UserCmd::Passthrough(line.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_goes_to_the_focus() {
        assert_eq!(parse("  bonjour ", MAIN), UserCmd::Say("bonjour".into()));
    }

    #[test]
    fn at_name_is_an_explicit_route() {
        assert_eq!(
            parse("@docs v2 please", MAIN),
            UserCmd::To {
                target: "docs".into(),
                text: "v2 please".into()
            }
        );
        assert!(matches!(parse("@docs", MAIN), UserCmd::Invalid(_)));
    }

    #[test]
    fn new_with_flags_and_name() {
        assert_eq!(
            parse("/new -w --with-changes fix-safari: le login casse", MAIN),
            UserCmd::New {
                name: Some("fix-safari".into()),
                brief: "le login casse".into(),
                worktree: true,
                with_changes: true
            }
        );
        assert_eq!(
            parse("/new corrige: ceci", MAIN),
            UserCmd::New {
                name: Some("corrige".into()),
                brief: "ceci".into(),
                worktree: false,
                with_changes: false
            }
        );
        // a colon inside a sentence is not a name
        assert_eq!(
            parse("/new Regarde ceci: le bug", MAIN),
            UserCmd::New {
                name: None,
                brief: "Regarde ceci: le bug".into(),
                worktree: false,
                with_changes: false
            }
        );
        assert!(matches!(
            parse("/new --with-changes x: y", MAIN),
            UserCmd::Invalid(_)
        ));
    }

    #[test]
    fn drop_defaults_to_the_task_in_focus() {
        assert_eq!(
            parse("/drop", "docs"),
            UserCmd::Drop {
                name: Some("docs".into()),
                force: false
            }
        );
        assert_eq!(
            parse("/drop", MAIN),
            UserCmd::Drop {
                name: None,
                force: false
            }
        );
        assert_eq!(
            parse("/drop @bench --force", MAIN),
            UserCmd::Drop {
                name: Some("bench".into()),
                force: true
            }
        );
    }

    #[test]
    fn answer_and_passthrough() {
        assert_eq!(
            parse("/answer #3 v2", MAIN),
            UserCmd::Answer {
                card: 3,
                text: "v2".into()
            }
        );
        assert_eq!(
            parse("/compact", "docs"),
            UserCmd::Passthrough("/compact".into())
        );
    }

    #[test]
    fn names_and_slugs() {
        assert!(valid_name("fix-safari-2"));
        assert!(!valid_name("Fix"));
        assert!(!valid_name("-x"));
        assert!(!valid_name(&"a".repeat(25)));
        assert_eq!(slug("Corrige le login Safari, vite"), "corrige-le-login");
        assert_eq!(slug("Écrire la doc de l'API"), "ecrire-la-doc");
        assert_eq!(slug("!!!"), "task");
        assert!(valid_name(&slug("un nom extrêmement long qui dépasse")));
    }
}
