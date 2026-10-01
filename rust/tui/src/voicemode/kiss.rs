//! The big talking kiss (owner: voice-tui; design §0, plan §2): 41 × 9
//! cells in half blocks (41 × 18 dots), the `:*` drawn big: the colon
//! on the left, the mouth on the right.
//!
//! - listening: the `*` breathes (a little bigger every other
//!   [`BREATH_MS`]);
//! - thinking: the `*` turns (15° every [`TURN_MS`]);
//! - speaking: the mouth opens with the agent's level (`:*` ↔ `:o`: a
//!   ring, taller when louder) and up to 3 arcs `)))` open beside it;
//!   quiet (under [`QUIET`]): a smaller `*`;
//! - cut in: it shuts to a line;
//! - rest: the `*` still (muted, typing, a failure).
//!
//! Under `BISE_REDUCE_MOTION` (`still`) every pose is one still frame:
//! the speaking mouth half open with two arcs. Pure: the frame comes from
//! the pose, the animation time and the level (the mocks' `face()`,
//! site/content/voice-ux.js, ported dot for dot).

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
/// speaking under this level: the mouth rests on a smaller `*`
pub const QUIET: f32 = 0.08;
/// the level of the still speaking frame (`BISE_REDUCE_MOTION`)
const STILL_LEVEL: f32 = 0.45;

/// What the kiss does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Listen,
    Think,
    Speak,
    Cut,
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
        for y0 in [4, 11] {
            for y in y0..y0 + 3 {
                for x in 6..10 {
                    self.set(x as f32, y as f32);
                }
            }
        }
    }

    /// the `*`: three strokes of half length `r`, turned by `rot`
    fn star(&mut self, r: f32, rot: f32) {
        for a in 0..3 {
            let ang = rot + a as f32 * PI / 3.0;
            let mut s = -r;
            while s <= r {
                let (x, y) = (CX + s * ang.sin() * 1.15, CY - s * ang.cos());
                self.set(x, y);
                self.set(x + 0.5, y);
                s += 0.25;
            }
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

    /// the shut mouth: a line `w` dots each side
    fn line(&mut self, w: f32) {
        let mut x = CX - w;
        while x <= CX + w {
            self.set(x, CY);
            x += 1.0;
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
    d.colon();
    match pose {
        Pose::Listen => {
            let big = still || (t_ms / BREATH_MS) % 2 == 1;
            d.star(if big { 5.75 } else { 5.0 }, 0.0);
        }
        Pose::Think => {
            let step = if still { 0 } else { t_ms / TURN_MS };
            d.star(5.0, (step % 24) as f32 * PI / 12.0);
        }
        Pose::Speak => {
            let v = if still { STILL_LEVEL } else { level.clamp(0.0, 1.0) };
            if v < QUIET {
                d.star(4.0, 0.0);
            } else {
                d.ring(4.5 + v * 2.5, 1.8 + v * 6.5);
                d.arcs(v);
            }
        }
        Pose::Cut => d.line(4.0),
        Pose::Rest => d.star(5.0, 0.0),
    }
    d.rows()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width(rows: &[String]) -> usize {
        rows.iter().map(|r| r.chars().count()).max().unwrap_or(0)
    }

    #[test]
    fn every_pose_is_41_by_9_in_half_blocks() {
        for pose in [Pose::Listen, Pose::Think, Pose::Speak, Pose::Cut, Pose::Rest] {
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
        let f = frame(Pose::Cut, 0, 0.0, false);
        // dots 4..7 and 11..14, columns 6..10: rows 2-3 and 5-7
        let cells = |r: usize| f[r].chars().skip(6).take(4).collect::<String>();
        assert_eq!([cells(2), cells(3)], ["████", "▀▀▀▀"]);
        assert_eq!([cells(5), cells(6)], ["▄▄▄▄", "████"]);
        assert!(f[0].trim().is_empty() && f[8].trim().is_empty());
    }

    #[test]
    fn listening_breathes_and_thinking_turns() {
        assert_ne!(frame(Pose::Listen, 0, 0.0, false), frame(Pose::Listen, BREATH_MS, 0.0, false));
        assert_eq!(frame(Pose::Listen, 0, 0.0, false), frame(Pose::Listen, 2 * BREATH_MS, 0.0, false));
        assert_ne!(frame(Pose::Think, 0, 0.0, false), frame(Pose::Think, TURN_MS, 0.0, false));
        // a turn of 180° is the same star
        assert_eq!(frame(Pose::Think, 0, 0.0, false), frame(Pose::Think, 12 * TURN_MS, 0.0, false));
    }

    #[test]
    fn the_mouth_opens_with_the_level_and_the_arcs_follow() {
        let quiet = frame(Pose::Speak, 0, 0.0, false);
        assert_eq!(quiet, {
            let mut d = Dots::new();
            d.colon();
            d.star(4.0, 0.0);
            d.rows()
        });
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
    fn cut_in_shuts_the_mouth_to_one_line() {
        let f = frame(Pose::Cut, 0, 1.0, false);
        let mouth: Vec<&str> = f.iter().map(|r| &r[r.char_indices().nth(16).unwrap().0..]).collect();
        let inked: Vec<usize> = (0..9).filter(|&i| !mouth[i].trim().is_empty()).collect();
        assert_eq!(inked, vec![4], "one row: the line");
    }

    #[test]
    fn still_frames_do_not_move() {
        for pose in [Pose::Listen, Pose::Think, Pose::Speak, Pose::Cut, Pose::Rest] {
            let a = frame(pose, 0, 0.1, true);
            for t in [150, 1200, 9999] {
                assert_eq!(a, frame(pose, t, 0.9, true), "{pose:?} at {t}");
            }
        }
        // still speaking: half open, two arcs
        let s = frame(Pose::Speak, 0, 0.0, true);
        assert_ne!(s, frame(Pose::Rest, 0, 0.0, true));
    }
}
