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

/// Why a key did not pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Fail {
    /// 401 / 403: the provider refused the key
    WrongKey,
    /// 402, or a quota/credit/billing error: the key is fine, the account
    /// can't pay
    NoCredit,
    /// 404, or a 400 about the model: the key is fine, the model isn't
    Model,
    /// no answer, or the provider's own trouble (5xx, a rate limit): the
    /// short reason
    Unreachable(String),
}

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
/// model.
pub(crate) fn verdict(status: u16, body: &[u8]) -> Result<(), Fail> {
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    let about = |words: &[&str]| words.iter().any(|w| text.contains(w));
    let money = ["credit", "quota", "billing", "balance", "insufficient", "payment", "purchase", "funds"];
    match status {
        200..=299 => Ok(()),
        401 | 403 if about(&money) => Err(Fail::NoCredit),
        401 | 403 => Err(Fail::WrongKey),
        402 => Err(Fail::NoCredit),
        404 => Err(Fail::Model),
        429 if about(&money) => Err(Fail::NoCredit),
        // some providers say a bad key with a 400
        400 if about(&["api key", "api_key", "apikey", "authentication", "unauthorized"]) => Err(Fail::WrongKey),
        400 if about(&money) => Err(Fail::NoCredit),
        400 | 422 if about(&["model"]) => Err(Fail::Model),
        // past the key and the model: a 400 about the small request itself
        400 | 422 => Ok(()),
        s => Err(Fail::Unreachable(format!("{} answered {}", "it", s))),
    }
}

/// Make the call (20 s at most) and say what it means.
pub(crate) fn check(c: &Call, env: &dyn Fn(&str) -> Option<String>) -> Result<(), Fail> {
    let req = request(c, env);
    match crate::voice::http::send(&req, Duration::from_secs(20)) {
        Ok(r) => verdict(r.status, &r.body),
        Err(e) => Err(Fail::Unreachable(e)),
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

/// Why a key did not pass, in the key step's words.
pub(crate) fn say(f: &Fail, provider: &str, model: &str) -> String {
    match f {
        Fail::WrongKey => format!("{} says this key is wrong", provider),
        Fail::NoCredit => "the key works, but the account has no credit".into(),
        Fail::Model => format!("{} doesn't know {}: pick another model (--model)", provider, model),
        Fail::Unreachable(e) => format!("i couldn't reach {}: {}", provider, e.trim_end_matches('.')),
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

    #[test]
    fn answers_mean_what_the_user_can_fix() {
        assert_eq!(verdict(200, b"{}"), Ok(()));
        assert_eq!(verdict(401, br#"{"detail":"Invalid API Key"}"#), Err(Fail::WrongKey));
        assert_eq!(verdict(403, b"forbidden"), Err(Fail::WrongKey));
        assert_eq!(verdict(402, b""), Err(Fail::NoCredit));
        assert_eq!(verdict(429, br#"{"error":{"code":"insufficient_quota"}}"#), Err(Fail::NoCredit));
        assert_eq!(verdict(400, br#"{"error":{"message":"Your credit balance is too low"}}"#), Err(Fail::NoCredit));
        assert_eq!(verdict(400, br#"{"error":{"message":"API key not valid"}}"#), Err(Fail::WrongKey));
        assert_eq!(verdict(404, b"not found"), Err(Fail::Model));
        assert_eq!(verdict(400, br#"{"message":"Invalid model: nope"}"#), Err(Fail::Model));
        assert_eq!(verdict(400, br#"{"error":"temperature out of range"}"#), Ok(()));
        assert!(matches!(verdict(503, b""), Err(Fail::Unreachable(_))));
        assert!(matches!(verdict(429, b"slow down"), Err(Fail::Unreachable(_))));
    }
}
