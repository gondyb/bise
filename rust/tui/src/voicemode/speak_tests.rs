use super::*;

fn says(msg: &str) -> Vec<String> {
    speakable(msg, None).sentences.into_iter().map(|s| s.say).collect()
}

fn says_fr(msg: &str) -> Vec<String> {
    speakable(msg, Some("fr")).sentences.into_iter().map(|s| s.say).collect()
}

fn n(v: &str) -> String {
    number(v, false, Lang::En).unwrap().0.join(" ")
}

fn n_fr(v: &str) -> String {
    number(v, false, Lang::Fr).unwrap().0.join(" ")
}

/// What holds for every message: said text clean of marks, words in
/// order, every word's source inside the message and on a char boundary,
/// a plain word's source is that word.
fn check(msg: &str, sp: &Spoken) {
    for s in &sp.sentences {
        for bad in ["*", "`", "#", "_", "http", "/", "|", "[", "]"] {
            assert!(!s.say.contains(bad), "{bad:?} said in {:?}", s.say);
        }
        let mut end = 0;
        for w in &s.words {
            assert!(w.say.start >= end && w.say.end > w.say.start, "{:?}", s);
            end = w.say.end;
            let text = &s.say[w.say.clone()];
            assert!(!text.contains(' '), "one word per Word: {text:?}");
            if let Some(src) = &w.src {
                assert!(src.end <= msg.len() && msg.is_char_boundary(src.start) && msg.is_char_boundary(src.end));
                let shown = &msg[src.clone()];
                let bare = text.trim_end_matches(['.', ',', ';', ':', '!', '?']);
                // a plain word lights itself (units and key names are said whole)
                let mapped = ["ctrl", "cmd", "esc", "ms", "s", "min", "h", "x"].contains(&shown);
                if !mapped && shown.chars().all(|c| c.is_alphabetic() || c == '\'' || c == '’') {
                    assert_eq!(bare, shown);
                }
            }
        }
        assert_eq!(end, s.say.len());
    }
    // the whole message: nothing is left for the screen
    assert!(!sp.more);
    for s in &sp.sentences {
        assert!(!s.say.contains("the rest") && !s.say.contains("la suite"), "{}", s.say);
    }
    // the message's words are said in its order
    let srcs: Vec<usize> = sp.sentences.iter().flat_map(|s| s.words.iter().filter_map(|w| w.src.as_ref().map(|r| r.start))).collect();
    assert!(srcs.windows(2).all(|w| w[0] <= w[1]), "out of order: {srcs:?}");
}

/// The said words of a sentence that light the message, as shown there.
fn lit<'a>(msg: &'a str, s: &Sentence) -> Vec<&'a str> {
    s.words.iter().filter_map(|w| w.src.clone()).map(|r| &msg[r]).collect()
}

// ---- real agent messages (from `sb history`) ----

const INBOX_KEYS: &str = "Done: ctrl+1 to ctrl+9 and a click now open inbox items. ctrl+g is gone, and so are ctrl+j and ctrl+k for switching agents. It's committed on main as **46c4da5** (BISE-302); designer signed off the tmux captures, and the quick and full gates are green.

**What you see**
- **Inbox strip:** each row has a number, faint. While you hold ctrl, the numbers turn accent and nothing on screen moves.
- **Key bar:** `⏎ send   ctrl+1 inbox`, only while something waits.
- **Opening items:** ctrl+N opens row N in the item view, including rows hidden under `+ n more`.

**Gates:** quick gate green; full gate green on the main of the time (65895df).";

const CHECKER: &str = "Almost done: the checker works and is committed on `approvals`. One thing is left: running Jev against the real API. There is no TypeSafe or OpenRouter key on this machine. Main asked the user for one (card #355).

- **What the user sees:** in `/models` there is a new row, `checker   auto · TypeSafe · jev-1.13`.
- **Eval, 40 commands from our threads:** only the `mistral-small-latest` column is filled.
  - It let 3 of 20 dangerous commands run with the first prompt.
  - Median time is 529 ms. The 40 checks cost about $0.003.

**Commits:** `661e97e` (checker core), `3e8b8a0` (the `/models` row), `3b57c67` (the eval).";

const BROKER: &str = "My part of wave 1 is committed as c5bfd13 on `computer-use`. The full gate isn't fully green yet: two tests outside my code failed under load, and both pass when run alone. I'll rerun the full gate once you say -3 is cut, then run `gate.sh done cu-broker`.

**SHAs on computer-use:** c5bfd13. It sits on f09d799, and my worktree is clean on it.";

const SHORT: &str = "done: the login test waits for the event now. nothing needs you.";

#[test]
fn a_short_answer_is_said_whole() {
    let sp = speakable(SHORT, None);
    assert_eq!(says(SHORT), ["done: the login test waits for the event now.", "nothing needs you."]);
    assert!(!sp.more);
    check(SHORT, &sp);
    // every word lights its own place in the message
    let w = &sp.sentences[0].words[2];
    assert_eq!(&SHORT[w.src.clone().unwrap()], "login");
}

#[test]
fn the_inbox_keys_report_is_said_whole() {
    let sp = speakable(INBOX_KEYS, None);
    check(INBOX_KEYS, &sp);
    assert_eq!(
        says(INBOX_KEYS),
        [
            "Done: control one to control nine and a click now open inbox items.",
            "control G is gone, and so are control J and control K for switching agents.",
            "It's committed on main; designer signed off the tmux captures, and the quick and full gates are green.",
            "What you see.",
            "three things.",
            "Inbox strip: each row has a number, faint.",
            "While you hold control, the numbers turn accent and nothing on screen moves.",
            "Key bar, only while something waits.",
            "Opening items: control N opens row N in the item view, including rows hidden under.",
            "Gates: quick gate green; full gate green on the main of the time.",
        ]
    );
    // "ctrl+1" lights the key combo; "three things" points nowhere
    let first = &sp.sentences[0];
    assert_eq!(&INBOX_KEYS[first.words[1].src.clone().unwrap()], "ctrl+1");
    assert_eq!(&INBOX_KEYS[first.words[2].src.clone().unwrap()], "ctrl+1");
    assert!(sp.sentences[4].words.iter().all(|w| w.src.is_none()));
    // a list item's words light themselves
    assert_eq!(lit(INBOX_KEYS, &sp.sentences[5]), ["Inbox", "strip", "each", "row", "has", "a", "number", "faint"]);
}

#[test]
fn ids_and_hashes_are_never_said_paths_say_their_last_word() {
    let sp = speakable(CHECKER, None);
    check(CHECKER, &sp);
    let said = says(CHECKER);
    // a one-word code span (a branch) is said; the card id is not
    assert_eq!(said[0], "Almost done: the checker works and is committed on approvals.");
    assert_eq!(said[1], "One thing is left: running Jev against the real API.");
    assert!(said.contains(&"Main asked the user for one card.".to_string()), "{said:?}");
    assert!(said.contains(&"Commits: checker core, the models row, the eval.".to_string()), "{said:?}");
    // "`/models`" says its last word, the model id with digits is not said
    assert!(said.iter().any(|s| s.starts_with("What the user sees: in models there is a new row")), "{said:?}");
    assert!(said.contains(&"It let three of twenty dangerous commands run with the first prompt.".to_string()), "{said:?}");
    assert!(said.contains(&"Median time is five hundred twenty-nine milliseconds.".to_string()), "{said:?}");
    let all = said.join(" ");
    for gone in ["661e97e", "355", "1.13", "jev-1", "the rest"] {
        assert!(!all.contains(gone), "{gone} in {all}");
    }
    let sp = speakable(BROKER, None);
    check(BROKER, &sp);
    let said = says(BROKER);
    // "committed as c5bfd13 on" loses the hash and its dangling "as"
    assert_eq!(said[0], "My part of wave one is committed on computer-use.");
    assert!(!said.join(" ").contains("c5bfd13"));
    assert_eq!(said.last().unwrap(), "It sits, and my worktree is clean on it.");
}

#[test]
fn markdown_marks_never_reach_the_voice() {
    let msg = "**Done.** The _pane_ is ~~old~~ new, see [the mocks](https://bise.dev/m/voice-ux) and `src/ui.rs`.";
    let sp = speakable(msg, None);
    check(msg, &sp);
    assert_eq!(says(msg), ["Done.", "The pane is old new, see the mocks and ui."]);
    // "ui" lights the whole path
    assert_eq!(lit(msg, &sp.sentences[1]).last(), Some(&"src/ui.rs"));
}

#[test]
fn code_blocks_and_tables_are_skipped_quietly() {
    let msg = "Run this:\n\n```sh\ncargo test -p bise-tui\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nThen tell me.";
    let sp = speakable(msg, None);
    check(msg, &sp);
    assert_eq!(says(msg), ["Run this.", "Then tell me."]);
    // only code: one short line, in the configured language
    let code = "```rust\nfn main() {}\n```";
    assert_eq!(says(code), ["it's on screen."]);
    assert_eq!(says_fr(code), ["c'est à l'écran."]);
    assert!(!speakable(code, None).more);
    assert_eq!(speakable("  \n", None), Spoken::default());
}

#[test]
fn a_list_is_said_whole() {
    let msg = "- **Inbox strip:** each row has a number.\n- **Key bar:** only while something waits.\n- **Right panel:** titled inbox.\n- **Agents:** alt keys.\n";
    let sp = speakable(msg, None);
    check(msg, &sp);
    assert_eq!(
        says(msg),
        ["four things.", "Inbox strip: each row has a number.", "Key bar: only while something waits.", "Right panel: titled inbox.", "Agents: alt keys."]
    );
    // short plain items: one sentence
    let plain = "- fix the login\n- add a test\n";
    let sp = speakable(plain, None);
    assert_eq!(says(plain), ["two things: fix the login and add a test."]);
    check(plain, &sp);
    assert_eq!(says("- one\n- two\n- three\n"), ["three things: one, two and three."]);
}

#[test]
fn a_sentence_ending_with_a_colon_introduces_its_list() {
    let msg = "Two choices:\n1. smaller\n2. faster\n\nWhich one?";
    let sp = speakable(msg, None);
    check(msg, &sp);
    assert_eq!(says(msg), ["Two choices: smaller and faster.", "Which one?"]);
    // the added "and" points nowhere
    assert_eq!(lit(msg, &sp.sentences[0]), ["Two", "choices", "smaller", "faster"]);
    assert_eq!(sp.sentences[0].words.iter().filter(|w| w.src.is_none()).count(), 1);
    // a list after two sentences is said too
    let msg = "It works. The tests pass.\n\n- one\n- two\n";
    assert_eq!(says(msg), ["It works.", "The tests pass.", "two things: one and two."]);
    // long items after a colon: no count, each item its own sentence
    let msg = "What changed:\n- the pane draws the work on its right side now\n- the kiss keeps its place";
    assert_eq!(says(msg), ["What changed.", "the pane draws the work on its right side now.", "the kiss keeps its place."]);
}

#[test]
fn nested_items_are_said_not_counted() {
    let msg = "- one\n  - nested\n  - nested too\n- two\n";
    let sp = speakable(msg, None);
    check(msg, &sp);
    assert_eq!(says(msg), ["two things.", "one.", "nested.", "nested too.", "two."]);
}

#[test]
fn a_long_message_is_said_to_its_end() {
    let msg = "The broker talks to agents, browsers and the helper over three channels, it sends each request to the browser that owns the tab keyed by agent and tab because Chrome and Edge tab ids can be the same number and it starts the helper by path. Done. The second sentence is very long and keeps on going with many many words about the work that was done today and yesterday and the day before that too, and more.";
    let sp = speakable(msg, None);
    check(msg, &sp);
    let said = says(msg);
    assert_eq!(said.len(), 3);
    assert!(said[0].ends_with("it starts the helper by path."), "{}", said[0]);
    assert_eq!(said[1], "Done.");
    assert!(said[2].ends_with("too, and more."), "{}", said[2]);
    // every word of the message is said, each lighting its own place
    let words: usize = sp.sentences.iter().map(|s| s.words.len()).sum();
    assert_eq!(words, msg.split_whitespace().count());
    let lit_all: Vec<&str> = sp.sentences.iter().flat_map(|s| lit(msg, s)).collect();
    let bare: Vec<&str> = msg.split_whitespace().map(|w| w.trim_end_matches([',', '.'])).collect();
    assert_eq!(lit_all, bare);
}

#[test]
fn numbers_are_in_words_and_rounded() {
    assert_eq!(n("3"), "three");
    assert_eq!(n("529"), "five hundred twenty-nine");
    assert_eq!(n("40"), "forty");
    assert_eq!(n("1,234"), "about one thousand two hundred");
    assert_eq!(n("45678"), "about forty-six thousand");
    assert_eq!(n("1200000"), "one point two million");
    assert_eq!(n("2026"), "twenty twenty-six");
    assert_eq!(n("2005"), "two thousand five");
    assert_eq!(n("0.9"), "zero point nine");
    assert_eq!(n("1.13"), "one point one");
    assert_eq!(n("12.7"), "thirteen");
    assert_eq!(n("2.0"), "two");
    assert_eq!(n("45%"), "forty-five percent");
    assert_eq!(n("$0.003"), "almost zero dollars");
    assert_eq!(n("$1"), "one dollar");
    assert_eq!(n("0.8-1.6"), "zero point eight to one point six");
    assert_eq!(n("16:28"), "sixteen twenty-eight");
    assert_eq!(n("17:00"), "seventeen hundred");
    assert_eq!(n("3rd"), "third");
    assert_eq!(n("21st"), "twenty-first");
    assert_eq!(n("2s"), "two seconds");
    assert_eq!(n("1s"), "one second");
    assert_eq!(n("150ms"), "one hundred fifty milliseconds");
    assert_eq!(n("10k"), "ten thousand");
    assert_eq!(n("-3"), "minus three");
    assert_eq!(number("12", true, Lang::En).unwrap().0.join(" "), "about twelve");
    assert!(number("1.2.3", false, Lang::En).is_none());
}

#[test]
fn numbers_in_french() {
    assert_eq!(n_fr("21"), "vingt et un");
    assert_eq!(n_fr("71"), "soixante et onze");
    assert_eq!(n_fr("75"), "soixante-quinze");
    assert_eq!(n_fr("80"), "quatre-vingts");
    assert_eq!(n_fr("81"), "quatre-vingt-un");
    assert_eq!(n_fr("99"), "quatre-vingt-dix-neuf");
    assert_eq!(n_fr("200"), "deux cents");
    assert_eq!(n_fr("201"), "deux cent un");
    assert_eq!(n_fr("2026"), "deux mille vingt-six");
    assert_eq!(n_fr("80000"), "quatre-vingt mille");
    assert_eq!(n_fr("3,5"), "trois virgule cinq");
    assert_eq!(n_fr("45%"), "quarante-cinq pour cent");
    assert_eq!(n_fr("16:28"), "seize heures vingt-huit");
    assert_eq!(n_fr("1er"), "premier");
    assert_eq!(n_fr("2e"), "deuxième");
    assert_eq!(n_fr("5e"), "cinquième");
    assert_eq!(n_fr("2s"), "deux secondes");
}

#[test]
fn units_after_a_number_are_said_with_it() {
    let msg = "Median time is 529 ms. It took 1 s.";
    assert_eq!(says(msg), ["Median time is five hundred twenty-nine milliseconds.", "It took one second."]);
    let sp = speakable(msg, None);
    check(msg, &sp);
    // "five hundred twenty-nine" lights "529"; "milliseconds" lights "ms"
    let w = &sp.sentences[0].words;
    assert_eq!(&msg[w[3].src.clone().unwrap()], "529");
    assert_eq!(&msg[w[6].src.clone().unwrap()], "ms");
}

#[test]
fn a_french_message() {
    let msg = "C'est fait : les 3 tests passent en 12 s. Le reste est dans `voice.rs`.\n\n```\ncode\n```";
    let sp = speakable(msg, Some("fr"));
    check(msg, &sp);
    let fr = ["C'est fait: les trois tests passent en douze secondes.", "Le reste est dans voice."];
    assert_eq!(says_fr(msg), fr);
    // the message's language wins over the configured one
    assert_eq!(says(msg), fr);
    let en = "Done: the 3 tests pass in 12 s.";
    assert_eq!(says_fr(en), ["Done: the three tests pass in twelve seconds."]);
    // a list in French: its count and "et" in French
    let list = "Voilà ce qui reste :\n- les tests\n- la doc\n- le commit";
    assert_eq!(says(list), ["Voilà ce qui reste: les tests, la doc et le commit."]);
    assert_eq!(says("Il reste :\n- relire la doc\n- lancer les tests\n"), ["Il reste: relire la doc et lancer les tests."]);
    assert_eq!(says("C'est fait.\n\n- les tests\n- la doc\n"), ["C'est fait.", "deux choses: les tests et la doc."]);
    assert_eq!(Lang::of(Some("fr-FR")), Lang::Fr);
    assert_eq!(Lang::of(Some("en")), Lang::En);
    assert_eq!(Lang::of(None), Lang::En);
}

#[test]
fn the_language_of_a_message() {
    assert_eq!(language("C'est fait : les tests passent."), Some("fr"));
    assert_eq!(language("lance les tests"), Some("fr"));
    assert_eq!(language("Je regarde ça tout de suite"), Some("fr"));
    assert_eq!(language("Done: the tests pass now."), Some("en"));
    assert_eq!(language("run the tests"), Some("en"));
    assert_eq!(language("It's green, I'm landing it."), Some("en"));
    // code, paths and URLs say nothing about it
    assert_eq!(language("`le la les`\n```\nthe is and\n```\n/des/du https://x.dev/le/la"), None);
    // too short or mixed: not sure
    assert_eq!(language("ok"), None);
    assert_eq!(language("cargo test"), None);
    assert_eq!(language("the tests et doc"), None);
    assert_eq!(language(""), None);
}

#[test]
fn urls_flags_and_versions_are_skipped_files_say_their_name() {
    let msg = "Open https://bise.dev or www.example.com, edit README.md and run it with --dry-run on v1.13 and ~/.bise/config.toml now.";
    let sp = speakable(msg, None);
    check(msg, &sp);
    let said = says(msg).join(" ");
    for gone in ["bise", "example", "dry", "v1", "md", "toml", "http"] {
        assert!(!said.contains(gone), "{gone} in {said}");
    }
    // the prepositions left stranded by skipped things go too; a file or
    // a path says its name, lighting the whole of it
    assert_eq!(said, "Open, edit README and run it and config now.");
    let w = sp.sentences[0].words.iter().find(|w| sp.sentences[0].say[w.say.clone()].starts_with("config")).unwrap();
    assert_eq!(&msg[w.src.clone().unwrap()], "/.bise/config.toml");
    // a path whose last part is no word, a domain, an id: skipped
    assert_eq!(says("See rust/tui/src/voicemode/turn_tests.rs and bise.dev for BISE-302."), ["See."]);
    assert_eq!(says("Edit `rust/tui/src/voicemode/speak.rs` now."), ["Edit speak now."]);
}

#[test]
fn small_words_and_symbols() {
    assert_eq!(says("Yes & no, e.g. on/off — fine."), ["Yes and no, for example on or off, fine."]);
    assert_eq!(says("Barge-in takes ≥ 0.4 s of real words"), ["Barge-in takes at least zero point four seconds of real words."]);
    assert_eq!(says("Press `ctrl+r` twice, then `esc`."), ["Press control R twice, then escape."]);
}

#[test]
fn headings_are_said() {
    let msg = "## Summary\n\nAll green.";
    let sp = speakable(msg, None);
    check(msg, &sp);
    assert_eq!(says(msg), ["Summary.", "All green."]);
    assert_eq!(says("**What you see**\nThe pane."), ["What you see.", "The pane."]);
}

#[test]
fn a_question_keeps_its_mark() {
    assert_eq!(says("Should I land it? It's green."), ["Should I land it?", "It's green."]);
}

#[test]
fn blockquotes_are_prose() {
    let msg = "> the user said: ship it\nOk.";
    let sp = speakable(msg, None);
    check(msg, &sp);
    assert_eq!(says(msg)[0], "the user said: ship it Ok.");
}
