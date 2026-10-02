# voice to voice: design proposal

Status: being built on the branch voice-to-voice (plan: docs/voice-mode-plan.md). Round 1 reviewed by designer; round 2 = the user's notes (§0), the pick is E, voice mode; round 3 = the user's notes after trying it (§0.1, wins over anything below). Mocks (local only): `site/content/voice-ux.html`
on localhost:4747 (four directions, an animation lab, dark/light, NO_COLOR, 80/100/150 columns).

You talk to main, main talks back. The agents keep working; the thread keeps the words.

## 0. Round 2: voice mode (E), the current pick

The user's notes on round 1: the big talking kiss (D's speaking) and "mouth + arcs" are the
favourites; doubt that hands-free turn-taking works (models misjudge when you're done); D dies,
but it should keep a minified history; the pick should put the composer at half the screen,
D's big kiss inside it, the thread above.

- **Name and keys:** *voice mode*. ctrl+r twice enters (ctrl+r alone stays dictation), esc
  leaves. Header `● voice mode 2:14`; divider `you ⇄ main · voice mode · headphones`; the
  transcript lines `· voice mode · 14:02 …` / `· voice mode ended · 7 min · …`.
- **The pane:** the composer grows to half the screen (12 rows at 100 × 40). Left: the big
  kiss, 41 × 9 cells in half blocks (round 5, the user: at rest the face smiles, sideways like
  the logo, so `:)`, and breathes; you talking or cutting in: the smile held a little rounder;
  thinking: the `*` comes back and turns; speaking: the mouth opens with the level and up to 3
  arcs `)))` open beside it; when main's voice ends on its own, a 1 s kiss: the lips press,
  pucker, the `:*` blows a small `*` up and away, the smile again; the eyes blink every 4.2 s,
  never while it speaks or kisses; reduced motion: the still smile, no kiss). Right: who talks (`you` in accent, `:* main`), then the words as they're said
  (≤ 28 cells a line, so 80 columns fit), then a status row. The thread stays above.
- **While main works:** the kiss turns its `*` and the status row says `∿ main is on it`; the
  minified work shows beside it (§0.1, the user brought it back after trying it).
- **What main says is in the thread too (round 3, the user):** the message lands whole in the
  thread at once; its spoken words light up there in step with the voice, the same words as
  beside the kiss; the shown-only details sit under it as usual.
- **Who talks when (the user's doubt):** the screen never guesses in silence. When you stop,
  `about to answer ●●●··` fills in ~1.2 s; talking again empties it; space sends at once;
  holding space keeps the floor while you think. Backchannels ("mm", "ok", "right") never cut
  main off: barge-in takes ~0.4 s of real words.
- **Any agent:** voice mode talks with the agent in view (`you ⇄ cookies`); one brand voice for
  now; the name over the words says who answers; the kiss stays (it's bise talking).
- **Approvals:** by voice ("allow") and by key, both always on.
- **Build:** all at once, no sequencing (the user).
- **`/voice`** is the one voice screen, dictation included (§0.1); ctrl+r twice is the way in.
- **Under 30 rows** the big kiss shrinks to B's two lanes (you, main) in a 4-row pane.
- **The brand voice:** to pick later, by ear.

### 0.1 Round 3: the user's notes after trying it (699f710)

He used it, on speakers and in French. What changes:

- **Transcribe 3, never Voxtral Mini.** Voxtral Mini misjudged the end of a turn and misheard
  words. Speech to text is `mistral/voxtral-transcribe-3` by default, for dictation and voice
  mode. Voxtral Mini is not offered anywhere (an old config that names it reads as Transcribe 3,
  no error). Voice mode transcribes each turn with that batch model; the end of the turn comes
  from voice activity (the `about to answer ●●●··` fill, space sends at once).
- **One `/voice` screen.** Setup and settings were two places; now one screen, the /models
  layout, top to bottom: dictation (on/off), speech to text (provider · model, Transcribe 3
  first), voice (Mistral's voices: name · language · gender, `▸ hear it` on press only),
  language (auto, or one: it feeds the transcription and what is said), listen, read aloud,
  sounds, who hears you. `/voice` is one row of the `/` list, no argument popup; `/voice setup`
  typed and `/models`' voice row open the same screen on speech to text.
- **The work beside the kiss.** While main works, talks or you hold the floor, a column right of
  the captions (150 columns: up to 44 cells, up to 9 rows, the newest on the status row) shows
  the minified work since your last message, one row each:
  `∿ bash npm test -- signup` running (text color), `· read src/signup/Hero.tsx ✓` done (dim,
  `✓` faint), `· bash npm test -- signup ✗` failed (dim, only `✗` in the error color),
  `· thinking` in italics, a message as `· text`; cut with `…`. While you talk or cut in, it
  hides: the pane is yours. At 95 and 80 columns there is no room beside the captions: while main
  works, its last 6 rows take the captions' place. Under 30 rows (the lanes), nothing.
  `what it does shows in the thread above` only shows where the column can't.
- **Main says the whole message.** No more "the rest is on screen": he can't always read. The
  message is said whole, sentence by sentence; code blocks, tables, URLs and hashes are skipped
  quietly, a path or an id is said as its last word or skipped, lists are said whole. Space cuts
  it at any time. Every word bise adds, every number and the quick "on it" are in the message's
  language (the one you spoke for "on it"): a French message is said in French.
- **ctrl+c leaves voice mode**, like esc. It never quits bise from voice mode.
- **It doesn't hear itself.** On speakers it took its own voice for him and answered itself. The
  echo is cancelled at the source (macOS VoiceProcessingIO: mic and speaker in one unit); bise
  also drops what it hears while it talks, or within 3 s after, when most of the words are the
  ones it just said. Without echo cancelling, cutting in needs headphones, and the mic stays shut
  while the voice plays (and 500 ms after) unless the route is headphones. When bise can't tell
  speakers from headphones, it treats them as speakers.

Round 1 follows, kept for the record; where it says "call", read "voice mode". Where it says
"the rest is on screen", Voxtral Realtime or `/talk settings`, §0.1 replaces it.

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
  the main lane (text color; main's `:*` there is its mouth, `:*` ↔ `:o` only: `:O` reads as shock), the keys row.
  The `:*` in the header and the panel never changes: it's the logo. The bar breathes in one
  slow ~2 s ease, accent → rule, while it listens. tab types (the call stays, the mic pauses).
- **Captions:** what you say builds live in the thread under a dim bar (not sent yet), partial
  words dim. Sent at the end of your turn (semantic turn detection, ~0.6 s), then `said ✓✓`.
- **Main's answer:** written whole at once; the spoken part highlighted word by word.
- **Speakers:** when the output is not headphones, the call switches itself to hold-space
  (A): main would hear its own voice and stop. The divider says `headphones` / `speakers`.
- **States and their cells:** listening `●` blink + your lane; thinking: the gust on main's
  lane (the gust: the brand's wind); speaking: mouth + main's lane + `)))`;
  cut in: main's lane drops within ~200 ms, its sentence ends `—` + faint `you cut in`;
  muted: `○ muted` in the top bar and the lane, flat and faint.
- **Header:** `● on a call 2:14` while the mic is open, in every view (also agents' views); in
  accent, never in the error color (a call isn't an error). Muted: `○` faint.
- **Waves:** blocks `▁▂▃▄▅▆▇█` for both lanes, the dictation chip's language. No braille
  (font-dependent, mush when small), no dots (too quiet).
- **Motion budget:** cells change in place at ~10 fps; nothing bounces or slides. Accent = you,
  text = main, dim = main at work, faint = nobody. `BISE_REDUCE_MOTION`: still frames (the `●`
  stays lit, the lanes freeze). `NO_COLOR`: bold/dim only; the breathing bar becomes bold/dim.
  ASCII: the chip's `_ . - = #` levels, `*` for `●`.
- **80 columns:** no panel, lanes 40 cells, the keys row drops `tab`.

## 4. What is said aloud, what is shown

One message, two depths: the first one or two sentences (~12 s) are said, the rest is shown.
Never read code, paths, ids, hashes, URLs, tables; numbers rounded and in words. Lists: how
many, then up to 3 items in 3 words. Only main speaks: an agent's message is told by main
("cookies asks…"). When there's more: "the rest is on screen". (Round 3: the whole message is
said, no "the rest is on screen", see §0.1.)

## 5. Inbox, news, transcript

- **Questions:** main reads the question and the numbered choices once. You answer like to a
  person ("1", "smaller", "the first one"). The item shows `heard "the first one" → 1 smaller`
  for 1.5 s before it counts (esc undoes). Unsure → main asks back. Keys still work.
- **Approvals:** "yes/ok/mm" never allow (backchannel words). Only the word "allow" or the
  key; "always allow" is key only.
- **News:** never over you, never in a pause you think in: after 2 s of silence, one
  sentence, after a soft click. Levels in `/talk settings`: what needs you + what you asked
  (default) / everything main would write / nothing. Several: "three things: … which first?"
- **Transcript:** a faint line opens and closes the call (`· call ended · 7 min · 4 things said,
  3 answers`: no clock time, the transcript has it); your messages carry `said`; no audio is kept.

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

## 8. Open questions (round 1; answered in §0)

"voice mode", not "call" · ctrl+r twice · approvals by voice and keys · one brand voice for now ·
voice mode with any agent · everything at once.
