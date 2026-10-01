//! The quick spoken "on it" (owner: voice-tts; design §7, plan §4.5):
//! one short chat call to the small-jobs model, ≤ 5 words in the
//! language you spoke, while the agent starts; past ACK_DEADLINE the
//! controller says a canned line instead.
//!
//! The call is the family's own, as `keycheck` makes it (anthropic:
//! `POST {base}/messages`; openai-responses: `{base}/responses`; else
//! `{base}/chat/completions`), over `voice::http`. The key is never printed.

use super::{AckJob, Acker};
use crate::voice::http;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Past this the line is useless (the controller waits ACK_DEADLINE).
const TIMEOUT: Duration = Duration::from_secs(3);
/// The longest line said.
pub const MAX_WORDS: usize = 5;

const INSTRUCTIONS: &str = "You voice a coding agent that just got a spoken request. \
Reply with one short spoken line, at most five words, saying you're on it. \
Use the language of the request. No quotes, no emoji, no markdown, nothing else.";

pub struct SmallAck;

impl Acker for SmallAck {
    fn start(&self, job: AckJob, heard: String, events: Sender<String>, cancel: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            let line = ack(&job, &heard, &|req| http::send(req, TIMEOUT));
            if let Some(line) = line {
                if !cancel.load(Ordering::SeqCst) {
                    let _ = events.send(line);
                }
            }
        });
    }
}

/// The line for `heard`, or None (no answer, a refusal, too long).
pub fn ack(job: &AckJob, heard: &str, send: &dyn Fn(&http::Request) -> Result<http::Response, String>) -> Option<String> {
    if heard.trim().is_empty() {
        return None;
    }
    let resp = send(&request(job, heard)).ok()?;
    if !(200..300).contains(&resp.status) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&resp.body).ok()?;
    clean(&text_of(&job.family, &v)?)
}

/// The chat request of the job's wire family.
pub fn request(job: &AckJob, heard: &str) -> http::Request {
    let base = job.api.base_url.trim_end_matches('/');
    let bearer = ("Authorization".to_string(), format!("Bearer {}", job.api.key));
    let (url, mut headers, body) = match job.family.as_str() {
        "anthropic" => (
            format!("{base}/messages"),
            vec![("x-api-key".to_string(), job.api.key.clone()), ("anthropic-version".to_string(), "2023-06-01".to_string())],
            serde_json::json!({
                "model": job.api.model,
                "max_tokens": 24,
                "system": INSTRUCTIONS,
                "messages": [{ "role": "user", "content": heard }],
            }),
        ),
        "openai-responses" => (
            format!("{base}/responses"),
            vec![bearer],
            serde_json::json!({
                "model": job.api.model,
                "instructions": INSTRUCTIONS,
                "input": heard,
                "max_output_tokens": 24,
                "store": false,
            }),
        ),
        _ => {
            // OpenAI's reasoning models take max_completion_tokens only
            let cap = if job.api.provider_name.eq_ignore_ascii_case("openai") { "max_completion_tokens" } else { "max_tokens" };
            let mut b = serde_json::json!({
                "model": job.api.model,
                "messages": [
                    { "role": "system", "content": INSTRUCTIONS },
                    { "role": "user", "content": heard },
                ],
            });
            b[cap] = serde_json::json!(24);
            (format!("{base}/chat/completions"), vec![bearer], b)
        }
    };
    headers.push(("Content-Type".into(), "application/json".into()));
    http::Request { url, headers, body: body.to_string().into_bytes() }
}

/// The model's text in the family's answer.
fn text_of(family: &str, v: &serde_json::Value) -> Option<String> {
    let s = match family {
        "anthropic" => v.pointer("/content/0/text").and_then(|t| t.as_str()).map(str::to_string),
        "openai-responses" => v.get("output_text").and_then(|t| t.as_str()).map(str::to_string).or_else(|| {
            v.get("output")?.as_array()?.iter().find_map(|o| {
                o.get("content")?.as_array()?.iter().find_map(|c| c.get("text").and_then(|t| t.as_str()).map(str::to_string))
            })
        }),
        _ => v.pointer("/choices/0/message/content").and_then(|t| t.as_str()).map(str::to_string),
    };
    s.filter(|t| !t.trim().is_empty())
}

/// A line fit to say: one line, no quotes or marks, ≤ [`MAX_WORDS`]
/// words; None when the model said more than that.
pub fn clean(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let line: String = line
        .chars()
        .filter(|c| !matches!(c, '"' | '“' | '”' | '«' | '»' | '*' | '_' | '`' | '#'))
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".,!?'’-…".contains(*c))
        .collect();
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.is_empty() || words.len() > MAX_WORDS {
        return None;
    }
    Some(words.join(" "))
}

/// A canned "on it" in English, turn `n`: [`canned_in`] with no language.
pub fn canned(n: u64) -> &'static str {
    canned_in(n, None)
}

/// A canned "on it", turn `n` (they take turns, never twice in a row), in
/// `lang`: the language you spoke (`speak::language(heard)`), English
/// when None or not French.
pub fn canned_in(n: u64, lang: Option<&str>) -> &'static str {
    const EN: [&str; 3] = ["on it.", "let me look.", "okay, one moment."];
    const FR: [&str; 3] = ["je m'en occupe.", "je regarde.", "d'accord, un instant."];
    let lines = match super::speak::Lang::of(lang) {
        super::speak::Lang::Fr => &FR,
        super::speak::Lang::En => &EN,
    };
    lines[(n % lines.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voicemode::Endpoint;
    use std::cell::RefCell;

    fn job(family: &str, provider: &str) -> AckJob {
        AckJob {
            api: Endpoint {
                name: format!("{provider}/small"),
                provider_name: provider.into(),
                base_url: "https://api.example.test/v1/".into(),
                model: "small".into(),
                key: "sk-secret".into(),
            },
            family: family.into(),
        }
    }

    fn body(r: &http::Request) -> serde_json::Value {
        serde_json::from_slice(&r.body).unwrap()
    }

    #[test]
    fn the_chat_request_asks_for_five_words_in_your_language() {
        let r = request(&job("openai", "mistral"), "lance les tests");
        assert_eq!(r.url, "https://api.example.test/v1/chat/completions");
        let b = body(&r);
        assert_eq!(b["model"], "small");
        assert_eq!(b["max_tokens"], 24);
        assert!(b["messages"][0]["content"].as_str().unwrap().contains("at most five words"));
        assert!(b["messages"][0]["content"].as_str().unwrap().contains("language of the request"));
        assert_eq!(b["messages"][1]["content"], "lance les tests");
        assert!(r.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer sk-secret"));
        assert!(!format!("{r:?}").contains("sk-secret"), "the key is never printed");
        assert_eq!(body(&request(&job("openai", "openai"), "x"))["max_completion_tokens"], 24);
    }

    #[test]
    fn anthropic_and_responses_families_use_their_own_calls() {
        let r = request(&job("anthropic", "anthropic"), "run the tests");
        assert_eq!(r.url, "https://api.example.test/v1/messages");
        assert!(r.headers.iter().any(|(k, v)| k == "x-api-key" && v == "sk-secret"));
        assert_eq!(body(&r)["system"], INSTRUCTIONS);
        let r = request(&job("openai-responses", "openai"), "run the tests");
        assert_eq!(r.url, "https://api.example.test/v1/responses");
        assert_eq!(body(&r)["input"], "run the tests");
    }

    #[test]
    fn the_answer_is_read_per_family() {
        fn ok(s: &'static str) -> impl Fn(&http::Request) -> Result<http::Response, String> {
            move |_| Ok(http::Response { status: 200, body: s.as_bytes().to_vec() })
        }
        let chat = r#"{"choices":[{"message":{"content":"\"On it, checking now.\""}}]}"#;
        assert_eq!(ack(&job("openai", "mistral"), "run it", &ok(chat)).as_deref(), Some("On it, checking now."));
        let anth = r#"{"content":[{"type":"text","text":"Je m'en occupe."}]}"#;
        assert_eq!(ack(&job("anthropic", "anthropic"), "lance", &ok(anth)).as_deref(), Some("Je m'en occupe."));
        let resp = r#"{"output":[{"type":"message","content":[{"type":"output_text","text":"Looking now."}]}]}"#;
        assert_eq!(ack(&job("openai-responses", "openai"), "look", &ok(resp)).as_deref(), Some("Looking now."));
    }

    #[test]
    fn nothing_is_said_on_a_failure_or_a_long_answer() {
        let calls = RefCell::new(0);
        let refused = |_: &http::Request| {
            *calls.borrow_mut() += 1;
            Ok(http::Response { status: 401, body: b"{}".to_vec() })
        };
        assert_eq!(ack(&job("openai", "mistral"), "run it", &refused), None);
        assert_eq!(*calls.borrow(), 1);
        let down = |_: &http::Request| Err("the request timed out".to_string());
        assert_eq!(ack(&job("openai", "mistral"), "run it", &down), None);
        let long = |_: &http::Request| {
            Ok(http::Response { status: 200, body: br#"{"choices":[{"message":{"content":"Sure! I will start working on that right away."}}]}"#.to_vec() })
        };
        assert_eq!(ack(&job("openai", "mistral"), "run it", &long), None);
        // nothing heard: no call
        let never = |_: &http::Request| -> Result<http::Response, String> { panic!("no call") };
        assert_eq!(ack(&job("openai", "mistral"), "  ", &never), None);
    }

    #[test]
    fn clean_keeps_one_plain_line() {
        assert_eq!(clean("**On it!** 🚀\nmore").as_deref(), Some("On it!"));
        assert_eq!(clean("« Je regarde. »").as_deref(), Some("Je regarde."));
        assert_eq!(clean("   "), None);
    }

    #[test]
    fn canned_lines_take_turns() {
        for n in 0..10 {
            assert_ne!(canned(n), canned(n + 1));
            assert_ne!(canned_in(n, Some("fr")), canned_in(n + 1, Some("fr")));
        }
    }

    #[test]
    fn canned_lines_are_in_the_language_you_spoke() {
        use crate::voicemode::speak::language;
        assert_eq!(canned_in(0, language("lance les tests et dis-moi")), "je m'en occupe.");
        assert_eq!(canned_in(1, Some("fr-FR")), "je regarde.");
        assert_eq!(canned_in(0, language("run the tests please")), "on it.");
        // not sure: English
        assert_eq!(canned_in(2, language("ok")), "okay, one moment.");
        assert_eq!(canned(0), canned_in(0, None));
    }

    #[test]
    fn the_acker_sends_the_line_from_its_thread() {
        // a local fake chat server
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut s, _) = l.accept().unwrap();
            // the whole request (headers, then Content-Length bytes):
            // answering after the first read closed the socket on a
            // body still being written (a flake under load)
            let mut req = Vec::new();
            let mut buf = vec![0u8; 8192];
            loop {
                let n = s.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if req.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let b = r#"{"choices":[{"message":{"content":"On it."}}]}"#;
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", b.len(), b);
        });
        let mut j = job("openai", "mistral");
        j.api.base_url = format!("http://{addr}/v1");
        let (tx, rx) = std::sync::mpsc::channel();
        SmallAck.start(j, "run the tests".into(), tx, Arc::new(AtomicBool::new(false)));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "On it.");
    }
}
