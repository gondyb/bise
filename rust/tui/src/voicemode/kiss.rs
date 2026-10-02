//! The big talking kiss (owner: voice-tui; design §0, plan §2): 41 × 9
//! cells in half blocks (41 × 18 dots), a face drawn big: the colon on
//! the left, the mouth on the right. Round 5 (the user picked the
//! designer's ':) the smile'): at rest the face smiles, sideways like
//! the logo, so the smile is a `)`; the `*` is bise at work.
//!
//! - listening: the smile breathes (a little rounder every other
//!   [`BREATH_MS`]);
//! - you talk (cutting in too): the smile, a little rounder, held still;
//! - thinking: the `*` turns (15° every [`TURN_MS`]);
//! - speaking: the mouth opens with the agent's level (`:)` ↔ `:o`: a
//!   ring, taller when louder) and up to 3 arcs `)))` open beside it;
//!   quiet (under [`QUIET`]): the smile, breathing;
//! - the kiss: [`KISS_MS`] once the agent's voice ended on its own: the
//!   lips press, pucker, the `:*` blows a small `*` up and away, then
//!   the smile again;
//! - rest: the smile still (muted, typing, a failure, hold to talk).
//!
//! Under `BISE_REDUCE_MOTION` (`still`) every pose is one still frame:
//! the speaking mouth half open with two arcs, no kiss. Pure: the frame
//! comes from the pose, the animation time and the level (the mocks'
//! `face()`, site/content/voice-ux.js, ported dot for dot).

use std::f32::consts::PI;

pub const W: u16 = 41;
pub const H: u16 = 9;
/// the dots: two per cell, one above the other
const DOTS_H: usize = H as usize * 2;
const DOTS_W: usize = W as usize;
/// the mouth's centre, in dots
const CX: f32 = 23.0;
const CY: f32 = 8.5;
/// listening: the `*` grows and shrinks back this often
pub const BREATH_MS: u64 = 1200;
/// thinking: the `*` turns one step this often
pub const TURN_MS: u64 = 150;
/// speaking under this level: the mouth rests on the smile
pub const QUIET: f32 = 0.08;
/// the level of the still speaking frame (`BISE_REDUCE_MOTION`)
const STILL_LEVEL: f32 = 0.45;
/// the eyes blink once this often, shut this long (the last ms of each
/// [`BLINK_EVERY_MS`])
pub const BLINK_EVERY_MS: u64 = 4200;
pub const BLINK_MS: u64 = 180;
/// the end-of-turn kiss lasts this long, then the smile breathes again
pub const KISS_MS: u64 = 1000;
/// the smile: its half height and its bulge (at rest, a breath in,
/// while you talk)
const SMILE_H: f32 = 5.5;
const SMILE: f32 = 3.0;
const SMILE_IN: f32 = 3.6;
const SMILE_TALK: f32 = 3.5;
/// the kiss's puff: a small `*` (its offset from the mouth's centre and
/// its half length), one place every [`PUFF_MS`] from [`KISS_BLOW_MS`]
const PUFF: [(f32, f32, f32); 3] = [(9.0, -2.0, 2.0), (12.0, -3.0, 2.0), (14.0, -4.0, 1.5)];
const PUFF_MS: u64 = 120;
/// the kiss's steps: the lips press, pucker, blow (the `:*` and the
/// puff), then the smile until [`KISS_MS`]
const KISS_PUCKER_MS: u64 = 150;
const KISS_BLOW_MS: u64 = 300;
const KISS_SMILE_MS: u64 = 800;

/// What the face does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    /// mic open, nobody talks: the smile breathes
    Listen,
    /// you talk, or cut in: the smile held still
    Talk,
    Think,
    Speak,
    /// the end-of-turn kiss, this many ms in (0..[`KISS_MS`])
    Kiss(u64),
    /// the smile, still
    Rest,
}

/// The dots of one frame, then the half blocks.
struct Dots([[bool; DOTS_W]; DOTS_H]);

impl Dots {
    fn new() -> Dots {
        Dots([[false; DOTS_W]; DOTS_H])
    }

    /// JS `Math.round`: halves go up.
    fn set(&mut self, x: f32, y: f32) {
        let (x, y) = ((x + 0.5).floor(), (y + 0.5).floor());
        if x >= 0.0 && y >= 0.0 && (x as usize) < DOTS_W && (y as usize) < DOTS_H {
            self.0[y as usize][x as usize] = true;
        }
    }

    /// the colon: two 4 × 3 blocks
    fn colon(&mut self) {
        self.eyes(false);
    }

    /// the colon as eyes; `shut`: a blink, each eye its middle line
    fn eyes(&mut self, shut: bool) {
        for y0 in [4, 11] {
            for y in y0..y0 + 3 {
                if shut && y != y0 + 1 {
                    continue;
                }
                for x in 6..10 {
                    self.set(x as f32, y as f32);
                }
            }
        }
    }

    /// the `*`: three strokes of half length `r`, turned by `rot`
    fn star(&mut self, r: f32, rot: f32) {
        self.star_at(r, rot, CX, CY);
    }

    /// a `*` around (`x0`, `y0`)
    fn star_at(&mut self, r: f32, rot: f32, x0: f32, y0: f32) {
        for a in 0..3 {
            let ang = rot + a as f32 * PI / 3.0;
            let mut s = -r;
            while s <= r {
                let (x, y) = (x0 + s * ang.sin() * 1.15, y0 - s * ang.cos());
                self.set(x, y);
                self.set(x + 0.5, y);
                s += 0.25;
            }
        }
    }

    /// the smile, a `)`: half height `h`, bulging `b` dots right
    fn paren(&mut self, h: f32, b: f32) {
        let mut y = CY - h;
        while y <= CY + h {
            let u = (y - CY) / h;
            let x = CX - 1.5 + b * (1.0 - u * u);
            self.set(x, y);
            self.set(x + 1.0, y);
            y += 0.25;
        }
    }

    /// the end-of-turn kiss, `ms` in
    fn kiss(&mut self, ms: u64) {
        if ms < KISS_PUCKER_MS {
            self.paren(SMILE_H, 1.2);
        } else if ms < KISS_BLOW_MS {
            self.star(3.0, 0.0);
        } else if ms < KISS_SMILE_MS {
            self.star(5.0, 0.0);
            if let Some(&(dx, dy, r)) = PUFF.get(((ms - KISS_BLOW_MS) / PUFF_MS) as usize) {
                self.star_at(r, 0.0, CX + dx, CY + dy);
            }
        } else {
            self.paren(SMILE_H, SMILE);
        }
    }

    /// the open mouth: an ellipse ring `rx` × `ry`
    fn ring(&mut self, rx: f32, ry: f32) {
        let (ix, iy) = ((rx - 1.7).max(0.5), (ry - 1.4).max(0.5));
        for y in 0..DOTS_H {
            for x in 0..DOTS_W {
                let (dx, dy) = (x as f32 - CX, y as f32 - CY);
                let outer = (dx / rx).powi(2) + (dy / ry).powi(2);
                let inner = (dx / ix).powi(2) + (dy / iy).powi(2);
                if outer <= 1.0 && inner > 1.0 {
                    self.0[y][x] = true;
                }
            }
        }
    }

    /// up to 3 arcs right of the mouth, with the level
    fn arcs(&mut self, v: f32) {
        for j in 0..arcs_for(v) {
            let (rx, ry) = (12.0 + 2.5 * j as f32, 5.5 + 1.5 * j as f32);
            let mut a = -0.9f32;
            while a <= 0.9 {
                self.set(CX + rx * a.cos(), CY + ry * a.sin());
                a += 0.04;
            }
        }
    }

    fn rows(&self) -> [String; H as usize] {
        std::array::from_fn(|r| {
            (0..DOTS_W)
                .map(|x| match (self.0[2 * r][x], self.0[2 * r + 1][x]) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
    }
}

/// How many arcs a level opens: none under 0.15, 3 from 0.7.
pub fn arcs_for(v: f32) -> usize {
    if v < 0.15 {
        0
    } else if v < 0.4 {
        1
    } else if v < 0.7 {
        2
    } else {
        3
    }
}

/// One frame: [`H`] rows of [`W`] cells (`█ ▀ ▄` and spaces). `level`:
/// the agent's output level 0..1 (speaking only); `still`: no motion.
pub fn frame(pose: Pose, t_ms: u64, level: f32, still: bool) -> [String; H as usize] {
    let mut d = Dots::new();
    // the user (round 5, the designer's row 4): the eyes blink now and
    // then, at rest, while you talk and while it thinks; never while it
    // speaks or kisses (one gesture at a time: a blink in the kiss is
    // skipped)
    let blinks = match pose {
        Pose::Listen | Pose::Talk | Pose::Think | Pose::Rest => true,
        Pose::Kiss(ms) => ms >= KISS_MS,
        Pose::Speak => false,
    };
    d.eyes(!still && blinks && t_ms % BLINK_EVERY_MS >= BLINK_EVERY_MS - BLINK_MS);
    // the smile at rest: a breath in every other BREATH_MS (still: the
    // smile, no breath)
    let breathing = |d: &mut Dots| {
        let br = !still && (t_ms / BREATH_MS) % 2 == 1;
        d.paren(SMILE_H, if br { SMILE_IN } else { SMILE });
    };
    match pose {
        Pose::Listen => breathing(&mut d),
        Pose::Talk => d.paren(SMILE_H, SMILE_TALK),
        Pose::Think => {
            let step = if still { 0 } else { t_ms / TURN_MS };
            d.star(5.0, (step % 24) as f32 * PI / 12.0);
        }
        Pose::Speak => {
            let v = if still { STILL_LEVEL } else { level.clamp(0.0, 1.0) };
            if v < QUIET {
                breathing(&mut d);
            } else {
                d.ring(4.5 + v * 2.5, 1.8 + v * 6.5);
                d.arcs(v);
            }
        }
        Pose::Kiss(ms) if !still && ms < KISS_MS => d.kiss(ms),
        Pose::Kiss(_) => breathing(&mut d),
        Pose::Rest => d.paren(SMILE_H, SMILE),
    }
    d.rows()
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSES: [Pose; 8] =
        [Pose::Listen, Pose::Talk, Pose::Think, Pose::Speak, Pose::Kiss(0), Pose::Kiss(200), Pose::Kiss(450), Pose::Rest];

    fn width(rows: &[String]) -> usize {
        rows.iter().map(|r| r.chars().count()).max().unwrap_or(0)
    }

    fn drawn(f: impl FnOnce(&mut Dots)) -> [String; H as usize] {
        let mut d = Dots::new();
        d.colon();
        f(&mut d);
        d.rows()
    }

    /// the mouth's cells: right of the colon
    fn mouth(f: &[String; 9]) -> Vec<String> {
        f.iter().map(|r| r.chars().skip(12).collect()).collect()
    }

    #[test]
    fn every_pose_is_41_by_9_in_half_blocks() {
        for pose in POSES {
            for t in [0, 700, 1300, 5000] {
                let f = frame(pose, t, 0.8, false);
                assert_eq!(f.len(), 9);
                assert_eq!(width(&f), 41, "{pose:?}");
                assert!(f.iter().all(|r| r.chars().count() == 41));
                assert!(f.iter().flat_map(|r| r.chars()).all(|c| " █▀▄".contains(c)));
            }
        }
    }

    #[test]
    fn the_colon_is_always_there_on_the_left() {
        for pose in POSES {
            let f = frame(pose, 0, 0.0, false);
            // dots 4..7 and 11..14, columns 6..10: rows 2-3 and 5-7
            let cells = |r: usize| f[r].chars().skip(6).take(4).collect::<String>();
            assert_eq!([cells(2), cells(3)], ["████", "▀▀▀▀"], "{pose:?}");
            assert_eq!([cells(5), cells(6)], ["▄▄▄▄", "████"], "{pose:?}");
        }
    }

    #[test]
    fn at_rest_it_smiles_a_tall_paren_right_of_the_colon() {
        let f = frame(Pose::Rest, 0, 0.0, false);
        assert_eq!(f, drawn(|d| d.paren(SMILE_H, SMILE)));
        // 5.5 dots each side of 8.5: rows 1..=7 inked, the middle the
        // furthest right (a ')'), the ends back left
        let m = mouth(&f);
        let inked: Vec<usize> = (0..9).filter(|&i| !m[i].trim().is_empty()).collect();
        assert_eq!(inked, (1..=7).collect::<Vec<_>>());
        let left = |r: &str| r.chars().position(|c| c != ' ').unwrap();
        assert!(left(&m[4]) > left(&m[1]) && left(&m[4]) > left(&m[7]), "{m:#?}");
        assert!(f.iter().all(|r| r.chars().skip(34).all(|c| c == ' ')), "no arcs at rest");
    }

    #[test]
    fn listening_breathes_you_talking_holds_and_thinking_turns() {
        let listen = |t| frame(Pose::Listen, t, 0.0, false);
        assert_eq!(listen(0), frame(Pose::Rest, 0, 0.0, false));
        assert_ne!(listen(0), listen(BREATH_MS));
        assert_eq!(listen(0), listen(2 * BREATH_MS));
        assert_eq!(listen(BREATH_MS), drawn(|d| d.paren(SMILE_H, SMILE_IN)));
        // you talk: a little rounder, and it does not move
        let talk = frame(Pose::Talk, 0, 0.0, false);
        assert_eq!(talk, drawn(|d| d.paren(SMILE_H, SMILE_TALK)));
        assert_eq!(talk, frame(Pose::Talk, BREATH_MS, 0.9, false));
        assert_ne!(talk, listen(0));
        // thinking: the * turns
        assert_ne!(frame(Pose::Think, 0, 0.0, false), frame(Pose::Think, TURN_MS, 0.0, false));
        // a turn of 180° is the same star
        assert_eq!(frame(Pose::Think, 0, 0.0, false), frame(Pose::Think, 12 * TURN_MS, 0.0, false));
        assert_eq!(frame(Pose::Think, 0, 0.0, false), drawn(|d| d.star(5.0, 0.0)));
    }

    #[test]
    fn the_mouth_opens_with_the_level_and_the_arcs_follow() {
        // quiet: the smile, breathing (never the small * any more)
        assert_eq!(frame(Pose::Speak, 0, 0.0, false), frame(Pose::Listen, 0, 0.0, false));
        assert_eq!(frame(Pose::Speak, BREATH_MS, 0.05, false), frame(Pose::Listen, BREATH_MS, 0.0, false));
        let ink = |f: &[String; 9]| f.iter().flat_map(|r| r.chars()).filter(|&c| c != ' ').count();
        let rows_used = |f: &[String; 9]| f.iter().filter(|r| !r[r.char_indices().nth(16).unwrap().0..].trim().is_empty()).count();
        let (soft, loud) = (frame(Pose::Speak, 0, 0.2, false), frame(Pose::Speak, 0, 0.9, false));
        assert!(rows_used(&loud) > rows_used(&soft), "louder is taller");
        assert!(ink(&loud) > ink(&soft));
        // the arcs live right of the mouth (past column 33)
        let right = |f: &[String; 9]| f.iter().any(|r| r.chars().skip(34).any(|c| c != ' '));
        assert!(!right(&frame(Pose::Speak, 0, 0.1, false)), "no arcs when soft");
        assert!(right(&loud));
        assert_eq!((arcs_for(0.1), arcs_for(0.2), arcs_for(0.5), arcs_for(0.9)), (0, 1, 2, 3));
    }

    #[test]
    fn the_kiss_presses_puckers_blows_a_puff_then_smiles() {
        let k = |ms| frame(Pose::Kiss(ms), 0, 0.0, false);
        assert_eq!(k(0), drawn(|d| d.paren(SMILE_H, 1.2)), "the lips press");
        assert_eq!(k(149), k(0));
        assert_eq!(k(150), drawn(|d| d.star(3.0, 0.0)), "pucker");
        assert_eq!(k(299), k(150));
        // the kiss: the logo's :* and a small * flying up and right
        let puff = |i: usize| {
            let (dx, dy, r) = PUFF[i];
            drawn(|d| {
                d.star(5.0, 0.0);
                d.star_at(r, 0.0, CX + dx, CY + dy);
            })
        };
        assert_eq!((k(300), k(419)), (puff(0), puff(0)));
        assert_eq!((k(420), k(539)), (puff(1), puff(1)));
        assert_eq!((k(540), k(659)), (puff(2), puff(2)));
        assert_eq!((k(660), k(799)), (drawn(|d| d.star(5.0, 0.0)), drawn(|d| d.star(5.0, 0.0))), "the :* alone");
        let top_right = |f: &[String; 9]| f[..4].iter().any(|r| r.chars().skip(30).any(|c| c != ' '));
        assert!(top_right(&k(420)) && top_right(&k(540)) && !top_right(&k(700)));
        assert!(k(540).iter().all(|r| r.chars().count() == 41), "the puff stays in the face");
        // back to the smile, then it breathes again
        assert_eq!(k(800), frame(Pose::Rest, 0, 0.0, false));
        assert_eq!(k(999), k(800));
        assert_eq!(frame(Pose::Kiss(KISS_MS), BREATH_MS, 0.0, false), frame(Pose::Listen, BREATH_MS, 0.0, false));
    }

    #[test]
    fn the_eyes_blink_now_and_then_but_not_while_it_speaks() {
        let shut = BLINK_EVERY_MS - BLINK_MS;
        let blinked = drawn(|_| {});
        let eyes_shut = |f: &[String; 9]| {
            let cells = |r: usize| f[r].chars().skip(6).take(4).collect::<String>();
            // each eye its middle line: dots 5 and 12, rows 2 (low half)
            // and 6 (high half)
            [cells(1), cells(2), cells(3), cells(5), cells(6), cells(7)] == ["    ", "▄▄▄▄", "    ", "    ", "▀▀▀▀", "    "]
        };
        assert!(!eyes_shut(&blinked));
        for pose in [Pose::Listen, Pose::Talk, Pose::Think, Pose::Rest, Pose::Kiss(KISS_MS)] {
            assert!(!eyes_shut(&frame(pose, shut - 1, 0.0, false)), "{pose:?}");
            assert!(eyes_shut(&frame(pose, shut, 0.0, false)), "{pose:?}");
            assert!(eyes_shut(&frame(pose, BLINK_EVERY_MS - 1, 0.0, false)), "{pose:?}");
            assert!(!eyes_shut(&frame(pose, BLINK_EVERY_MS, 0.0, false)), "{pose:?}");
            assert!(!eyes_shut(&frame(pose, shut, 0.0, true)), "still: no blink, {pose:?}");
        }
        assert!(!eyes_shut(&frame(Pose::Speak, shut, 0.5, false)));
        assert!(!eyes_shut(&frame(Pose::Speak, shut, 0.0, false)));
        // one gesture at a time: no blink in the kiss
        for ms in [0, 200, 450, 900] {
            assert!(!eyes_shut(&frame(Pose::Kiss(ms), shut, 0.0, false)), "{ms}");
        }
    }

    #[test]
    fn still_frames_do_not_move_and_never_kiss() {
        for pose in POSES {
            let a = frame(pose, 0, 0.1, true);
            for t in [150, 1200, 9999] {
                assert_eq!(a, frame(pose, t, 0.9, true), "{pose:?} at {t}");
            }
        }
        for ms in [0, 200, 450, 900] {
            assert_eq!(frame(Pose::Kiss(ms), 0, 0.0, true), frame(Pose::Listen, 0, 0.0, true), "{ms}");
        }
        // the still smile: the canonical one, no breath in
        assert_eq!(frame(Pose::Listen, BREATH_MS, 0.0, true), frame(Pose::Rest, 0, 0.0, false));
        // still speaking: half open, two arcs
        let s = frame(Pose::Speak, 0, 0.0, true);
        assert_ne!(s, frame(Pose::Rest, 0, 0.0, true));
    }
}

#[cfg(test)]
mod captures {
    //! `KISS_CAPTURES=<dir> cargo test -p bend-tui kiss_captures --
    //! --ignored` writes the designer's 41 × 9 captures of round 5: the
    //! smile, the blink, the kiss's steps (text, ANSI dark, one HTML
    //! page dark and NO_COLOR).
    use super::*;
    use crate::theme::{self, Mode};
    use ratatui::style::Color;

    #[test]
    #[ignore]
    fn kiss_captures() {
        let Some(dir) = std::env::var_os("KISS_CAPTURES") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        theme::set_mode(Mode::Dark);
        let Color::Rgb(r, g, b) = theme::accent() else { panic!("an rgb accent") };
        let shut = BLINK_EVERY_MS - BLINK_MS;
        let shots: [(&str, Pose, u64, f32); 14] = [
            ("smile-rest", Pose::Listen, 0, 0.0),
            ("smile-breath-in", Pose::Listen, BREATH_MS, 0.0),
            ("smile-you-talk", Pose::Talk, 0, 0.0),
            ("smile-blink", Pose::Listen, shut, 0.0),
            ("think", Pose::Think, 2 * TURN_MS, 0.0),
            ("speak", Pose::Speak, 0, 0.6),
            ("kiss-0000-press", Pose::Kiss(0), 0, 0.0),
            ("kiss-0150-pucker", Pose::Kiss(150), 0, 0.0),
            ("kiss-0300-puff1", Pose::Kiss(300), 0, 0.0),
            ("kiss-0420-puff2", Pose::Kiss(420), 0, 0.0),
            ("kiss-0540-puff3", Pose::Kiss(540), 0, 0.0),
            ("kiss-0700-alone", Pose::Kiss(700), 0, 0.0),
            ("kiss-0800-smile", Pose::Kiss(800), 0, 0.0),
            ("rest-muted", Pose::Rest, 0, 0.0),
        ];
        let mut html = String::from(
            "<!doctype html><meta charset=utf-8><title>voice mode: the smile and the kiss</title><style>body{background:#111;color:#ccc;font:13px ui-monospace,Menlo,monospace;display:flex;flex-wrap:wrap;gap:16px;padding:16px}figure{margin:0}pre{margin:0;padding:8px;line-height:1;border:1px solid #333}.nc{color:#ddd}figcaption{margin-top:4px}</style>\n",
        );
        for (name, pose, t, level) in shots {
            let f = frame(pose, t, level, false);
            let text = f.join("\n") + "\n";
            std::fs::write(dir.join(format!("{name}.txt")), &text).unwrap();
            let ansi: String = f.iter().map(|row| format!("\x1b[38;2;{r};{g};{b}m{row}\x1b[0m\n")).collect();
            std::fs::write(dir.join(format!("{name}.ansi")), ansi).unwrap();
            for (cls, style) in [("dark", format!("color:rgb({r},{g},{b})")), ("nc", String::new())] {
                html.push_str(&format!(
                    "<figure><pre class={cls} style=\"{style}\">{text}</pre><figcaption>{name} · {}</figcaption></figure>\n",
                    if cls == "nc" { "NO_COLOR" } else { "dark" }
                ));
            }
        }
        std::fs::write(dir.join("index.html"), html).unwrap();
    }
}
