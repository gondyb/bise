//! Answering the inbox by voice (owner: voice-settings; design §5, plan
//! §2). Pure. Stub: filled by voice-settings.

/// Only the word "allow" allows ("yes", "ok", "mm" never do; "always
/// allow" is key only).
pub fn is_allow(heard: &str) -> bool {
    heard.split(|c: char| !c.is_alphanumeric()).any(|w| w.eq_ignore_ascii_case("allow"))
}

/// The choice `heard` names ("1", "the first one", "smaller"); None:
/// unsure (the agent asks back).
pub fn pick(_heard: &str, _choices: &[String]) -> Option<usize> {
    None
}
