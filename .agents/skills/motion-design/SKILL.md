---
name: motion-design
description: Make bise's motion design (launch films, feature clips, animated demos of the real UI, kinetic type, ASCII animations) the way Gabriel wants it and on brand. Use when asked for a video, an animation, a storyboard, a motion test, a hero film, a social clip, or animated UI demos for bise.
---

# Motion design for bise

What these notes come from: the launch hero film (12 scenes, 5 rounds of frame-by-frame review with Gabriel).
They hold what he asked for, what he rejected, and the techniques that worked. Follow them, and add to them
when a new review teaches you something.

## The brand on screen

- **Paper, ink, pen.** The background is paper `#f2ede2` with a light grain (an SVG turbulence noise at ~10%)
  and a soft vignette. Text is ink `#1d1a17`. The accent is the pen red `#c8264a`. On dark: cream `#ece6da`,
  and pink `#f4a6b0` for the accent. Full-frame color cards alternate paper / ink / pen.
- **The screens stay dark**, as figures laid on the paper: `#141211`, a 2–3 px ink outline, a flat offset
  shadow (`16px 18px 0 #1d1a1726`). No glow, no blur halo: a strong pink glow was rejected.
- **Type.** Newsreader (serif, 600) for every line of copy. JetBrains Mono for the UI, terminal lines and
  ASCII art. Caveat (handwritten) only as a light accent. For emphasis, prefer the serif word with a
  **hand-drawn underline** under it, drawn after the word has landed (a pen path with
  `stroke-dashoffset` animated from 1 to 0). Gabriel asked to switch Caveat words back to serif + underline
  several times (`relevant.`, `available.`, `expect.`, `goodbye`).
- **`:*` in Caveat:** the asterisk sits too high. Move it down (`translateY(.3em)`) next to the colon.
- **Voice:** lowercase except "I", short lines, no marketing words. The site's lines are the source
  (bise.dev, `site/index.html`): "a multi-agent harness, made for humans.", "you didn't become an
  engineer to babysit robots", "kiss your backlog goodbye".
- **The mascot vocabulary:**
  - the pixel kiss face from `rust/tui/src/voicemode/kiss.rs` (41×9 half blocks, sideways `:*`), with its
    poses (rest smile, listen, think, speak, kiss, blink, wink);
  - the site's pencil cloud (`site/index.html`, `.lcloud`);
  - sticky notes, pen tallies, the `✓✓` and `ψ` glyphs of the real UI.
  Build new characters from these. A dead face = the same pixel face, X eyes and a frown; a heart = a pixel
  sprite (9×8) that leaves the lips and floats up. A heart drawn inside the 41×9 grid read as a blob.

## Rules from the reviews

1. **Words and screens never share a frame.** A text card, then the UI, then a text card. Captions laid
   over the UI were rejected.
2. **Readable holds.** People watch with partial attention, so the text must stay up long enough. Rough minimums:
   - 0.8 s for one or two words;
   - 1.2–1.6 s for a line;
   - +0.4 s on the key lines ("a harness where multi-agent", "painless", "relevant").
   Short words can go faster only when that is the point (a staccato "try / bise / now!").
3. **Rhythm, not a metronome.** Equal shot lengths read as "robotic". Vary them on purpose:
   - start slow and speed up (popups: one every 500 ms, each gap ×0.885, ~4.5 s);
   - let a list accelerate, then stop hard on the payoff ("I manage …", then a long hold on "for you.");
   - keep one deliberate slow-down with no shake ("in flow", the breathing cloud).
4. **Never clip glyphs.** Mask the words with a `clip-path: inset(-1em -.6em -.28em -.6em)` that closes only
   at the bottom. Don't use `overflow:hidden`: it cut the `!` of "robots!" and "more!", and it broke the
   baseline between serif and Caveat words on the same line (an `overflow:hidden` inline-block takes its
   bottom edge as baseline).
5. **Causal order in UI demos.** On screen, always:
   1. your line lands in the thread;
   2. main answers;
   3. the spawn chips pop;
   4. the agents appear in the panel.

   Answers that showed up before the question was in the thread were the worst bug of round 4. Compute
   event times from the previous event (the typed line lands at `ts.end + 0.3`; the answer comes ≥ 0.35 s
   after, the chips ≥ 0.45 s after the answer).
6. **The whole UI must fit the frame** (1:1 is the launch format). Make the demo screen narrower and
   squarer instead of cropping it:
   - 60 columns with a compact agents panel (23 columns);
   - long thread lines wrap word by word (chips stay whole);
   - the height fits the content (`rows = thread lines + 7`), so there is no empty band between the
     thread and the composer.

   Widen the screen only when a key line must not wrap (70 columns for main's brief).
   For the first full view of the UI, Gabriel wanted it squarer and roomier still (72 × 24 cells, ~1.2:1):
   the history must not feel cramped. Then center the finished history vertically in its box by adding
   blank rows above it (a film-only cheat), and put a blank line between your ask and main's answer.
7. **Camera on the UI.** Define the camera by the width of screen it shows (`w` in screen px), not by a
   zoom factor, so a whole line or message is always in frame in every format.
   - The first time the UI appears: close on the input while it's typed, glide to the thread when it lands,
     to the agents panel while they pop, then end on the whole UI and hold ~1 s.
   - Elsewhere: start on the whole UI, then push in on the detail that matters (main's brief). Make sure the
     full detail is readable.
   - A long UI moment can be split around a text card: UI (your ask, main's answer, the chips), then
     "you talk, / I run the agents.", then back to the UI (the agents pop into the panel, zoom out). He
     preferred this to one long uninterrupted shot.
   - Close-ups must not cut text at the frame's edge. Frame them on a structural line: the history close-up
     stops at the divider, the panel close-up starts at it. Let the camera go past the screen's edge
     (paper shows) rather than clamp it back onto half a word.
   - A fast "whip" is fine from a text card into the UI. Between two UI moments, glide in one shot: a whip
     between two UI shots looked broken.
8. **Show what's new.** A new chip or agent row gets a pink flash plus a small scale pop (1.4 → 1 with a
   back ease), fading in 0.8 s.
9. **The film can exaggerate the UI to make the point readable** (e.g. in zen mode the film also dims the
   history; the real UI doesn't). Say so in the notes.
10. **Illustrations: simple or none.** Detailed SVG drawings (agents, envelopes, bubbles) were judged ugly
    and complex. What worked: one ASCII figure on a character grid that morphs from one concept to the next
    (letter → `?` → `✓` → progress bar → branch), with a left-to-right wipe and a noise character
    (`%#*+=-:`) at the switch. No words inside illustrations: nobody should have to read fast.
11. **Each moment needs a clear reading.** A fixed head ("I manage") with the changing word entering the
    same way as the head. Big text cards for features ("computer use", "voice mode") before their demo.
12. **Content facts.** Agent names and prompts stay consistent across all films (one world, `~/acme`:
    signup-perf, cookie-banner, pricing-page, login-test, empty-state, onboarding, changelog). Don't
    invent features or numbers; mark anything unsure "(to verify)".

## Easing

| use | curve |
|---|---|
| words and cards arriving | expo out |
| moves that start and stop (a branch, a cursor, a pan) | expo in-out / cubic in-out |
| pops (stickers, chips, ticks) | back out (a small overshoot) |
| camera glides | smoothstep |
| progress / download bars ("fast, ease-in") | expo in |
| hit on a cut | a 0.1 s exponential shake, scale 1.03 |

## How to build it

- **Code, not a timeline editor:** one HTML page per film; each shot is a pure function
  `render(localTime, duration) -> html`; a film is a sequence of shots (`seq([[dur, shot], …])`). This makes
  every frame reproducible, lets you freeze any time with `#<film>=<seconds>`, and later renders to MP4
  (Remotion or a headless frame dump).
- **One render, every format:** 1:1 (launch), 4:5, 9:16, 16:9 via `window.FRAME`, with sizes in units of
  the short side. Check at least 1:1 and 9:16.
- **A player for review:** play/pause, ±1 frame (30 fps), a full-width timeline with scene marks, keys
  (space, ← →, shift ± 1 s, [ ] scenes). Gabriel reviews frame by frame and gives timestamps.
- **The real UI, cell for cell:** draw the TUI from a character grid with the design-system colors
  (frame, thread, panel, divider, composer, key bar). Events (`say`, `lead`, `spawns`, `agent`, `done`,
  `zen`, `inbox`) drive the state at time `s`.
- **Check frames yourself before you show anything:** headless Chrome screenshots at frozen times, around
  every cut and in the middle of every move. Look for clipping, empty bands, wrong order, unreadable
  text. Then say exactly what you checked and what you did not (e.g. "~20 frozen frames, not full
  playback").

## Process with Gabriel

1. Storyboard first, as an interactive HTML page he can edit. If his notes are handwritten, transcribe them
   and confirm you read them right.
2. Explore art directions as short loops side by side, let him pick or mix, then build the film.
3. After each review, apply every numbered point, then answer point by point (what changed, where), give the
   new length, and name the weak spots you know about. Keep the previous version reachable.
4. Long films are fine while iterating. Tell him the length, and propose what to cut if he wants it shorter.
