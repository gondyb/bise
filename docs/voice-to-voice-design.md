# voice to voice: design proposal

Status: proposal for review, nothing built. Mocks (local only): `site/content/voice-ux.html`
on localhost:4747 (four directions, an animation lab, dark/light, NO_COLOR, 80/100/150 columns).

You talk to main, main talks back. The agents keep working; the thread keeps the words.

## 1. What exists

`rust/tui/src/voice.rs` + `voice/chip.rs`: ctrl+r records, any key stops, the whole clip goes
to the voice role's model (Voxtral by default, batch, BISE-130) and the words land in the
composer. The chip at the cursor: blinking `●`, the last 6 levels as `▁▂▃▄▅▆▇█`, a timer;
then a dim rolling wave while it transcribes. Missing for voice to voice: listening without a
key (turn detection), a voice out, being cut off, and rules for what main says aloud.

## 2. Directions

| | idea | gains | costs |
|---|---|---|---|
| A walkie-talkie | no mode: hold space on an empty composer, let go sends; said → spoken answer, typed → written | no false starts; works on speakers (no echo); smallest change | hands on the keyboard; cutting in is deliberate; holding needs the kitty keyboard protocol (else tap-tap) |
| B the call | `/talk` opens a call; the composer becomes two live lanes, you and main; `● on a call 2:14` in the top bar | real conversation; the lanes show who talks and the overlap; privacy visible in every view | needs headphones or echo cancelling; open mic = false starts; you see what was heard only after |
| C captions | voice is a composer state; your words appear live in the composer, main's are typed into the thread at speech speed | see what was heard as you talk; least new UI; 80 cols and NO_COLOR for free | little presence; text crawls at speech speed; the composer is busy |
| D the stage | the thread steps back; a big `:*` (34×9 cells, half blocks) breathes, turns, opens its mouth | readable from across the room; the brand moment | hides the thread; by far the most motion; useless under 30 rows |

## 3. The pick: the call, captioned (B + C + A)

- **Enter / leave:** `/talk` or ctrl+r twice (ctrl+r alone stays dictation). esc hangs up.
  The first `/talk` shows who hears you, once (§6).
- **The pane:** the composer's 4 rows become: the bar, the `you` lane (accent, your level),
  the main lane (text color; main's `:*` is its mouth, `:o` `:O` with the level), the keys row.
  The bar breathes accent → rule while it listens. tab types (the call stays, the mic pauses).
- **Captions:** what you say builds live in the thread under a dim bar (not sent yet), partial
  words dim. Sent at the end of your turn (semantic turn detection, ~0.6 s), then `said ✓✓`.
- **Main's answer:** written whole at once; the spoken part highlighted word by word.
- **Speakers:** when the output is not headphones, the call switches itself to hold-space
  (A): main would hear its own voice and stop. The divider says `headphones` / `speakers`.
- **States and their cells:** listening `●` blink + your lane; thinking: the gust on main's
  lane (`{g3} thinking`) or today's rolling wave; speaking: mouth + main's lane + `)))`;
  cut in: main's lane drops within ~200 ms, its sentence ends `—` + faint `you cut in`;
  muted: `○ muted` in the top bar and the lane, flat and faint.
- **Header:** `● on a call 2:14` while the mic is open, in every view (also agents' views).
- **Motion budget:** cells change in place at ~10 fps; nothing bounces or slides. Accent = you,
  text = main, dim = main at work, faint = nobody. `BISE_REDUCE_MOTION`: still frames (the `●`
  stays lit, the lanes freeze). `NO_COLOR`: bold/dim only; the breathing bar becomes bold/dim.
  ASCII: the chip's `_ . - = #` levels, `*` for `●`.
- **80 columns:** no panel, lanes 40 cells, the keys row drops `tab`.

## 4. What is said aloud, what is shown

One message, two depths: the first one or two sentences (~12 s) are said, the rest is shown.
Never read code, paths, ids, hashes, URLs, tables; numbers rounded and in words. Lists: how
many, then up to 3 items in 3 words. Only main speaks: an agent's message is told by main
("cookies asks…"). When there's more: "the rest is on screen".

## 5. Inbox, news, transcript

- **Questions:** main reads the question and the numbered choices once. You answer like to a
  person ("1", "smaller", "the first one"). The item shows `heard "the first one" → 1 smaller`
  for 1.5 s before it counts (esc undoes). Unsure → main asks back. Keys still work.
- **Approvals:** "yes/ok/mm" never allow (backchannel words). Only the word "allow" or the
  key; "always allow" is key only.
- **News:** never over you, never in a pause you think in: after 2 s of silence, one
  sentence, after a soft click. Levels in `/talk settings`: what needs you + what you asked
  (default) / everything main would write / nothing. Several: "three things: … which first?"
- **Transcript:** a faint line opens and closes the call (`· call ended · 14:09 · 7 min · 4
  things said, 3 answers`); your messages carry `said`; no audio is kept.

## 6. Settings and privacy

`/talk settings`, the /models layout: listen (hands-free with headphones, hold on speakers),
voice (provider · voice · ▸ hear it), speed (0.8-1.6×), read aloud, sounds, language, who hears
you. Default provider = the voice role's: one key, one company hears you. First `/talk`: who
hears you, what is kept (the words, never the audio), when it listens; `1 start the call · 2
hold-to-talk only · 3 not now`.

## 7. Tech, as constraints for the UI

| | cascade STT → main → TTS | speech-to-speech front | local |
|---|---|---|---|
| how | Voxtral Realtime (streaming, sub-second) → main's model and tools → Voxtral TTS sentence by sentence | gpt-realtime / Gemini Live talks; hands work to main as a tool | Voxtral Realtime weights or whisper.cpp; Kokoro / Piper |
| first sound | ~1.5-6 s (main's first sentence) | ~0.5-1 s | 2-8 s |
| who answers | main itself | a second mind | main |
| cutting in | bise stops playback, cancels the turn; needs AEC or headphones | built in | as cascade |
| who hears you | one provider (Mistral by default) | OpenAI / Google | nobody |

Recommendation for v1: the cascade on the voice role's provider, plus a spoken "on it" from
the small-jobs model (~0.3 s) so thinking never feels dead. cpal has no echo cancelling: on
macOS, VoiceProcessingIO; elsewhere headphones. Headphones vs speakers from the output device
(macOS); when unknown, ask once.

## 8. Open questions

1. The word: "call" (/talk, hang up) or "voice mode"?
2. The key: ctrl+r twice, or a new one?
3. Approvals by voice ("allow") or keys only?
4. One bise voice for everyone, or the user's pick?
5. Calls with an agent directly (you → cookies), or main only?
6. Build order: A first (hold-to-talk + spoken answers, smallest), then the call?
