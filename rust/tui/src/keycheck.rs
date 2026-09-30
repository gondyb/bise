//! The first run's key check (BISE-266): one tiny real call to the model
//! just picked, with the key just pasted (or found), before anything is
//! saved. It says in plain words what went wrong: a wrong key, an account
//! with no credit, a model the provider doesn't know, or no answer at all.
//!
//! The call is the family's own (openai-chat: `POST {base}/chat/completions`
//! with a bearer key; anthropic: `POST {base}/messages` with `x-api-key`),
//! a one-word prompt and a small output cap: it costs a few tokens.
//! `BEND_PROVIDER_URL` points it elsewhere like the runtime's calls (the
//! tests' fake provider). The key is never printed nor logged.

use std::time::Duration;

/// What a check needs: the provider's wire family, where it answers, the
/// model id (without the provider) and the key.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Call {
    pub provider: String,
    pub api: String,
    pub base_url: String,
    pub model: String,
    pub key: String,
}

impl std::fmt::Debug for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Call {{ {} {} {} }}", self.provider, self.api, self.model)
    }
}

/// Why a key did not pass: bise's kind of failure and the provider's own
/// words (BISE-282).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fail {
    pub why: Why,
    /// the provider's error message, one line, at most [`SAID_MAX`] chars,
    /// the key masked; "" when it said nothing useful
    pub said: String,
}

/// The kinds of failure, each with its own fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    /// 401 / 403, an authentication error: the provider refused the key
    WrongKey,
    /// 402, or a quota/credit/billing error: the key is fine, the account
    /// can't pay
    NoCredit,
    /// 404, or a 400 about the model: the key is fine, the model isn't
    Model,
    /// a permission error (Anthropic's 403): the key is fine, but it may
    /// not use this model
    NoAccess,
    /// no answer, or the provider's own trouble (5xx, a rate limit): the
    /// short reason
    Unreachable(String),
}

impl Fail {
    /// A failure with no words from the provider.
    pub(crate) fn of(why: Why) -> Fail {
        Fail { why, said: String::new() }
    }
}

/// The longest provider message kept.
const SAID_MAX: usize = 200;

/// How long a refused key waits before its one quiet retry (a key made
/// seconds ago may not have reached every server yet).
const RETRY_AFTER: Duration = Duration::from_secs(3);

/// The request of a call (its url, headers and body).
pub(crate) fn request(c: &Call, env: &dyn Fn(&str) -> Option<String>) -> crate::voice::http::Request {
    let base = c.base_url.trim_end_matches('/');
    let (endpoint, mut headers, body) = if c.api == "anthropic" {
        (
            format!("{base}/messages"),
            vec![("x-api-key".to_string(), c.key.clone()), ("anthropic-version".to_string(), "2023-06-01".to_string())],
            serde_json::json!({
                "model": c.model,
                "max_tokens": 16,
                "messages": [{ "role": "user", "content": "hi" }],
            }),
        )
    } else {
        // OpenAI's reasoning models take max_completion_tokens only
        let cap = if c.provider == "openai" { "max_completion_tokens" } else { "max_tokens" };
        let mut b = serde_json::json!({
            "model": c.model,
            "messages": [{ "role": "user", "content": "hi" }],
        });
        b[cap] = serde_json::json!(16);
        (format!("{base}/chat/completions"), vec![("Authorization".to_string(), format!("Bearer {}", c.key))], b)
    };
    headers.push(("Content-Type".into(), "application/json".into()));
    let url = env("BEND_PROVIDER_URL").filter(|u| !u.trim().is_empty()).unwrap_or(endpoint);
    crate::voice::http::Request { url, headers, body: body.to_string().into_bytes() }
}

/// What an answer means: Ok when the provider accepted the key and the
/// model; else why not, with the provider's words (`key` masked in them).
pub(crate) fn verdict(status: u16, body: &[u8], key: &str) -> Result<(), Fail> {
    why(status, body).map_err(|why| Fail { why, said: said(body, key) })
}

/// The kind of an answer: the provider's structured error type or code
/// first (Anthropic's `error.type`, OpenAI's `error.code`), then the
/// status and the words of the body.
fn why(status: u16, body: &[u8]) -> Result<(), Why> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    let about = |words: &[&str]| words.iter().any(|w| text.contains(w));
    let money = ["credit", "quota", "billing", "balance", "insufficient", "payment", "purchase", "funds"];
    if status == 402 {
        return Err(Why::NoCredit);
    }
    for tag in error_tags(body) {
        match tag.as_str() {
            "authentication_error" | "invalid_api_key" => return Err(Why::WrongKey),
            "insufficient_quota" | "billing_error" => return Err(Why::NoCredit),
            "permission_error" if about(&money) => return Err(Why::NoCredit),
            "permission_error" => return Err(Why::NoAccess),
            "not_found_error" | "model_not_found" => return Err(Why::Model),
            _ => {}
        }
    }
    match status {
        401 | 403 if about(&money) => Err(Why::NoCredit),
        401 | 403 => Err(Why::WrongKey),
        404 => Err(Why::Model),
        429 if about(&money) => Err(Why::NoCredit),
        // Anthropic says no credit with a 400 invalid_request_error
        400 if about(&money) => Err(Why::NoCredit),
        // some providers say a bad key with a 400
        400 if about(&["api key", "api_key", "apikey", "authentication", "unauthorized"]) => Err(Why::WrongKey),
        400 | 422 if about(&["model"]) => Err(Why::Model),
        // past the key and the model: a 400 about the small request itself
        400 | 422 => Ok(()),
        s => Err(Why::Unreachable(format!("it answered {}", s))),
    }
}

/// The body as JSON, its first item when it is a list (Google's
/// OpenAI-compatible errors).
fn json_of(body: &[u8]) -> Option<serde_json::Value> {
    match serde_json::from_slice(body).ok()? {
        serde_json::Value::Array(a) => a.into_iter().next(),
        v => Some(v),
    }
}

/// The error's type and code, when the body has them (`error.type`,
/// `error.code`), lowercase.
fn error_tags(body: &[u8]) -> Vec<String> {
    let Some(v) = json_of(body) else { return Vec::new() };
    ["/error/type", "/error/code"]
        .iter()
        .filter_map(|p| v.pointer(p).and_then(|x| x.as_str()))
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// The provider's own message: `error.message`, `message`, `error` (a
/// string) or `detail` of a JSON body, else a short text body (never an
/// HTML page); one line, at most [`SAID_MAX`] chars, `key` masked. ""
/// when there is nothing useful.
pub(crate) fn said(body: &[u8], key: &str) -> String {
    let raw = match json_of(body) {
        Some(v) => ["/error/message", "/message", "/error", "/detail"]
            .iter()
            .find_map(|p| v.pointer(p).and_then(|x| x.as_str()).filter(|t| !t.trim().is_empty()))
            .unwrap_or("")
            .to_string(),
        None => {
            let t = String::from_utf8_lossy(body);
            if t.trim_start().starts_with('<') { String::new() } else { t.into_owned() }
        }
    };
    let line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let line = mask(&line, key);
    if matches!(line.to_ascii_lowercase().as_str(), "" | "error" | "null" | "unknown error") {
        return String::new();
    }
    match line.char_indices().nth(SAID_MAX) {
        Some((i, _)) => format!("{}…", line[..i].trim_end()),
        None => line,
    }
}

/// `text` with every run of key characters that is part of `key` (12
/// chars or more: the key, or a piece of it echoed back) shown as
/// `sk-…abcd`.
fn mask(text: &str, key: &str) -> String {
    let key = key.trim();
    if key.len() < 12 {
        return text.to_string();
    }
    let shown = {
        let head: String = key.chars().take_while(|c| *c != '-').take(8).collect();
        let head = if head.len() < key.len() && key[head.len()..].starts_with('-') { format!("{}-", head) } else { key.chars().take(3).collect() };
        let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        format!("{}…{}", head, tail)
    };
    let is_key_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '*' | '+' | '/' | '=');
    let mut out = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() >= 12 && key.contains(run.trim_end_matches('.')) {
            out.push_str(&shown);
            if run.ends_with('.') {
                out.push('.');
            }
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if is_key_char(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// Make the call (20 s at most) and say what it means. A refused key is
/// tried once more after [`RETRY_AFTER`], quietly: a new key may take a
/// few seconds to work everywhere.
pub(crate) fn check(c: &Call, env: &dyn Fn(&str) -> Option<String>) -> Result<(), Fail> {
    check_with(c, env, RETRY_AFTER)
}

fn check_with(c: &Call, env: &dyn Fn(&str) -> Option<String>, retry_after: Duration) -> Result<(), Fail> {
    match check_once(c, env) {
        Err(f) if f.why == Why::WrongKey => {
            std::thread::sleep(retry_after);
            check_once(c, env)
        }
        r => r,
    }
}

fn check_once(c: &Call, env: &dyn Fn(&str) -> Option<String>) -> Result<(), Fail> {
    let req = request(c, env);
    match crate::voice::http::send(&req, Duration::from_secs(20)) {
        Ok(r) => verdict(r.status, &r.body, &c.key),
        Err(e) => Err(Fail::of(Why::Unreachable(mask(&e, &c.key)))),
    }
}

/// The same check for the command line (`bise login --check`, `bise auth
/// check`, BISE-273): `model` ("provider/id") resolved in `setup`'s
/// catalog, called with `key`; Err = the reason in the key step's words
/// (never the key).
pub fn check_model(
    setup: &bise_catalog::Setup,
    model: &str,
    key: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(), String> {
    let r = setup.catalog.resolve(model);
    if r.known == bise_catalog::Known::NoProvider {
        return Err(format!("unknown provider '{}' in {}", r.provider, model));
    }
    let name = setup.catalog.provider(&r.provider).map(|p| p.name.clone()).unwrap_or_else(|| r.provider.clone());
    let call = Call { provider: r.provider.clone(), api: r.api.clone(), base_url: r.base_url.clone(), model: r.id.clone(), key: key.to_string() };
    check(&call, env).map_err(|f| say(&f, &name, &r.name))
}

/// Why a key did not pass, in the key step's words, with the provider's
/// own ("Anthropic said: \"…\"").
pub(crate) fn say(f: &Fail, provider: &str, model: &str) -> String {
    let base = match &f.why {
        Why::WrongKey => format!("{} says this key is wrong", provider),
        Why::NoCredit => format!("the key works, but your {} account has no credit yet", provider),
        Why::Model => format!("{} doesn't know {}: pick another model (--model)", provider, model),
        Why::NoAccess => format!("this key can't use {}: pick another model (--model)", model),
        Why::Unreachable(e) => format!("i couldn't reach {}: {}", provider, e.trim_end_matches('.')),
    };
    if f.said.is_empty() {
        base
    } else {
        format!("{}. {} said: \"{}\"", base, provider, f.said)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(api: &str, provider: &str) -> Call {
        Call { provider: provider.into(), api: api.into(), base_url: "https://x.test/v1/".into(), model: "m1".into(), key: "k-secret".into() }
    }

    #[test]
    fn each_family_gets_its_own_tiny_request() {
        let none = |_: &str| None;
        let r = request(&call("anthropic", "anthropic"), &none);
        assert_eq!(r.url, "https://x.test/v1/messages");
        assert!(r.headers.contains(&("x-api-key".into(), "k-secret".into())));
        let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!((b["model"].as_str(), b["max_tokens"].as_u64()), (Some("m1"), Some(16)));
        let r = request(&call("openai-chat", "mistral"), &none);
        assert_eq!(r.url, "https://x.test/v1/chat/completions");
        assert!(r.headers.contains(&("Authorization".into(), "Bearer k-secret".into())));
        let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(b["max_tokens"].as_u64(), Some(16));
        let b: serde_json::Value = serde_json::from_slice(&request(&call("openai-chat", "openai"), &none).body).unwrap();
        assert_eq!((b["max_completion_tokens"].as_u64(), b.get("max_tokens")), (Some(16), None));
        // the tests' fake provider
        let fake = |k: &str| (k == "BEND_PROVIDER_URL").then(|| "http://127.0.0.1:9/v1/chat/completions".to_string());
        assert_eq!(request(&call("openai-chat", "mistral"), &fake).url, "http://127.0.0.1:9/v1/chat/completions");
        // the key never shows in the debug form
        assert!(!format!("{:?} {:?}", call("anthropic", "a"), r).contains("k-secret"));
    }

    fn v(status: u16, body: &str) -> Result<(), Why> {
        verdict(status, body.as_bytes(), KEY).map_err(|f| f.why)
    }

    const KEY: &str = "sk-ant-api03-FAKEfakeFAKEfake0123456789abcdefABCDEF-wxyzAA";

    #[test]
    fn answers_mean_what_the_user_can_fix() {
        assert_eq!(v(200, "{}"), Ok(()));
        assert_eq!(v(401, r#"{"detail":"Invalid API Key"}"#), Err(Why::WrongKey));
        assert_eq!(v(403, "forbidden"), Err(Why::WrongKey));
        assert_eq!(v(402, ""), Err(Why::NoCredit));
        assert_eq!(v(429, r#"{"error":{"code":"insufficient_quota"}}"#), Err(Why::NoCredit));
        assert_eq!(v(400, r#"{"error":{"message":"Your credit balance is too low"}}"#), Err(Why::NoCredit));
        assert_eq!(v(400, r#"{"error":{"message":"API key not valid"}}"#), Err(Why::WrongKey));
        assert_eq!(v(404, "not found"), Err(Why::Model));
        assert_eq!(v(400, r#"{"message":"Invalid model: nope"}"#), Err(Why::Model));
        assert_eq!(v(400, r#"{"error":"temperature out of range"}"#), Ok(()));
        assert!(matches!(v(503, ""), Err(Why::Unreachable(_))));
        assert!(matches!(v(429, "slow down"), Err(Why::Unreachable(_))));
    }

    // BISE-282: the providers' real answers (copied from their APIs; the
    // request ids shortened)
    const ANTHROPIC_NO_CREDIT: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits."},"request_id":"req_011CTx"}"#;
    const ANTHROPIC_BAD_KEY: &str = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"},"request_id":"req_011CTy"}"#;
    const ANTHROPIC_NO_ACCESS: &str = r#"{"type":"error","error":{"type":"permission_error","message":"Your API key does not have permission to use the specified resource."},"request_id":"req_011CTz"}"#;
    const ANTHROPIC_NO_MODEL: &str = r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-nope"},"request_id":"req_011CTw"}"#;
    const OPENAI_BAD_KEY: &str = r#"{
    "error": {
        "message": "Incorrect API key provided: sk-proj-********************wxyz. You can find your API key at https://platform.openai.com/account/api-keys.",
        "type": "invalid_request_error",
        "param": null,
        "code": "invalid_api_key"
    }
}"#;
    const OPENAI_NO_QUOTA: &str = r#"{
    "error": {
        "message": "You exceeded your current quota, please check your plan and billing details. For more information on this error, read the docs: https://platform.openai.com/docs/guides/error-codes/api-errors.",
        "type": "insufficient_quota",
        "param": null,
        "code": "insufficient_quota"
    }
}"#;
    const OPENAI_NO_MODEL: &str = r#"{"error":{"message":"The model `gpt-nope` does not exist or you do not have access to it.","type":"invalid_request_error","param":null,"code":"model_not_found"}}"#;
    const OPENROUTER_NO_CREDIT: &str = r#"{"error":{"message":"Insufficient credits. Add more using https://openrouter.ai/settings/credits","code":402}}"#;
    const OPENROUTER_BAD_KEY: &str = r#"{"error":{"message":"No auth credentials found","code":401}}"#;

    #[test]
    fn the_providers_real_answers() {
        // the user's first run: a new key of an account with no credit yet
        // (BISE-282: bise said "Anthropic says this key is wrong")
        assert_eq!(v(400, ANTHROPIC_NO_CREDIT), Err(Why::NoCredit));
        assert_eq!(v(401, ANTHROPIC_BAD_KEY), Err(Why::WrongKey));
        // a 403 permission error: the key is right, the model is not for it
        assert_eq!(v(403, ANTHROPIC_NO_ACCESS), Err(Why::NoAccess));
        assert_eq!(v(404, ANTHROPIC_NO_MODEL), Err(Why::Model));
        assert_eq!(v(401, OPENAI_BAD_KEY), Err(Why::WrongKey));
        assert_eq!(v(429, OPENAI_NO_QUOTA), Err(Why::NoCredit));
        assert_eq!(v(404, OPENAI_NO_MODEL), Err(Why::Model));
        assert_eq!(v(402, OPENROUTER_NO_CREDIT), Err(Why::NoCredit));
        assert_eq!(v(401, OPENROUTER_BAD_KEY), Err(Why::WrongKey));
        // the structured type wins over the status and the words
        assert_eq!(v(400, r#"{"error":{"type":"authentication_error","message":"no credit for you"}}"#), Err(Why::WrongKey));
        // Google's list form
        assert_eq!(v(400, r#"[{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}]"#), Err(Why::WrongKey));
    }

    #[test]
    fn the_providers_words_are_kept_short_and_without_the_key() {
        let said = |status: u16, body: &str| verdict(status, body.as_bytes(), KEY).unwrap_err().said;
        assert_eq!(said(401, ANTHROPIC_BAD_KEY), "invalid x-api-key");
        assert_eq!(said(400, ANTHROPIC_NO_CREDIT), "Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.");
        // one line; OpenAI masks the key itself
        let s = said(401, OPENAI_BAD_KEY);
        assert!(s.starts_with("Incorrect API key provided: sk-proj-****") && !s.contains('\n'), "{}", s);
        assert_eq!(said(402, OPENROUTER_NO_CREDIT), "Insufficient credits. Add more using https://openrouter.ai/settings/credits");
        // the other shapes: message, error as a string, detail, plain text
        assert_eq!(said(401, r#"{"message":"Unauthorized"}"#), "Unauthorized");
        assert_eq!(said(401, r#"{"error":"bad token"}"#), "bad token");
        assert_eq!(said(401, r#"{"detail":"Invalid API Key"}"#), "Invalid API Key");
        assert_eq!(said(403, "  forbidden\n here "), "forbidden here");
        // nothing useful: no line
        for b in ["", "{}", r#"{"error":"error"}"#, "<html><body>403 Forbidden</body></html>", r#"{"error":{"code":401}}"#] {
            assert_eq!(said(401, b), "", "{:?}", b);
        }
        // the key echoed back, whole or a piece of it: masked
        let echo = format!(r#"{{"error":{{"message":"key {} is revoked. ({})"}}}}"#, KEY, &KEY[..20]);
        let s = said(401, &echo);
        assert_eq!(s, "key sk-…yzAA is revoked. (sk-…yzAA)");
        assert!(!s.contains("FAKEfake"), "{}", s);
        // at most 200 chars
        let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "word ".repeat(100));
        let s = said(401, &long);
        assert!(s.chars().count() <= 201 && s.ends_with('…'), "{}", s);
    }

    #[test]
    fn a_refused_key_is_tried_once_more() {
        // a fake provider: 401 on the first call, then 200
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/messages", l.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut calls = 0;
            for (status, body) in [(401, ANTHROPIC_BAD_KEY), (200, "{}")] {
                let (mut s, _) = l.accept().unwrap();
                use std::io::{Read, Write};
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                calls += 1;
                let _ = write!(s, "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, body.len(), body);
            }
            calls
        });
        let env = move |k: &str| (k == "BEND_PROVIDER_URL").then(|| url.clone());
        assert_eq!(check_with(&call("anthropic", "anthropic"), &env, Duration::from_millis(10)), Ok(()));
        assert_eq!(server.join().unwrap(), 2);
    }

    #[test]
    fn the_command_line_says_it_with_the_providers_words() {
        let f = verdict(400, ANTHROPIC_NO_CREDIT.as_bytes(), KEY).unwrap_err();
        assert_eq!(
            say(&f, "Anthropic", "claude-opus-5-5"),
            "the key works, but your Anthropic account has no credit yet. Anthropic said: \"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.\""
        );
        assert_eq!(say(&Fail::of(Why::NoAccess), "Anthropic", "claude-opus-5-5"), "this key can't use claude-opus-5-5: pick another model (--model)");
    }
}
