# crossterm 0.28.1, patched for bise

Upstream: crossterm 0.28.1 from crates.io (the version ratatui 0.29 and
bend-tui use), unchanged except the diff below. `rust/Cargo.toml` puts it
in place with `[patch.crates-io]` and keeps it out of the workspace.
Removed from the crate: `examples/`, `docs/`, `CHANGELOG.md`, `Cargo.lock`
and the `[[example]]` entries of `Cargo.toml` (a `.gitignore` keeps the
lock the crate writes when tested alone out); added to `Cargo.toml`: a
`[lints.rust]` table (a path dependency is not lint-capped like a
registry one).

Why (the ctrl hints, `rust/tui/src/ctrlhint.rs`): ctrl pressed alone is
only reported with the kitty keyboard protocol's flag 8 (report all keys
as escape codes). With flag 8 a terminal sends every letter as `CSI
key-code u`, so the text a key typed through a dead key (option+e, e →
é), a macOS option character (option+a → å) or caps lock (a → A) is lost
unless flag 16 (report associated text) adds it: `CSI 101;;233 u`.
crossterm 0.28.1 and 0.29.0 do not parse that text (the flag is commented
out upstream). The patch uses it as the character typed.

Second patch (BISE-302, ctrl+1-9 open inbox items): tmux with
`extended-keys always` sends ctrl+digit in xterm's modifyOtherKeys form,
`CSI 27 ; modifiers ; key-code ~` (ctrl+1 = `CSI 27;5;49~`), which
crossterm rejects; `parse_csi_special_key_code` reads it as the key with
its modifiers (`test_bise_modify_other_keys_ctrl_digit`).

Tests: `cargo test --manifest-path rust/vendor/crossterm/Cargo.toml --lib
event::sys::unix::parse` (`test_bise_associated_text_is_the_character_typed`,
`test_bise_modify_other_keys_ctrl_digit`;
gate.sh runs them when this folder changes).

Dropping the patch: when crossterm parses the associated text itself,
remove this folder and the `[patch.crates-io]` entry.

## The diff (src/, CRLF line ends as upstream)

```diff
--- crossterm-0.28.1/src/event/sys/unix/parse.rs
+++ crossterm/src/event/sys/unix/parse.rs
@@ -606,6 +606,22 @@
         }
     }
 
+    // bise's patch (REPORT_ASSOCIATED_TEXT): the third field is the text
+    // the key produced (a dead key's accent, a macOS option character,
+    // caps lock's capital): one character is the one typed; a text other
+    // than the key itself consumed the option key.
+    if let (KeyCode::Char(key), Some(text)) = (keycode, split.next()) {
+        let mut chars = text.split(':').map(|c| c.parse::<u32>().ok().and_then(char::from_u32));
+        if let (Some(Some(c)), None) = (chars.next(), chars.next()) {
+            if !c.eq_ignore_ascii_case(&key) {
+                modifiers.remove(KeyModifiers::ALT);
+            }
+            // as without the protocol: an upper case letter has shift
+            modifiers.set(KeyModifiers::SHIFT, c.is_uppercase());
+            keycode = KeyCode::Char(c);
+        }
+    }
+
     let input_event = Event::Key(KeyEvent::new_with_kind_and_state(
         keycode,
         modifiers,
@@ -1502,5 +1518,40 @@
                 KeyEventKind::Release,
             )))),
         );
+    }
+    // bise's patch: REPORT_ALL_KEYS_AS_ESCAPE_CODES + REPORT_ASSOCIATED_TEXT
+    // as Ghostty sends them (an empty modifier field before the text)
+    fn key(seq: &[u8]) -> (KeyCode, KeyModifiers, KeyEventKind) {
+        match parse_csi_u_encoded_key_code(seq).unwrap() {
+            Some(InternalEvent::Event(Event::Key(k))) => (k.code, k.modifiers, k.kind),
+            e => panic!("{e:?}"),
+        }
+    }
+
+    #[test]
+    fn test_bise_associated_text_is_the_character_typed() {
+        let (none, shift, alt) = (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::ALT);
+        let press = KeyEventKind::Press;
+        // a dead key (option+e, then e): the key is e, the text é
+        assert_eq!(key(b"\x1B[101;;233u"), (KeyCode::Char('\u{e9}'), none, press));
+        // an option character (option+a on macOS): the text å took the option
+        assert_eq!(key(b"\x1B[97;3;229u"), (KeyCode::Char('\u{e5}'), none, press));
+        // caps lock (lock bit 64): A, with shift as without the protocol
+        assert_eq!(key(b"\x1B[97;65;65u"), (KeyCode::Char('A'), shift, press));
+        // shift+1 on a French layout: the key is &, the text 1
+        assert_eq!(key(b"\x1B[38;2;49u"), (KeyCode::Char('1'), none, press));
+        // option as alt: the text is the key, alt stays (the editor's dead keys)
+        assert_eq!(key(b"\x1B[101;3;101u"), (KeyCode::Char('e'), alt, press));
+        // no text: the key, as before
+        assert_eq!(key(b"\x1B[97u"), (KeyCode::Char('a'), none, press));
+        assert_eq!(key(b"\x1B[99;5u"), (KeyCode::Char('c'), KeyModifiers::CONTROL, press));
+        // a release carries no text
+        assert_eq!(key(b"\x1B[101;1:3u"), (KeyCode::Char('e'), none, KeyEventKind::Release));
+        // a modifier alone: left control pressed, then released
+        assert_eq!(
+            key(b"\x1B[57442;5u"),
+            (KeyCode::Modifier(ModifierKeyCode::LeftControl), KeyModifiers::CONTROL, press)
+        );
+        assert_eq!(key(b"\x1B[57442;1:3u").2, KeyEventKind::Release);
     }
 }
--- crossterm-0.28.1/src/event.rs
+++ crossterm/src/event.rs
@@ -301,10 +301,9 @@
         /// Represent all keyboard events as CSI-u sequences. This is required to get repeat/release
         /// events for plain-text keys.
         const REPORT_ALL_KEYS_AS_ESCAPE_CODES = 0b0000_1000;
-        // Send the Unicode codepoint as well as the keycode.
-        //
-        // *Note*: this is not yet supported by crossterm.
-        // const REPORT_ASSOCIATED_TEXT = 0b0001_0000;
+        /// Send the text a key produces with its keycode (bise's patch: the
+        /// text becomes the `KeyCode::Char`, see `parse_csi_u_encoded_key_code`).
+        const REPORT_ASSOCIATED_TEXT = 0b0001_0000;
     }
 }
 
--- crossterm-0.28.1/src/event/sys/unix/parse.rs (second patch, BISE-302)
+++ crossterm/src/event/sys/unix/parse.rs
@@ -638,6 +638,33 @@ pub(crate) fn parse_csi_special_key_code(buffer: &[u8]) -> io::Result<Option<Int
 
     let s = std::str::from_utf8(&buffer[2..buffer.len() - 1])
         .map_err(|_| could_not_parse_event_error())?;
+
+    // bise's patch (BISE-302): xterm's modifyOtherKeys form, what tmux
+    // sends with `extended-keys always` (its default format, xterm):
+    // `CSI 27 ; modifiers ; key-code ~`, e.g. ctrl+1 = `CSI 27;5;49~`.
+    if let Some(rest) = s.strip_prefix("27;") {
+        let mut f = rest.split(';');
+        let mask = f.next().and_then(|m| m.parse::<u8>().ok());
+        let code = f.next().and_then(|c| c.parse::<u32>().ok());
+        if let (Some(mask), Some(code), None) = (mask, code, f.next()) {
+            let keycode = match code {
+                9 => KeyCode::Tab,
+                13 => KeyCode::Enter,
+                27 => KeyCode::Esc,
+                127 => KeyCode::Backspace,
+                c => KeyCode::Char(char::from_u32(c).ok_or_else(could_not_parse_event_error)?),
+            };
+            let input_event = Event::Key(KeyEvent::new_with_kind_and_state(
+                keycode,
+                parse_modifiers(mask),
+                KeyEventKind::Press,
+                parse_modifiers_to_state(mask),
+            ));
+            return Ok(Some(InternalEvent::Event(input_event)));
+        }
+        return Err(could_not_parse_event_error());
+    }
+
     let mut split = s.split(';');
 
     // This CSI sequence can be a list of semicolon-separated numbers.
@@ -1528,6 +1555,20 @@ mod tests {
         }
     }
 
+    #[test]
+    fn test_bise_modify_other_keys_ctrl_digit() {
+        // tmux with `extended-keys always`: ctrl+1, ctrl+shift+&
+        assert_eq!(
+            parse_event(b"\x1B[27;5;49~", false).unwrap(),
+            Some(InternalEvent::Event(Event::Key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::CONTROL))))
+        );
+        assert_eq!(
+            parse_event(b"\x1B[27;6;38~", false).unwrap(),
+            Some(InternalEvent::Event(Event::Key(KeyEvent::new(KeyCode::Char('&'), KeyModifiers::CONTROL | KeyModifiers::SHIFT))))
+        );
+        assert!(parse_event(b"\x1B[27;5~", false).is_err());
+    }
+
     #[test]
     fn test_bise_associated_text_is_the_character_typed() {
         let (none, shift, alt) = (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::ALT);
```
