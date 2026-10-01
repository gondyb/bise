//! Is someone talking (owner: voice-audio; plan §2, §4.1). Pure: blocks
//! of MIC_RATE PCM in, speech or not out.
//!
//! An energy VAD on 10 ms frames: a high-pass (the hum and the rumble
//! out), the frame's RMS in dBFS against an adaptive noise floor (the
//! quietest frame of the last [`FLOOR_WINDOW`]), speech when a frame is
//! [`OVER_FLOOR_DB`] over it.
//! Speech starts after [`ONSET`] of loud frames in a row (a key click
//! rings ~30 ms: never speech) and ends [`HANGOVER`] after the last one
//! (a breath between two words stays speech). Tested on the recorded
//! fixtures (testdata/): a sentence, the same in a room, "mm", "ok",
//! silence, a room, a keyboard.

use super::MIC_RATE;
use std::collections::VecDeque;
use std::time::Duration;

/// The analysis frame.
pub const FRAME: Duration = Duration::from_millis(10);
/// Loud frames in a row before speech starts.
pub const ONSET: Duration = Duration::from_millis(60);
/// Speech stays on this long after the last loud frame (plan §4.1).
pub const HANGOVER: Duration = Duration::from_millis(150);
/// A frame this far over the noise floor is loud.
pub const OVER_FLOOR_DB: f32 = 10.0;
/// Quieter than this is never speech (a dead-quiet mic's floor is
/// -90 dBFS: its own small noise must not count).
pub const MIN_SPEECH_DB: f32 = -55.0;
/// The noise floor is the quietest frame of this window: a fan turned
/// on becomes the floor within it.
pub const FLOOR_WINDOW: Duration = Duration::from_secs(3);

const FRAME_LEN: usize = (MIC_RATE as usize) / 100;

fn frames(d: Duration) -> u32 {
    (d.as_millis() / FRAME.as_millis()) as u32
}

#[derive(Debug)]
pub struct Vad {
    /// the noise floor, dBFS
    floor_db: f32,
    /// loud frames in a row
    loud_run: u32,
    speaking: bool,
    /// frames of speech left after the last loud one
    hang: u32,
    /// samples waiting for a whole frame
    carry: Vec<i16>,
    /// the high-pass's state (previous input, previous output)
    hp: (f32, f32),
    /// the last FLOOR_WINDOW of frame energies, dBFS
    recent: VecDeque<f32>,
}

impl Default for Vad {
    fn default() -> Self {
        Vad::new()
    }
}

impl Vad {
    pub fn new() -> Vad {
        Vad {
            floor_db: f32::INFINITY,
            loud_run: 0,
            speaking: false,
            hang: 0,
            carry: Vec::with_capacity(FRAME_LEN),
            hp: (0.0, 0.0),
            recent: VecDeque::with_capacity(frames(FLOOR_WINDOW) as usize),
        }
    }

    /// One block (any length): is it speech? True when speech was on in
    /// any of its frames (with the hangover, so a breath between two
    /// words stays speech). A block shorter than a frame answers for the
    /// frames it completes, else the state so far.
    pub fn feed(&mut self, pcm: &[i16]) -> bool {
        let mut any = false;
        let mut whole = false;
        for &s in pcm {
            self.carry.push(s);
            if self.carry.len() == FRAME_LEN {
                let frame = std::mem::take(&mut self.carry);
                any |= self.frame(&frame);
                whole = true;
                self.carry = frame;
                self.carry.clear();
            }
        }
        if whole {
            any
        } else {
            self.speaking
        }
    }

    /// Speech now (the last frame's state).
    pub fn speaking(&self) -> bool {
        self.speaking
    }

    /// The noise floor now, dBFS (tests, a debug line).
    pub fn floor_db(&self) -> f32 {
        self.floor_db
    }

    fn frame(&mut self, frame: &[i16]) -> bool {
        let db = self.energy_db(frame);
        // the floor: the quietest frame of the last FLOOR_WINDOW (speech
        // has quiet frames between its words; a room never gets louder
        // than its own quietest by 10 dB in 3 s)
        if self.recent.len() == frames(FLOOR_WINDOW) as usize {
            self.recent.pop_front();
        }
        self.recent.push_back(db);
        self.floor_db = self.recent.iter().copied().fold(f32::INFINITY, f32::min);
        let loud = db >= MIN_SPEECH_DB && db >= self.floor_db + OVER_FLOOR_DB;
        self.loud_run = if loud { self.loud_run + 1 } else { 0 };
        if loud && (self.speaking || self.loud_run >= frames(ONSET)) {
            self.speaking = true;
            self.hang = frames(HANGOVER);
        } else if self.speaking {
            if self.hang > 0 {
                self.hang -= 1;
            }
            if self.hang == 0 {
                self.speaking = false;
            }
        }
        self.speaking
    }

    /// The frame's RMS in dBFS after a ~80 Hz high-pass.
    fn energy_db(&mut self, frame: &[i16]) -> f32 {
        const A: f32 = 0.969; // 1 - 2π·80/16000
        let (mut px, mut py) = self.hp;
        let mut sum = 0.0f64;
        for &s in frame {
            let x = s as f32 / 32768.0;
            let y = A * (py + x - px);
            px = x;
            py = y;
            sum += (y as f64) * (y as f64);
        }
        self.hp = (px, py);
        let rms = (sum / frame.len().max(1) as f64).sqrt() as f32;
        20.0 * rms.max(1e-6).log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voicemode::audio::fixture;

    const BLOCK: usize = 320; // 20 ms, the mic's blocks

    /// The VAD's answer per 20 ms block.
    fn run(pcm: &[i16], block: usize) -> Vec<bool> {
        let mut v = Vad::new();
        pcm.chunks(block).map(|b| v.feed(b)).collect()
    }

    fn ms(blocks: usize, block: usize) -> usize {
        blocks * block * 1000 / MIC_RATE as usize
    }

    fn speech_ms(pcm: &[i16]) -> usize {
        ms(run(pcm, BLOCK).iter().filter(|&&s| s).count(), BLOCK)
    }

    fn first_speech_ms(pcm: &[i16]) -> Option<usize> {
        run(pcm, BLOCK).iter().position(|&s| s).map(|i| ms(i, BLOCK))
    }

    fn last_speech_ms(pcm: &[i16]) -> Option<usize> {
        run(pcm, BLOCK).iter().rposition(|&s| s).map(|i| ms(i + 1, BLOCK))
    }

    fn louder(pcm: &[i16], gain: f32) -> Vec<i16> {
        pcm.iter().map(|&s| (s as f32 * gain).clamp(-32767.0, 32767.0) as i16).collect()
    }

    #[test]
    fn silence_is_never_speech() {
        assert_eq!(speech_ms(&fixture("silence")), 0);
        assert_eq!(speech_ms(&[0; 16000]), 0);
    }

    #[test]
    fn a_room_is_never_speech() {
        assert_eq!(speech_ms(&fixture("room")), 0);
        // a noisy room (+18 dB): the floor learns it
        assert_eq!(speech_ms(&louder(&fixture("room"), 8.0)), 0);
    }

    #[test]
    fn a_keyboard_is_never_speech() {
        assert_eq!(speech_ms(&fixture("keyboard")), 0);
    }

    #[test]
    fn a_sentence_is_speech_from_its_start_to_its_end() {
        let pcm = fixture("sentence");
        let len = ms(pcm.len(), 1);
        let s = speech_ms(&pcm);
        assert!(s * 10 >= len * 7, "{} ms of speech in {} ms", s, len);
    }

    #[test]
    fn a_sentence_in_a_room_starts_and_ends_on_time() {
        // 500 ms of room, the sentence, 700 ms of room (noise.py)
        let pcm = fixture("sentence_room");
        let speech = fixture("sentence");
        let start = first_speech_ms(&pcm).expect("speech");
        let end = last_speech_ms(&pcm).expect("speech");
        // say's file has its own lead-in and tail of silence
        let lead = first_speech_ms(&speech).unwrap();
        let tail_end = last_speech_ms(&speech).unwrap();
        assert!((500 + lead).abs_diff(start) <= 40, "starts at {} ms, the words at {} ms", start, 500 + lead);
        assert!((500 + tail_end).abs_diff(end) <= 60, "ends at {} ms, the words at {} ms", end, 500 + tail_end);
        // and nothing in the room before or after
        assert!(run(&pcm, BLOCK).iter().skip(end / 20 + 1).all(|&s| !s));
    }

    #[test]
    fn mm_and_ok_are_speech() {
        // the VAD hears them; the controller decides they never cut in
        for name in ["mm", "ok"] {
            let s = speech_ms(&fixture(name));
            assert!(s >= 150, "{}: {} ms of speech", name, s);
        }
    }

    #[test]
    fn a_short_pause_stays_speech_a_long_one_ends_it() {
        let tone = |ms: usize| -> Vec<i16> {
            (0..ms * 16).map(|i| ((i as f32 * 220.0 * std::f32::consts::TAU / 16000.0).sin() * 6000.0) as i16).collect()
        };
        let gap = |ms: usize| vec![0i16; ms * 16];
        let mut pcm = gap(300);
        pcm.extend(tone(400));
        pcm.extend(gap(100)); // a breath
        pcm.extend(tone(400));
        pcm.extend(gap(600));
        let blocks = run(&pcm, BLOCK);
        let on = |at: usize| blocks[at / 20];
        assert!(!on(280));
        assert!(on(400) && on(750) && on(1000), "{:?}", blocks);
        // the hangover: on ~150 ms past the last sound (1200 ms), off after
        assert!(on(1200 + 120));
        assert!(!on(1200 + 200));
    }

    #[test]
    fn block_size_does_not_change_the_answer() {
        let pcm = fixture("sentence_room");
        let on_ms = |block: usize| ms(run(&pcm, block).iter().filter(|&&s| s).count(), block);
        let (a, b, c) = (on_ms(320), on_ms(1600), on_ms(160));
        assert!(a.abs_diff(c) <= 20, "20 ms blocks {} ms, 10 ms blocks {} ms", a, c);
        // 100 ms blocks round up to whole blocks
        assert!(b >= a && b <= a + 300, "20 ms blocks {} ms, 100 ms blocks {} ms", a, b);
        // and a block shorter than a frame keeps the state
        let mut v = Vad::new();
        for blk in pcm.chunks(320).take(60) {
            v.feed(blk);
        }
        assert_eq!(v.feed(&[0; 10]), v.speaking());
    }

    #[test]
    fn the_floor_follows_the_room() {
        let mut v = Vad::new();
        for b in louder(&fixture("room"), 8.0).chunks(BLOCK) {
            v.feed(b);
        }
        let loud_room = v.floor_db();
        for b in fixture("room").chunks(BLOCK) {
            v.feed(b);
        }
        assert!(v.floor_db() < loud_room - 10.0, "{} then {}", loud_room, v.floor_db());
        // and speech in the loud room is still speech
        let room = louder(&fixture("room"), 8.0);
        let speech = fixture("sentence");
        let mut pcm = room.clone();
        for (i, s) in speech.iter().enumerate() {
            pcm[i % room.len()] = pcm[i % room.len()].saturating_add(*s);
        }
        let mut both = room;
        both.extend(&pcm[..speech.len().min(pcm.len())]);
        assert!(speech_ms(&both) >= 1500, "{} ms", speech_ms(&both));
    }
}
