# voice mode: build plan and contracts

Status: building, on the local branch `voice-to-voice` only (never main, never pushed). Lead: the
task `voice-mode`. Design: `docs/voice-to-voice-design.md` §0 (E, validated by the user); the mocks:
`site/content/voice-ux.html` on localhost:4747 (local only), pick E. Everything at once, no sequencing.

## 1. What we build (v1)

ctrl+r twice (within 400 ms) enters voice mode with the agent in view; esc leaves. The composer
becomes the voice pane (half the screen; under 30 rows: B's two lanes in 4 rows). You talk; when you
stop, `about to answer ●●●··` fills in 1.2 s and the turn is sent (space: now; hold space: keep the
floor; talking again empties it). A spoken "on it" (small-jobs model, else canned) covers the start.
The agent works as usual (its tool lines come into the thread above); its message lands in the thread
whole, its first sentence or two are said (Voxtral TTS, streamed), the said words light up in step in
the pane and in the thread. With headphones you cut in by talking (≥ 0.4 s of real words; "mm",
"ok", "right" never cut); on speakers the mic waits while the agent talks (hold space to talk).
Approvals: the word "allow" or the key. Questions: "1", "the first one", "smaller". `/voice` is voice
mode's settings. One default Voxtral voice (the brand voice is picked later, by ear).

Not in v1: echo cancelling on speakers (VoiceProcessingIO: stretch goal of voice-audio), news read
aloud while idle (design §5 "news": after v1), a local model, the brand voice.

The cascade: mic (16 kHz) → VAD + turn controller → listener (Voxtral Realtime websocket; else the
voice role's batch model per turn) → `sb` input to the agent in view (`voice: true`) → its message →
`speakable` → TTS (24 kHz PCM stream) → speaker, whose clock lights the words.

## 2. The pieces and who owns which file

One owner per file: never edit a file you do not own; ask its owner (or the lead for shared types).
All new code is under `rust/tui/src/voicemode/`; the contracts are `voicemode/mod.rs` (lead).

| agent | owns | delivers |
|---|---|---|
| **voice-mode** (lead) | `voicemode/mod.rs`, `turn.rs`, `fakes.rs`; `app.rs`, `input.rs`, `run.rs`, `sb.rs`, `lib.rs`, `rust/tui/Cargo.toml`, `rust/Cargo.lock`; the hub's `voice` flag (`rust/switchboard/…`); `docs/voice-mode-plan.md` | the turn controller and its tests, the wiring (keys, ticks, feed events → controller → acts), ctrl+r twice, hold space (kitty release events, else key repeat), the e2e test with fakes, integration, gates, the try build |
| **voice-audio** | `voicemode/audio.rs`, `vad.rs`, `route.rs`, `voicemode/testdata/` (recorded fixtures); `voice.rs` (only to share its cpal/resampler/meter parts) | `CpalMic`, `open_speaker()` (cpal output, queue of utterances, clock, ≤ 50 ms stop with a fade, level), `Vad`, `output_route()` (macOS headphones vs speakers); fixtures; stretch: VoiceProcessingIO on macOS |
| **voice-stt** | `voicemode/listen.rs` (+ `voicemode/ws.rs` if wanted); `voice/stt.rs` (only if the batch path needs it) | `RealtimeListener` (Voxtral Realtime websocket, from 0659158's protocol), `BatchListener` (one `voice::transcribe_clip` per Flush), `listener_for(job)` |
| **voice-tts** | `voicemode/speak.rs`, `timing.rs`, `tts.rs`, `ack.rs`; `voice/http.rs` (a streaming read, additive) | `speakable()` (design §4), word timings, `VoxtralTts` (SSE stream), `SmallAck` + `canned()`, `DEFAULT_VOICE` |
| **voice-tui** | `voicemode/kiss.rs`, `pane.rs`; `ui.rs`, `render.rs`, `feed.rs`, `chrome.rs`, `keybar.rs`, `ctrlhint.rs`, `layout.rs` (their voice mode parts) | the kiss animation, the pane (captions, status row, keys row), the lanes under 30 rows, the header `● voice mode 2:14`, the divider, the thread lighting (`Lit`), `said` / transcript lines' style, NO_COLOR / ASCII / reduce motion; the designer's sign-off |
| **voice-settings** | `voicemode/config.rs`, `answers.rs`, `voicemode/settings.rs` (new); `commands.rs` (`/voice`); `rust/catalog/src/voice.rs` (+ its tests); `sb/cards.rs` (the `heard "…" → 1` line only) | `[voice]` keys for voice mode in config.toml, the jobs (`listen_job`, `say_job`, `ack_job`), `/voice` = voice mode's settings screen (the /models layout), the first-time "who hears you" screen, answers by voice |

Shared files no piece owns: ask the lead. A new crate dependency: ask the lead (one lockfile change,
one gate seed for everybody).

## 3. How every agent works (the branch rules)

- Your worktree: `tests/gate.sh new <your name>`, run the `cd … && export CARGO_TARGET_DIR=…` it
  prints, then `git checkout --detach voice-to-voice`. Never work in the shared folder.
- Commit onto the branch only, never main, never push, never the shared index, no stash/reset/rebase:

  ```sh
  B=refs/heads/voice-to-voice; export GIT_INDEX_FILE=$TMPDIR/vm.index
  old=$(git rev-parse $B) && git read-tree $old
  git add -A -- <your owned paths>          # whole files: you own them
  new=$(git commit-tree $(git write-tree) -p $old -m "<long subject: what and why>")
  git update-ref $B $new $old    # CAS: on failure redo from read-tree
  unset GIT_INDEX_FILE && git checkout -q -m --detach $new       # after the unset, never before
  ```
  The checkout brings the others' work into your worktree; your owned files are unchanged by it.
  (Checked out with the private index still set, your worktree's index stays behind: `git
  read-tree HEAD` in your own worktree fixes it.)
- Gates: `tests/gate.sh quick` before each commit (green: clippy -D warnings + the tests). The lead
  runs the full gate (`ulimit -n 8192; tests/gate.sh full`) once at integration.
- The machine: the bash tool caps files at 50 MB: a release build or a big binary goes through
  `launchctl submit -l bise.<you>.N -o $TMPDIR/N.log -e $TMPDIR/N.log -- nice -n 10 …`, then
  `launchctl remove bise.<you>.N`. `df -h .` before a new target (under 8 GB free: wait). Node capped.
  `tests/gate.sh done <name>` at the end (it deletes your target/).
- Real audio: never open the mic or play a sound on the user's Mac. Tests use fakes and recorded
  audio (fixtures made with `say -o file.aiff …` + `afconvert`, which write a file and play nothing).
  Live API calls (Mistral STT/TTS with the user's key, resolved through `bise_catalog`) are allowed in
  `#[ignore]` tests run by hand; never print a key or the environment.
- Done: `sb report done "<SHAs, what works, what is not done>"` to main, `sb send voice-mode` the
  same. Blocked on a contract: `sb send voice-mode --expect-reply "…"`.

## 4. The contracts (`rust/tui/src/voicemode/mod.rs`, read it whole)

### 4.1 Audio in (voice-audio → lead)

`Mic::open(Sender<MicBlock>) -> Box<dyn MicStream>`: blocks of 20-100 ms, 16 kHz mono i16, with their
capture `Instant`; dropping the stream closes the device (the macOS mic dot goes off). `level()`: the
last block's loudness 0..1 (`voice::loudness`). `Vad::feed(&[i16]) -> bool`: speech in this block,
adaptive noise floor, ~150 ms hangover; tested on fixtures: speech, room noise, a keyboard, "mm".

### 4.2 Audio out (voice-audio → lead)

`Speaker`: `push(utt, &[f32])` at 24 kHz (resampled to the device), utterances back to back in push
order; `end(utt)`; `stop()` silent within 50 ms (10 ms fade), the queue dropped; `clock()` = (the
utt playing, how much of it played, from samples actually handed to the device); `done(utt)`;
`level()` of the output now (the mouth). `output_route()`: Headphones / Speakers / Unknown.

### 4.3 Speech to text (voice-stt → lead)

`Listener::start(job, Receiver<ListenMsg>, Sender<Heard>, cancel)` on its own thread, one session for
the whole voice mode. In: `Audio(pcm)` (only live mic audio), `Flush` (the turn ends), `Clear` (drop
the half turn). Out: `Text(words)` appended as is (they carry their leading space; realtime sends
them as you talk, ~0.5 s behind), `Flushed` once every word of the audio before the Flush is in
(realtime: commit/flush then wait for the deltas, 2 s at most), `Failed(line)`. Batch: collect the
audio since the last Flush, one `voice::transcribe_clip` at Flush, `Text` then `Flushed`; under
`MIN_CLIP` nothing is sent and `Flushed` comes at once. A dropped websocket reconnects once, quietly.

### 4.4 The turn controller (lead), the timings

States = `Phase` (mod.rs). Listening →(VAD speech) Hearing →(silence ≥ `PAUSE` 0.3 s) AboutToAnswer
{fill over `END_OF_TURN` 1.2 s} →(full, or space) Flush → (`Flushed`) `Act::Send` → Working (+ the
ack: the small-jobs line if it comes within `ACK_DEADLINE` 0.7 s, else `canned(n)`; skipped when the
agent's message already came) → (the agent's message) Speaking → Listening. Space held: Holding (no
fill). Speaking + your speech: with headphones, ≥ `BARGE_IN` 0.4 s and a heard word that is not in
`BACKCHANNELS`, or ≥ `BARGE_IN_ANYWAY` 1.2 s → `Speaker::stop`, the unsaid words `Cut`, `CutIn`,
`Act::Interrupt` when the agent's turn still runs, then Hearing; on speakers the mic is gated while
the agent talks (HoldToTalk: space held opens it, and cuts in). m mutes, tab types (the mic waits; ⏎
sends the typed text, back to listening), esc leaves (stops the speaker, drops the half turn, the
transcript line `· voice mode ended · 7 min · 4 things said, 3 answers`).

What is said: every assistant message of the agent in view that arrives after a voice turn and before
the next one, through `speakable`, sentence by sentence (one `UttId` per sentence: the TTS of
sentence n+1 starts when sentence n's is done), skipped when `read_aloud` is Nothing. Questions and
approvals that arrive in voice mode: the agent's message says them; the next turn is first matched by
`answers` (allow / a choice), shown `heard "…" → 1 smaller` 1.5 s (esc undoes), else sent as words.

### 4.5 What is said (voice-tts → lead)

`speakable(msg, language) -> Spoken`: design §4. The first one or two sentences, ≤ ~12 s (~30 words
at 1×); skip code blocks, inline code, paths, ids, hashes, URLs, tables; numbers rounded and in words
(English; French when `language` is fr); a list: how many, then up to 3 items in ≤ 3 words; `more`
and a last sentence "the rest is on screen." when something was left out. Each `Word` keeps its byte
range in the message (`src`) for the thread's lighting; added words have `src: None`. Markdown marks
(`**`, `_`, `#`) never reach `say`.

`timing::estimate(say, speed)` (before the audio is all in) and `said_upto(sentence, played, total)`:
the words share the audio by weight (syllables or letters, + a pause after `,` `.`), monotonic, the
last word lit exactly at the end. `VoxtralTts`: `POST {base}/audio/speech` streamed (SSE, base64 PCM
at 24 kHz; check the current docs: model `voxtral-mini-tts-2603`, the voice field, the stream flag),
`Synth::Audio` as chunks come, `Done`, `Failed(one line)`; cancel closes the socket. `SmallAck`: one
chat call to the small-jobs model ("say in ≤ 5 words that you're on it", in the language you spoke),
`canned(n)` otherwise.

### 4.6 The pane and the thread (lead → voice-tui)

The controller builds a `PaneView` each frame (`app.voice_mode.view()`); `pane::height(screen_h)`
gives the pane's rows (≥ 30 rows: half the screen; else 4: the lanes); `pane::draw(buf, area, view,
t_ms)` draws it, no state of its own (motion from `t_ms`, `BISE_REDUCE_MOTION` still frames). The
header `● voice mode 2:14` (accent; `○` faint when muted) in every view; the divider `you ⇄ <agent> ·
voice mode · headphones|speakers`. The thread: `app.voice_mode.lit() -> Option<Lit>`; the feed finds
the last assistant message of `lit.agent` with that text and styles the spans (Said / ToSay / Cut;
the designer picks the styles). Your said messages carry `said` (the lead marks them); the transcript
lines are plain faint notes (`Act::Note`). Captions: ≤ 28 cells a line (80 columns fit).

### 4.7 Settings (voice-settings → everyone)

`config::load() -> VoiceModeConfig` (listen Auto/HandsFree/Hold, tts model, voice, speed 0.8-1.6,
read aloud Needs/All/Nothing, sounds, language, seen_privacy) from `[voice]` in config.toml via
bise_catalog; `save`. Jobs with keys (never printed; `Endpoint`'s Debug hides the key):
`listen_job()` (realtime = `mistral/voxtral-mini-transcribe-realtime-2602` when the voice role's
provider is Mistral, else None; batch = the voice role's job), `say_job(&cfg)` (the TTS on the voice
role's provider: Mistral by default; another provider without a TTS: the one-line reason),
`ack_job()` (the small-jobs model). `/voice` = the settings screen in the /models layout: listen,
voice (provider · voice · ▸ hear it: plays only after the user presses it), speed, read aloud, sounds,
language, who hears you; ctrl+r twice stays the way in; dictation (one ctrl+r) keeps its on/off in
it. The first voice mode: who hears you, what is kept (the words, never the audio), when it listens;
`1 start voice mode · 2 hold-to-talk only · 3 not now`.

`answers::is_allow(heard)`: the word "allow" only. `answers::pick(heard, choices)`: "1"/"one"/"the
first one"/"premier"/a word of the choice's label; None when unsure.

## 5. Tests

- Each piece: unit tests in its file (pure parts) or next to it (`voicemode/<piece>_tests.rs`).
- The controller: scripted scenarios over the fakes (fake mic blocks from fixtures, a fake listener
  that echoes scripted words, a fake synth with scripted chunks, a fake speaker with a manual clock):
  a full turn, space sends, hold space, talking again empties the fill, "mm" never cuts, barge-in
  cuts within one tick, speakers gate the mic, esc mid-turn, approvals by voice, a listener failure.
- The pane: buffer snapshots at 150/95/80 columns × 40 and 29 rows, dark/light, NO_COLOR, ASCII.
- Live (`#[ignore]`, by hand, no sound): realtime STT of a fixture, one TTS sentence (checks the
  chunks decode, 24 kHz, duration in the expected range).
- The tmux e2e (`tests/`): voice mode with the fakes behind `BISE_VOICE_FAKE=1` (a scripted mic
  and a silent speaker): ctrl+r twice, a turn, the reply lit, esc.

## 6. Integration (lead)

The pieces land as they are green; the lead wires them (`app.voice_mode: VoiceMode`, run loop pump at
50 ms in voice mode, feed events), removes the module's `allow(dead_code)`, runs the full gate,
gets the designer's sign-off on every screen (captures 150/95/80, dark + light, NO_COLOR), and sends
main the SHA for a try build.

## 7. The helpers' briefs (main spawns them as written)

Common to all: read `docs/voice-to-voice-design.md` (§0 first) and this plan whole, then
`rust/tui/src/voicemode/mod.rs`; follow §3 to the letter; own only your files (§2); report to main
and tell `voice-mode` when done, with the SHAs on `voice-to-voice`.

**voice-audio.** Build voice mode's audio: `voicemode/audio.rs` (`CpalMic`: the default input, open
for the whole voice mode, 16 kHz mono blocks with their capture time, reusing `voice.rs`'s cpal
stream, resampler and meter (make them `pub(crate)` there, you own `voice.rs` for that); `open_speaker`:
cpal output, utterance queue, played-samples clock, `stop` silent within 50 ms with a fade, level),
`vad.rs` (adaptive-floor energy VAD + hangover, tested on fixtures), `route.rs` (macOS: the default
output's transport type and data source through coreaudio-sys: headphones, AirPods/Bluetooth,
USB headsets vs built-in speakers; else Unknown), and `voicemode/testdata/` (a few short 16 kHz WAVs
made with `say -o` + `afconvert`: a sentence, "mm", "ok", silence, room noise; keep each < 150 KB).
Never open the real mic or play sound: the cpal paths are compiled and unit-tested around the device
(queue, clock, fade over a fake output callback). Stretch, after the rest: echo cancelling on macOS
via a VoiceProcessingIO AudioUnit behind the same `Mic`/`Speaker` traits.

**voice-stt.** Build `voicemode/listen.rs`: `RealtimeListener` (Voxtral Realtime over a websocket,
tungstenite + rustls already in Cargo.toml; recover the protocol from `git show
0659158:rust/tui/src/voice.rs`: session.created/update, input_audio.append base64, flush/commit,
transcription.text.delta/done; check the current Mistral docs), `BatchListener` (collect the audio
between flushes, `voice::transcribe_clip` at Flush), `listener_for(job)`. Contract §4.3: `Text` as
you talk, `Flushed` after a Flush (2 s at most), `Clear`, one quiet reconnect, `Failed` with
`voice::fail_lines`' wording. Tests: the protocol's pure parts (messages built and parsed), a fake
websocket server on localhost for the session and the flush timing, the batch path with the voice
module's fakes; one `#[ignore]` live test on a fixture from voice-audio (or your own `say -o` file).

**voice-tts.** Build `voicemode/speak.rs` (`speakable`, design §4 and plan §4.5: the first sentence
or two, no code/paths/ids/hashes/URLs/tables, numbers in words (en, fr), lists as count + ≤ 3 items,
"the rest is on screen.", every word mapped back to its byte range in the message; many tests on
real agent messages: take some from `sb history`), `timing.rs` (estimate, `said_upto`), `tts.rs`
(`VoxtralTts`: streamed `POST /v1/audio/speech` per the current Mistral docs, SSE of base64 PCM at
24 kHz; pick `DEFAULT_VOICE`, a neutral default Voxtral voice, the brand voice comes later; add a
streaming read to `voice/http.rs`, which you own, without changing its batch use), `ack.rs`
(`SmallAck`: one short chat call to the small-jobs model, ≤ 5 words, the language you spoke; the
chat call: reuse how bise calls the small-jobs model elsewhere, or a minimal OpenAI-compatible
request over `voice/http.rs`). Tests: pure parts, the SSE parser on recorded chunks, a fake local
HTTP server for the stream and the cancel; one `#[ignore]` live test (no sound: decode and measure).

**voice-tui.** Build what voice mode looks like: `voicemode/kiss.rs` (the big kiss 41 × 9 in half
blocks, from the mocks' E: `site/content/voice-ux.py`/`.js` in the shared folder, local only:
breathing `*` listening, turning thinking, the mouth `:*` ↔ `:o` opening with the level + up to 3
arcs speaking, shut to a line when cut in, still frames under `BISE_REDUCE_MOTION`), `voicemode/pane.rs`
(`height`, `draw`: kiss left, captions right (`you` in accent, `:* main`, words ≤ 28 cells, partial
dim, said/to-say/cut), the status row (`about to answer ●●●··`, `main is on it`, `you cut in`,
`○ muted`, `hold space to talk`), the keys row; under 30 rows B's two lanes in 4 rows; 80 columns),
and in `ui.rs` (the pane in place of the composer when `app.voice_mode` is on, the lead adds the field
and a `pub fn view(&self) -> Option<PaneView>` you call), `chrome.rs` (header `● voice mode 2:14`),
the divider, `render.rs`/`feed.rs` (the thread lighting from `lit()`, `said` on your messages, the
faint transcript lines). NO_COLOR, ASCII, light theme. Until the lead's field lands, test through
`pane::draw` on a `Buffer` with hand-built `PaneView`s. Get the designer's sign-off on every screen
(captures 150/95/80 columns, 40 and 29 rows, dark + light, NO_COLOR): `sb send designer`.

**voice-settings.** Build voice mode's settings: `rust/catalog/src/voice.rs` (the `[voice]` keys
for voice mode: listen, tts model, voice, speed, read aloud, sounds, seen_privacy; the realtime model
of the voice role's provider; the TTS model per provider, Mistral only for now), `voicemode/config.rs`
(load/save, `listen_job`, `say_job`, `ack_job` with keys from the chat keys' resolution, as
`voice::resolve_job` does), `voicemode/settings.rs` + `commands.rs` (`/voice` opens voice mode's
settings screen in the /models layout; dictation's on/off stays reachable there; ▸ hear it plays
only on the user's press, and in tests never), the first-time "who hears you" screen (design §6),
`voicemode/answers.rs` (`is_allow`, `pick`, plan §4.7) and in `sb/cards.rs` the `heard "…" → 1
smaller` line for 1.5 s (esc undoes). Screens: the designer signs off (captures 150/95/80, dark +
light, NO_COLOR; `sb send designer`).

## 8. Round 2: the user's notes on 699f710

What he found, and the fix:

| # | the user | the fix | owner |
|---|---|---|---|
| 1 | hands-free end of turn works badly; Voxtral Mini isn't good enough; Transcribe 3 is much better: never use Voxtral Mini, don't even offer it | the voice role defaults to `mistral/voxtral-transcribe-3`; Voxtral Mini (batch `voxtral-mini-latest`, realtime `voxtral-mini-transcribe-realtime-2602`) leaves the catalog and every picker (an old config naming it reads as Transcribe 3); voice mode transcribes each turn with the voice role's batch model (VAD ends the turn) | voice-models |
| 2 | the model choice is in "setup", the rest in "settings": one menu | `/voice` is one screen: dictation on/off, the speech-to-text model, the voice, the language, listen, read aloud, sounds, who hears you; `/voice setup` opens the same screen | voice-settings2 |
| 3 | bring back the minified tool calls and thinking beside the kiss while bise works | `PaneView.work` (the controller fills it from the feed); the pane draws it on its right side | voice-tui2 (draw), lead (data) |
| 4 | never stop on "the rest is on screen" (he can't always read); read the whole message; space cuts it; the fixed English phrase breaks French | `speakable` says the whole message (code blocks, tables and URLs skipped quietly, paths and ids said as their last word or skipped), no added "the rest is on screen"; numbers and every added word in the message's language; the canned "on it" in the language you spoke | voice-tts2 |
| 5 | ctrl+c in voice mode should leave voice mode, like esc | ctrl+c = esc in voice mode | lead |
| 6 | on speakers it hears itself, takes it for him and answers itself | echo cancelled at the source (macOS VoiceProcessingIO: mic + speaker in one voice-processing unit) behind `audio::open_voice_io()`; the controller also drops what it hears that matches what the agent just said, and without AEC never lets the mic through while the agent talks unless the route is headphones | voice-echo (AEC), lead (filter, gating) |
| 7 | choose the voice (type of voice) and the language | the Mistral voices list (`GET /v1/audio/voices`: name, languages, gender) in `/voice`, ▸ hear it on press; the language row (auto, or one) feeds the transcription and `speakable` | voice-settings2 |

Same rules as §3 (branch only, own worktree, launchctl for gates: the bend-tui test binary is over
the bash tool's 50 MB cap, `launchctl submit` jobs restart when they exit: `launchctl remove` as soon
as the log has its end). New contracts in `mod.rs`: `PaneView.work: Vec<Work>` (`Work { kind:
Tool|Thinking|Message, text, state: Running|Done|Failed }`), `VoiceIo { mic, speaker, aec }` and
`audio::open_voice_io()` (a stub with `aec: false` until voice-echo lands).

### 8.1 Briefs, round 2 (main spawns them as written)

Common: read §8 and the files you own first; same rules as §3; tell `voice-mode` and main your SHAs.

**voice-models.** Owns `rust/catalog/models.toml` (the voice models), `rust/catalog/src/voice.rs`
(+ tests), `voicemode/listen.rs`, `voice/stt.rs`, `voice.rs` (dictation's texts only). Make
`mistral/voxtral-transcribe-3` the default voice model (dictation and voice mode); remove Voxtral
Mini from the catalog and from every list the user sees (`voxtral-mini-latest`, the realtime mini);
a config or env that names it resolves to Transcribe 3 with no error. `realtime_model` returns None
for every provider now (no realtime model we'd offer); `listener_for` gives the batch listener; keep
the realtime code compiled but unused only if it costs nothing, else remove it with its tests.
Check the Transcribe 3 request (language, context bias) against the current Mistral docs; one
`#[ignore]` live test on a fixture (nothing played).

**voice-echo.** Owns `voicemode/audio.rs`, `route.rs`, a new `voicemode/aec.rs`. Build
`open_voice_io()` on macOS with a VoiceProcessingIO AudioUnit (coreaudio-sys; input and output in one
unit, so the speaker's echo is removed from the mic), 16 kHz mono blocks out (as `CpalMic`), TTS PCM
in (as the speaker today, same `Speaker` contract: queue, clock, stop within 50 ms, level),
`aec: true`; any failure falls back to the plain devices with `aec: false`. Never open the real
devices in tests: the unit's render/input callbacks are tested through their pure parts; tell main
when a by-hand check with the user's mic is the only way left. Also: re-check `output_route()`
(the user's speakers may have read as headphones): when unsure, Unknown.

**voice-settings2.** Owns `voicemode/settings.rs`, `voicemode/config.rs`, a new
`voicemode/voices.rs`, `commands.rs` (`/voice`), `onboarding/*` only where `/voice setup` and the
dictation picker point to (make them open the one screen). One `/voice` screen in the /models
layout: dictation on/off, speech to text (provider · model: Transcribe 3 first; no Voxtral Mini),
voice (Mistral's voices from `GET {base}/audio/voices`, cached for the session; name · language ·
gender; ▸ hear it on press only), language (auto + the voices' languages; feeds the transcription
and what is said), listen, read aloud, sounds, who hears you. `config` saves `tts_voice` and
`language`; `say_job` uses the chosen voice. Designer sign-off on the screen (150/95/80, dark +
light, NO_COLOR).

**voice-tui2.** Owns `voicemode/pane.rs`, `kiss.rs`, `ui.rs` (voice parts) and their tests. Draw
`PaneView.work` on the pane's right side while the agent works and talks (the mocks' D minified
history: one row per tool call `· bash cargo test ✓`, thinking `· thinking…`, running ones lit,
failed ones in the error color, newest at the bottom, the last ones that fit); the kiss and the
captions keep their places; under 30 rows and at 80 columns nothing new (no room). Designer
sign-off (150/95/80, dark + light, NO_COLOR).

**voice-tts2.** Owns `voicemode/speak.rs`, `timing.rs`, `ack.rs`, `tts.rs` and their tests. The whole
message is said, sentence by sentence (code blocks, tables, URLs, hashes skipped quietly; a path or
an id said as its last word or skipped; lists said whole); no "the rest is on screen" (drop `more`'s
sentence); every word bise adds and every number in the message's language (detect it: a
`speak::language(text) -> Option<&'static str>` on the message, else the config's); `canned(n,
lang)` in English and French (the language you spoke: the heard text). The word mapping to the
message (`src`) must stay exact for the thread's lighting.
