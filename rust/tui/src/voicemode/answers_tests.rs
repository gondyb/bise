//! Answers by voice: only "allow" allows, a choice by its number, its
//! rank or a word of its label, None when unsure.

use super::*;

fn c(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|s| s.to_string()).collect()
}

#[test]
fn only_the_word_allow_allows() {
    for yes in ["allow", "Allow.", "allow it", "ok allow", "allow, go", "yes, allow"] {
        assert!(is_allow(yes), "{yes}");
    }
    // backchannels never allow (design §5)
    for no in ["yes", "ok", "mm", "yeah sure", "oui", "d'accord", "go ahead", "", "allowance"] {
        assert!(!is_allow(no), "{no}");
    }
    // always allow is key only; a no is a no
    for no in ["always allow", "allow always", "don't allow", "do not allow", "never allow", "deny", "allow? hmm"] {
        assert!(!is_allow(no), "{no}");
    }
    // a sentence that happens to say it is words for the agent
    assert!(!is_allow("i wonder whether we should allow the script to write in the home folder"));
}

#[test]
fn a_choice_by_its_number() {
    let ch = c(&["bigger", "smaller", "keep it"]);
    for (h, want) in [("1", 0), ("2.", 1), ("3", 2), ("one", 0), ("two", 1), ("number two", 1), ("option 3", 2), ("deux", 1), ("trois", 2)] {
        assert_eq!(pick(h, &ch), Some(want), "{h}");
    }
    // out of range
    assert_eq!(pick("4", &ch), None);
    assert_eq!(pick("nine", &ch), None);
}

#[test]
fn a_choice_by_its_rank() {
    let ch = c(&["bigger", "smaller", "keep it"]);
    for (h, want) in [
        ("the first one", 0),
        ("first", 0),
        ("The second one, please.", 1),
        ("the third", 2),
        ("the last one", 2),
        ("le premier", 0),
        ("la première", 0),
        ("le deuxième", 1),
        ("le dernier", 2),
    ] {
        assert_eq!(pick(h, &ch), Some(want), "{h}");
    }
}

#[test]
fn a_choice_by_a_word_of_its_label() {
    let ch = c(&["make it bigger", "make it smaller", "keep the size"]);
    assert_eq!(pick("smaller", &ch), Some(1));
    assert_eq!(pick("the smaller one", &ch), Some(1));
    assert_eq!(pick("keep", &ch), Some(2));
    // a word in two labels names neither
    assert_eq!(pick("make it", &ch), None);
    // plurals
    let ch = c(&["both branches", "the tests only"]);
    assert_eq!(pick("the test", &ch), Some(1));
    assert_eq!(pick("both", &ch), Some(0));
}

#[test]
fn unsure_is_none() {
    let ch = c(&["bigger", "smaller", "keep it"]);
    for h in [
        "",
        "mm",
        "ok",
        "hmm let me think",
        "bigger or smaller",
        "the first or the second",
        "the second one, bigger",
        "which one is faster?",
        "actually can you explain the difference between the first and the third ones",
    ] {
        assert_eq!(pick(h, &ch), None, "{h}");
    }
    assert_eq!(pick("1", &[]), None);
}

#[test]
fn the_heard_line() {
    assert_eq!(heard_line("the first one", 1, "smaller"), "heard \"the first one\" → 1 smaller");
    assert_eq!(heard_line("Smaller.", 2, "smaller"), "heard \"Smaller\" → 2 smaller");
    let long = heard_line("the one that keeps everything as it is now", 3, "keep everything as it is right now please");
    assert!(long.contains('…') && long.chars().count() < 70, "{long}");
    assert_eq!(heard_allow("allow", "allow once"), "heard \"allow\" → allow once");
    assert_eq!(HEARD_FOR.as_millis(), 1500);
}
