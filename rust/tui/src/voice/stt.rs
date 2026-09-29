//! The speech-to-text providers behind one interface (BISE-130): a clip
//! and a [`VoiceJob`] → one HTTP request; the response → the text. The
//! family (`job.api`, bise_catalog::voice::STT_FAMILIES) picks the wire
//! format; which provider speaks which family is catalog data.

use super::http::{Request, Response};
use super::VoiceJob;
use serde_json::Value;

/// A multipart/form-data body.
struct Form {
    boundary: &'static str,
    body: Vec<u8>,
}

impl Form {
    fn new() -> Form {
        Form { boundary: "bise-voice-7d1f3c9a2e5b", body: Vec::new() }
    }
    fn text(&mut self, name: &str, value: &str) -> &mut Form {
        let head = format!("--{}\r\nContent-Disposition: form-data; name=\"{}\"\r\n\r\n", self.boundary, name);
        self.body.extend_from_slice(head.as_bytes());
        self.body.extend_from_slice(value.as_bytes());
        self.body.extend_from_slice(b"\r\n");
        self
    }
    fn file(&mut self, name: &str, filename: &str, mime: &str, data: &[u8]) -> &mut Form {
        let head = format!(
            "--{}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
            self.boundary, name, filename, mime
        );
        self.body.extend_from_slice(head.as_bytes());
        self.body.extend_from_slice(data);
        self.body.extend_from_slice(b"\r\n");
        self
    }
    fn finish(mut self) -> (String, Vec<u8>) {
        self.body.extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        (format!("multipart/form-data; boundary={}", self.boundary), self.body)
    }
}

/// `%XX` for everything but the unreserved characters (a query value).
pub fn url_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

fn language(job: &VoiceJob) -> Option<&str> {
    job.language.as_deref().map(str::trim).filter(|l| !l.is_empty())
}

/// The request of one clip (`wav`: a WAV file).
pub fn request(job: &VoiceJob, wav: &[u8]) -> Request {
    let base = job.base_url.trim_end_matches('/');
    let mut headers = Vec::new();
    let (url, content_type, body) = match job.api.as_str() {
        "deepgram" => {
            // nova-3: language=multi transcribes French and English mixed
            let mut q = format!(
                "model={}&smart_format=true&punctuate=true&language={}",
                url_encode(&job.model),
                url_encode(language(job).unwrap_or("multi"))
            );
            for w in &job.vocabulary {
                q.push_str(&format!("&keyterm={}", url_encode(w)));
            }
            headers.push(("Authorization".to_string(), format!("Token {}", job.key)));
            (format!("{}/listen?{}", base, q), "audio/wav".to_string(), wav.to_vec())
        }
        "elevenlabs" => {
            let mut f = Form::new();
            f.text("model_id", &job.model).text("tag_audio_events", "false");
            if let Some(l) = language(job) {
                f.text("language_code", l);
            }
            for w in &job.vocabulary {
                f.text("keyterms", w);
            }
            f.file("file", "audio.wav", "audio/wav", wav);
            let (ct, body) = f.finish();
            headers.push(("xi-api-key".to_string(), job.key.clone()));
            (format!("{}/speech-to-text", base), ct, body)
        }
        family => {
            // mistral, openai (and the OpenAI-compatible servers)
            let mut f = Form::new();
            f.text("model", &job.model);
            if let Some(l) = language(job) {
                f.text("language", l);
            }
            if family == "mistral" {
                for w in &job.vocabulary {
                    f.text("context_bias", w);
                }
            } else {
                f.text("response_format", "json");
                if !job.vocabulary.is_empty() {
                    f.text("prompt", &job.vocabulary.join(", "));
                }
            }
            f.file("file", "audio.wav", "audio/wav", wav);
            let (ct, body) = f.finish();
            if !job.key.is_empty() {
                headers.push(("Authorization".to_string(), format!("Bearer {}", job.key)));
            }
            (format!("{}/audio/transcriptions", base), ct, body)
        }
    };
    headers.push(("Content-Type".to_string(), content_type));
    Request { url, headers, body }
}

/// The text of a response; Err: one line (the status and the provider's
/// message, never the request).
pub fn parse(api: &str, resp: &Response) -> Result<String, String> {
    let v: Option<Value> = serde_json::from_slice(&resp.body).ok();
    if !(200..300).contains(&resp.status) {
        let msg = v
            .as_ref()
            .and_then(error_message)
            .unwrap_or_else(|| String::from_utf8_lossy(&resp.body).to_string());
        let msg = one_line(&msg);
        return Err(if msg.is_empty() {
            format!("HTTP {}", resp.status)
        } else {
            format!("HTTP {}: {}", resp.status, msg)
        });
    }
    let v = v.ok_or_else(|| "the response is not JSON".to_string())?;
    let text = match api {
        "deepgram" => v.pointer("/results/channels/0/alternatives/0/transcript"),
        _ => v.get("text"),
    };
    text.and_then(|t| t.as_str())
        .map(str::to_string)
        .ok_or_else(|| "the response has no text".to_string())
}

/// The providers' error shapes: {"error": {"message"}}, {"error": "..."},
/// {"message"}, {"detail": "..." | {"message"} | [{"msg"}]}, {"err_msg"}.
fn error_message(v: &Value) -> Option<String> {
    let s = |x: &Value| x.as_str().map(str::to_string);
    let e = v.get("error");
    e.and_then(|e| e.get("message"))
        .and_then(s)
        .or_else(|| e.and_then(s))
        .or_else(|| v.get("message").and_then(s))
        .or_else(|| v.get("detail").and_then(s))
        .or_else(|| v.pointer("/detail/message").and_then(s))
        .or_else(|| v.pointer("/detail/0/msg").and_then(s))
        .or_else(|| v.get("err_msg").and_then(s))
}

/// Whitespace folded, at most 200 characters.
pub fn one_line(s: &str) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 200 {
        format!("{}…", flat.chars().take(200).collect::<String>())
    } else {
        flat
    }
}
