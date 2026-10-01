use super::*;

const LIST: &str = r##"{"items": [
  {"name": "Marie - Neutral", "slug": "fr_marie_neutral", "languages": ["fr_fr"], "gender": "female", "age": 30,
   "tags": ["neutral"], "id": "5f0a-uuid-1", "retention_notice": 30},
  {"name": "Paul - Neutral", "slug": "en_paul_neutral", "languages": ["en_us"], "gender": "male", "id": "5f0a-uuid-2"},
  {"name": "My clone", "slug": null, "languages": [], "gender": null, "id": "c10e-uuid"},
  {"name": "no id at all"}
], "page": 1, "page_size": 100, "total": 3, "total_pages": 1}"##;

#[test]
fn a_list_is_read_slug_first_and_a_voice_without_an_id_skipped() {
    let v = parse(LIST).unwrap();
    assert_eq!(v.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["fr_marie_neutral", "en_paul_neutral", "c10e-uuid"]);
    assert_eq!(v[0].languages, ["fr_fr"]);
    assert_eq!(v[0].gender.as_deref(), Some("female"));
    assert_eq!(v[2].gender, None);
    assert_eq!(total(LIST), Some(3));
    // `data` and a bare array too
    assert_eq!(parse(r#"{"data": [{"id": "a"}]}"#).unwrap()[0].id, "a");
    assert_eq!(parse(r#"[{"slug": "b"}]"#).unwrap()[0].id, "b");
    assert!(parse("<html>").is_err());
    assert!(parse(r#"{"detail": "nope"}"#).is_err());
}

#[test]
fn a_voice_says_its_name_its_language_and_its_gender() {
    let v = parse(LIST).unwrap();
    assert_eq!(v[0].label(), "Marie, neutral");
    assert_eq!(v[0].line("·"), "Marie, neutral · French · female");
    assert_eq!(v[2].line("-"), "My clone");
    // no languages listed: the slug's prefix
    let p = Voice { id: "fr_x".into(), name: String::new(), languages: vec![], gender: None };
    assert_eq!(p.langs(), ["fr"]);
    assert_eq!(p.label(), "X");
}

#[test]
fn the_languages_the_voices_speak_known_ones_first() {
    let mut v = fake();
    v.push(Voice { id: "xx".into(), name: "X".into(), languages: vec!["sw_ke".into(), "de_de".into()], gender: None });
    assert_eq!(languages(&v), ["en", "fr", "de", "sw"]);
    assert_eq!(languages(&[]), ["en", "fr"]);
    assert_eq!(language_name("fr_FR"), "French");
    assert_eq!(language_name("sw"), "sw");
    assert_eq!(base_language("EN-gb"), "en");
}

#[test]
fn the_voices_of_the_language_come_first() {
    let ids = |l: Option<&str>| ordered(&fake(), l).into_iter().map(|v| v.id).collect::<Vec<_>>();
    assert_eq!(ids(Some("fr")), ["fr_louis_calm", "fr_marie_neutral", "en_jane_cheerful", "en_paul_neutral"]);
    assert_eq!(ids(None), ["en_jane_cheerful", "en_paul_neutral", "fr_louis_calm", "fr_marie_neutral"]);
}

#[test]
fn the_request_is_a_get_with_the_key_in_its_header_only() {
    let h = head("/v1/audio/voices?limit=100&offset=0", "api.mistral.ai", "sk-test");
    assert!(h.starts_with("GET /v1/audio/voices?limit=100&offset=0 HTTP/1.1\r\n"));
    assert!(h.contains("Authorization: Bearer sk-test\r\n"));
    assert!(h.ends_with("\r\n\r\n"));
    assert!(!head("/", "localhost", "").contains("Authorization"));
}

/// A local server answers the list: fetch reads it (no network).
#[test]
fn fetch_reads_the_list_from_the_server() {
    use std::io::{BufRead, BufReader};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (s, _) = l.accept().unwrap();
        let mut r = BufReader::new(s.try_clone().unwrap());
        let mut first = String::new();
        r.read_line(&mut first).unwrap();
        let mut auth = false;
        loop {
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            auth |= line.trim() == "Authorization: Bearer k";
            if line == "\r\n" {
                break;
            }
        }
        let mut s = s;
        let body = LIST.as_bytes();
        write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).unwrap();
        s.write_all(body).unwrap();
        (first, auth)
    });
    let api = Endpoint {
        name: "mistral/voxtral-mini-tts-2603".into(),
        provider_name: "Mistral".into(),
        base_url: format!("http://127.0.0.1:{}/v1/", port),
        model: "voxtral-mini-tts-2603".into(),
        key: "k".into(),
    };
    let v = fetch(&api).unwrap();
    assert_eq!(v.len(), 3);
    let (first, auth) = server.join().unwrap();
    assert_eq!(first.trim(), "GET /v1/audio/voices?limit=100&offset=0 HTTP/1.1");
    assert!(auth);
}

/// By hand, with the user's Mistral key (nothing played):
/// `cargo test -p bend-tui voices_live -- --ignored --nocapture`
#[test]
#[ignore]
fn voices_live() {
    let cfg = crate::voicemode::config::VoiceModeConfig::default();
    let job = crate::voicemode::config::say_job(&cfg).expect("a Mistral key");
    let v = fetch(&job.api).expect("the list");
    assert!(!v.is_empty());
    for x in &v {
        println!("{}  {}", x.id, x.line("·"));
    }
}

#[test]
fn a_failed_list_says_why_once_and_a_refused_key_what_fixes_it() {
    assert_eq!(failed_line("Mistral", &FetchError::Status(401)), "Mistral refused the key (401). /provider fixes it.");
    assert_eq!(
        failed_line("Mistral", &FetchError::Status(503)),
        "Mistral's voices didn't load (HTTP 503). esc, then /voice tries again."
    );
    for e in [FetchError::Status(500), FetchError::Unreachable, FetchError::NotAList, FetchError::Status(403)] {
        assert_eq!(failed_line("Mistral", &e).matches('(').count(), 1, "{e:?}");
    }
}
