//! When each word of a sentence is said (owner: voice-tts; plan §4.5).
//! The TTS gives no word timings: the words share the audio by weight
//! (their syllables, plus a pause after `,` and `.`). A word is said once
//! its sound is over; the last word is said exactly at the end. Pure.

use super::Sentence;
use std::time::Duration;

/// One syllable at 1× (~4.8 syllables a second, a calm voice).
const SYLLABLE_MS: f32 = 210.0;
/// The pause after a sentence's end, in syllables.
const STOP_PAUSE: f32 = 1.5;
/// The pause after a comma, a colon, a semicolon or a dash.
const COMMA_PAUSE: f32 = 0.6;

/// How long `say` takes at `speed` before its audio is all in.
pub fn estimate(say: &str, speed: f32) -> Duration {
    let w: f32 = say.split_whitespace().map(|t| syllables(t) + pause(t)).sum();
    Duration::from_millis((w * SYLLABLE_MS / speed.clamp(0.5, 3.0)).round() as u64)
}

/// How many words of `s` are said after `played` of `total` audio:
/// monotonic in `played`, all of them once `played >= total`, never the
/// last before.
pub fn said_upto(s: &Sentence, played: Duration, total: Duration) -> usize {
    if s.words.is_empty() {
        return 0;
    }
    if played >= total {
        return s.words.len();
    }
    let f = played.as_secs_f64() / total.as_secs_f64();
    let ends = word_ends(s);
    ends.iter().take_while(|e| **e <= f).count()
}

/// When each word's sound is over, as a fraction of the sentence's audio
/// (increasing; the last is exactly 1).
pub fn word_ends(s: &Sentence) -> Vec<f64> {
    let words: Vec<&str> = s.words.iter().map(|w| s.say.get(w.say.clone()).unwrap_or("")).collect();
    let total: f64 = words.iter().map(|t| (syllables(t) + pause(t)) as f64).sum();
    let mut ends = Vec::with_capacity(words.len());
    let mut at = 0.0f64;
    for t in &words {
        let sound = syllables(t) as f64;
        ends.push(((at + sound) / total).min(1.0));
        at += sound + pause(t) as f64;
    }
    if let Some(last) = ends.last_mut() {
        *last = 1.0;
    }
    ends
}

/// About how many syllables a word has: its vowel groups (English's
/// silent final e dropped), at least one.
pub fn syllables(word: &str) -> f32 {
    let lower: Vec<char> = word.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect();
    if lower.is_empty() {
        return 0.0;
    }
    let vowel = |c: char| "aeiouyàâäéèêëîïôöùûüÿœæ".contains(c);
    let mut n = 0;
    let mut prev = false;
    for &c in &lower {
        let v = vowel(c);
        if v && !prev {
            n += 1;
        }
        prev = v;
    }
    let len = lower.len();
    if n > 1 && lower[len - 1] == 'e' && !vowel(lower[len - 2]) && !(len >= 3 && lower[len - 2] == 'l' && !vowel(lower[len - 3])) {
        n -= 1;
    }
    n.max(1) as f32
}

/// The pause after a word, from its punctuation.
fn pause(word: &str) -> f32 {
    match word.trim_end_matches(['"', '\'', ')', '”', '’']).chars().last() {
        Some('.' | '!' | '?' | '…') => STOP_PAUSE,
        Some(',' | ';' | ':' | '—' | '–') => COMMA_PAUSE,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voicemode::Word;

    fn sentence(say: &str) -> Sentence {
        let mut words = Vec::new();
        let mut at = 0;
        for t in say.split(' ') {
            words.push(Word { say: at..at + t.len(), src: None });
            at += t.len() + 1;
        }
        Sentence { say: say.into(), words }
    }

    #[test]
    fn syllables_are_about_right() {
        assert_eq!(syllables("on"), 1.0);
        assert_eq!(syllables("agents"), 2.0);
        assert_eq!(syllables("the"), 1.0);
        assert_eq!(syllables("make"), 1.0);
        assert_eq!(syllables("table"), 2.0);
        assert_eq!(syllables("committed"), 3.0);
        assert_eq!(syllables("écran."), 2.0);
        assert_eq!(syllables("—"), 0.0);
    }

    #[test]
    fn the_estimate_grows_with_the_text_and_shrinks_with_the_speed() {
        let short = estimate("on it.", 1.0);
        let long = estimate("the login test waits for the event now, and nothing needs you.", 1.0);
        assert!(short < long);
        assert!(short >= Duration::from_millis(300) && short <= Duration::from_millis(1200), "{short:?}");
        // ~12 s for ~30 words is the plan's budget
        let thirty = ["word"; 30].join(" ") + ".";
        let t = estimate(&thirty, 1.0);
        assert!(t >= Duration::from_secs(5) && t <= Duration::from_secs(12), "{t:?}");
        assert!(estimate(&thirty, 1.5) < t);
        assert_eq!(estimate("", 1.0), Duration::ZERO);
    }

    #[test]
    fn words_light_in_order_and_the_last_exactly_at_the_end() {
        let s = sentence("done: the login test waits for the event now.");
        let total = Duration::from_millis(3000);
        assert_eq!(said_upto(&s, Duration::ZERO, total), 0);
        let mut prev = 0;
        for ms in (0..=3000).step_by(10) {
            let n = said_upto(&s, Duration::from_millis(ms), total);
            assert!(n >= prev, "monotonic at {ms}");
            if ms < 3000 {
                assert!(n < s.words.len(), "the last word waits for the end ({ms} ms)");
            }
            prev = n;
        }
        assert_eq!(said_upto(&s, total, total), s.words.len());
        assert_eq!(said_upto(&s, total * 2, total), s.words.len());
        // half the audio: about half the words
        let half = said_upto(&s, total / 2, total);
        assert!((3..=6).contains(&half), "{half}");
    }

    #[test]
    fn a_long_word_takes_longer_than_a_short_one() {
        let s = sentence("a extraordinarily b");
        let e = word_ends(&s);
        assert!(e[1] - e[0] > e[0], "{e:?}");
        assert_eq!(*e.last().unwrap(), 1.0);
    }

    #[test]
    fn the_pause_after_a_comma_is_not_a_word() {
        // "one," then a pause: "two" starts later than without the comma
        let with = word_ends(&sentence("one, two three"));
        let without = word_ends(&sentence("one two three"));
        assert!(with[0] < without[0]);
    }

    #[test]
    fn nothing_to_say_or_no_audio() {
        let empty = Sentence { say: String::new(), words: vec![] };
        assert_eq!(said_upto(&empty, Duration::from_secs(1), Duration::from_secs(1)), 0);
        let s = sentence("on it.");
        assert_eq!(said_upto(&s, Duration::ZERO, Duration::ZERO), 2);
    }
}
