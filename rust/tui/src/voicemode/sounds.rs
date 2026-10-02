//! Voice mode's two sounds (voice-echo3; the user: "n'importe quel texte
//! que tu dis de manière automatique à chaque fois que j'arrête de
//! parler, c'est pas ouf ... du son"): they replace the spoken "on it".
//! Both are wind (the brand: la bise), filtered noise, no tone, no beep;
//! the direction tells them apart (designer's recipe):
//! - [`Sound::Gone`]: your turn was taken and sent; rises, goes away.
//! - [`Sound::Back`]: the agent finished talking on its own, the mic is
//!   yours again; falls, comes back (narrower: a soft pitch, as blowing
//!   across a bottle).
//!
//! Pure: [`pcm`] synthesizes one at [`TTS_RATE`] mono f32, the same
//! samples every time (white noise from a fixed seed through a swept
//! RBJ band-pass, a raised-cosine attack, an exponential decay, a 5 ms
//! fade at the end, the peak normalized). The controller plays them on
//! the voice's speaker (the echo canceller sees them) unless the /voice
//! "sounds" row is off; no test plays them.

use super::TTS_RATE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Gone,
    Back,
}

/// One sound's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub length_ms: f32,
    /// the band-pass center sweeps exponentially from `from_hz` to `to_hz`
    pub from_hz: f32,
    pub to_hz: f32,
    pub q: f32,
    pub attack_ms: f32,
    /// the decay's time constant after the attack
    pub decay_ms: f32,
    pub peak_dbfs: f32,
}

impl Sound {
    pub fn recipe(self) -> Recipe {
        match self {
            Sound::Gone => Recipe {
                length_ms: 260.0,
                from_hz: 500.0,
                to_hz: 1800.0,
                q: 1.2,
                attack_ms: 40.0,
                decay_ms: 60.0,
                peak_dbfs: -24.0,
            },
            Sound::Back => Recipe {
                length_ms: 340.0,
                from_hz: 1400.0,
                to_hz: 700.0,
                q: 4.0,
                attack_ms: 80.0,
                decay_ms: 90.0,
                peak_dbfs: -26.0,
            },
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sound::Gone => "gone",
            Sound::Back => "back",
        }
    }

    pub fn length(self) -> std::time::Duration {
        std::time::Duration::from_secs_f32(self.recipe().length_ms / 1000.0)
    }
}

/// The filter's coefficients are recomputed every this many samples.
const STEP: usize = 32;
/// The end's fade (no click).
const FADE_MS: f32 = 5.0;

/// xorshift32 from a fixed seed: white noise in [-1, 1).
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// RBJ band-pass, constant 0 dB peak gain: (b0, b2, a1, a2), b1 = 0,
/// normalized by a0.
fn band_pass(center: f32, q: f32, rate: f32) -> (f32, f32, f32, f32) {
    let w0 = 2.0 * std::f32::consts::PI * center / rate;
    let alpha = w0.sin() / (2.0 * q);
    let a0 = 1.0 + alpha;
    (alpha / a0, -alpha / a0, -2.0 * w0.cos() / a0, (1.0 - alpha) / a0)
}

/// The envelope at `t` ms: raised-cosine attack, exponential decay, a
/// cosine fade over the last [`FADE_MS`].
fn envelope(r: &Recipe, t: f32) -> f32 {
    use std::f32::consts::PI;
    let body = if t < r.attack_ms { 0.5 * (1.0 - (PI * t / r.attack_ms).cos()) } else { (-(t - r.attack_ms) / r.decay_ms).exp() };
    let to_end = r.length_ms - t;
    let fade = if to_end < FADE_MS { 0.5 * (1.0 - (PI * (to_end.max(0.0) / FADE_MS)).cos()) } else { 1.0 };
    body * fade
}

/// The sound, [`TTS_RATE`] mono f32.
pub fn pcm(sound: Sound) -> Vec<f32> {
    render(&sound.recipe(), TTS_RATE, 0x2545_F491)
}

/// `r` at `rate` from noise `seed` (pure: the same samples every time).
pub fn render(r: &Recipe, rate: u32, seed: u32) -> Vec<f32> {
    let fs = rate as f32;
    let n = (r.length_ms / 1000.0 * fs).round() as usize;
    let mut noise = Noise(seed.max(1));
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut coef = band_pass(r.from_hz, r.q, fs);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / fs * 1000.0;
        if i % STEP == 0 {
            let center = r.from_hz * (r.to_hz / r.from_hz).powf(t / r.length_ms);
            coef = band_pass(center, r.q, fs);
        }
        let (b0, b2, a1, a2) = coef;
        let x = noise.next();
        let y = b0 * x + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
        out.push(y * envelope(r, t));
    }
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        let gain = 10f32.powf(r.peak_dbfs / 20.0) / peak;
        for s in &mut out {
            *s *= gain;
        }
    }
    out
}

/// `pcm` as a 16-bit mono WAV file at `rate` (the designer's listening
/// files; never played by bise).
pub fn wav(pcm: &[f32], rate: u32) -> Vec<u8> {
    let samples: Vec<i16> = pcm.iter().map(|x| crate::voice::to_i16(*x)).collect();
    crate::voice::wav_bytes(&samples, rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak_dbfs(p: &[f32]) -> f32 {
        20.0 * p.iter().fold(0.0f32, |m, s| m.max(s.abs())).log10()
    }

    /// Zero crossings per second: the sound's rough pitch.
    fn crossings(p: &[f32], rate: u32) -> f32 {
        let c = p.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        c as f32 / (p.len() as f32 / rate as f32)
    }

    #[test]
    fn each_sound_is_short_soft_and_the_same_every_time() {
        for (s, ms, db) in [(Sound::Gone, 260, -24.0), (Sound::Back, 340, -26.0)] {
            let p = pcm(s);
            assert_eq!(p.len(), TTS_RATE as usize * ms / 1000, "{s:?}");
            assert!((peak_dbfs(&p) - db).abs() < 0.01, "{s:?}: {}", peak_dbfs(&p));
            assert_eq!(p, pcm(s), "a sign, not a texture");
            // starts and ends at silence: no click
            assert!(p[0].abs() < 1e-3 && p[p.len() - 1].abs() < 1e-3, "{s:?}");
            assert!(p.iter().all(|x| x.is_finite()));
        }
        assert_ne!(pcm(Sound::Gone)[..1000], pcm(Sound::Back)[..1000]);
    }

    #[test]
    fn gone_rises_and_back_falls() {
        let half = |p: &[f32]| {
            let m = p.len() / 2;
            (crossings(&p[..m], TTS_RATE), crossings(&p[m..], TTS_RATE))
        };
        let (a, b) = half(&pcm(Sound::Gone));
        assert!(b > a, "gone rises: {a} then {b}");
        let (a, b) = half(&pcm(Sound::Back));
        assert!(b < a, "back falls: {a} then {b}");
    }

    #[test]
    fn the_envelope_rises_then_decays_and_fades_out() {
        let r = Sound::Gone.recipe();
        assert_eq!(envelope(&r, 0.0), 0.0);
        assert!((envelope(&r, r.attack_ms) - 1.0).abs() < 1e-6);
        assert!(envelope(&r, 100.0) < envelope(&r, 50.0));
        assert!(envelope(&r, r.length_ms) < 1e-6);
    }

    /// The designer's listening files (by hand: `cargo test ... -- --ignored
    /// write_the_listening_files`, BISE_SOUNDS_DIR=<a folder>): gone.wav,
    /// back.wav, and a 6 s sequence (gone, 2 s of silence, a stand-in for
    /// a short line, back). Writes files only; plays nothing.
    #[test]
    #[ignore]
    fn write_the_listening_files() {
        let dir = std::path::PathBuf::from(std::env::var("BISE_SOUNDS_DIR").expect("BISE_SOUNDS_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        let (gone, back) = (pcm(Sound::Gone), pcm(Sound::Back));
        std::fs::write(dir.join("gone.wav"), wav(&gone, TTS_RATE)).unwrap();
        std::fs::write(dir.join("back.wav"), wav(&back, TTS_RATE)).unwrap();
        // BISE_SOUNDS_LINE: a 16-bit mono WAV at 24 kHz (`say -o x.wav
        // --data-format=LEI16@24000 "…"` writes one, playing nothing)
        let line: Vec<f32> = std::env::var("BISE_SOUNDS_LINE")
            .ok()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| {
                let at = b.windows(4).position(|w| w == b"data")? + 8;
                Some(b[at..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / i16::MAX as f32).collect())
            })
            .unwrap_or_default();
        let mut seq = gone.clone();
        seq.extend(std::iter::repeat_n(0.0, 2 * TTS_RATE as usize));
        seq.extend(line);
        let total = 6 * TTS_RATE as usize;
        seq.resize(total - back.len(), 0.0);
        seq.extend(back);
        std::fs::write(dir.join("sequence.wav"), wav(&seq, TTS_RATE)).unwrap();
    }
}
