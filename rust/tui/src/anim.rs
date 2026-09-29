//! The animation clock (BISE-204): every animation of the TUI reads its
//! frame from the time since the clock's start, never from a count of
//! loop turns, draws or events. More input, a faster terminal or a
//! backlog of hub lines draws more often, and each draw shows the frame
//! of its time: the speed stays the same.
//!
//! - the gust (header, divider, panel cells; [`crate::gust`]): a frame
//!   every [`crate::gust::MOTIF`]`.frame_ms`, [`Clock::gust`];
//! - the tick pulses (`∿` of a running tool, fold and compacting rows,
//!   `·` of a starting agent; [`crate::theme::working_frame`]): a tick
//!   every [`PULSE_MS`] of [`Clock::pulse_ms`]. They hold still while you
//!   type (zen, BISE-121): the held time is left out, so the pulse goes
//!   on where it stopped.
//!
//! - the voice chip's blink and wave (voice/chip.rs, BISE-222): the
//!   pulses' time in ms, [`Clock::pulse_ms`]; they hold in zen too.
//!
//! The zen fade, the ctrl hints' delay, the voice chip's timer, the tips
//! and the onboarding read `Instant`s of their own.

use std::time::{Duration, Instant};

/// One tick of the pulses: the loop's old idle turn, so a pulse phase
/// (4 ticks) stays ~320 ms.
pub(crate) const PULSE_MS: u64 = 80;

#[derive(Debug, Default)]
pub(crate) struct Clock {
    /// the first reading (the TUI's start, in practice)
    start: Option<Instant>,
    /// the pulses' time held so far (zen), and since when they hold now
    held: Duration,
    hold_from: Option<Instant>,
}

impl Clock {
    fn since_start(&mut self, now: Instant) -> Duration {
        now.saturating_duration_since(*self.start.get_or_insert(now))
    }

    /// The gust's frame at `now`.
    pub(crate) fn gust(&mut self, now: Instant) -> u64 {
        crate::gust::frame(self.since_start(now))
    }

    /// The pulses' tick at `now`; `hold` (zen) keeps it where it is.
    /// `ended` is when the hold stopped (zen's end, [`crate::zen::Zen::end`]):
    /// the first reading after it may come a loop turn later, the held
    /// time does not count that turn.
    /// (The draw loop reads [`Clock::pulse_ms`] and divides.)
    #[cfg(test)]
    pub(crate) fn pulse(&mut self, now: Instant, hold: bool, ended: Option<Instant>) -> u32 {
        (self.pulse_ms(now, hold, ended) / PULSE_MS) as u32
    }

    /// The pulses' time at `now` in ms, the held time left out (the
    /// voice chip's blink and wave, voice/chip.rs); same `hold` and
    /// `ended` as [`Clock::pulse`], which reads it.
    pub(crate) fn pulse_ms(&mut self, now: Instant, hold: bool, ended: Option<Instant>) -> u64 {
        let run = self.since_start(now);
        match (hold, self.hold_from) {
            (true, None) => self.hold_from = Some(now),
            (false, Some(h)) => {
                let stop = ended.unwrap_or(now).clamp(h, now);
                self.held += stop.saturating_duration_since(h);
                self.hold_from = None;
            }
            _ => {}
        }
        let at = match self.hold_from {
            Some(h) => h.saturating_duration_since(self.start.unwrap_or(h)),
            None => run,
        };
        at.saturating_sub(self.held).as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(t: Instant, n: u64) -> Instant {
        t + Duration::from_millis(n)
    }

    #[test]
    fn same_time_same_frame_whatever_the_number_of_turns() {
        let t = Instant::now();
        // one reading at 2 s, or a turn every 1 ms (a flood of events,
        // a fast terminal), or every 80 ms: the same frames at 2 s
        let mut once = Clock::default();
        let (mut busy, mut idle) = (Clock::default(), Clock::default());
        once.gust(t);
        once.pulse(t, false, None);
        for n in 0..2000 {
            busy.gust(ms(t, n));
            busy.pulse(ms(t, n), false, None);
        }
        for n in (0..2000).step_by(80) {
            idle.gust(ms(t, n));
            idle.pulse(ms(t, n), false, None);
        }
        let end = ms(t, 2000);
        let want = (once.gust(end), once.pulse(end, false, None));
        assert_eq!(want, (2000 / crate::gust::MOTIF.frame_ms, (2000 / PULSE_MS) as u32));
        assert_eq!((busy.gust(end), busy.pulse(end, false, None)), want);
        assert_eq!((idle.gust(end), idle.pulse(end, false, None)), want);
    }

    #[test]
    fn the_pulse_holds_while_zen_and_goes_on_where_it_stopped() {
        let t = Instant::now();
        let mut c = Clock::default();
        assert_eq!(c.pulse(t, false, None), 0);
        assert_eq!(c.pulse(ms(t, 400), false, None), 5);
        // zen from 400 ms to 1400 ms: however many turns, it stays
        for n in (400..1400).step_by(7) {
            assert_eq!(c.pulse(ms(t, n), true, None), 5);
        }
        // zen ended at 1400 ms, seen at 1800 ms (or at 1401 ms: the
        // same): 1 s held is left out, 1800 ms of time, 800 ms of pulse
        let mut soon = Clock::default();
        soon.pulse(t, false, None);
        soon.pulse(ms(t, 400), true, None);
        assert_eq!(soon.pulse(ms(t, 1401), false, Some(ms(t, 1400))), 5);
        assert_eq!(soon.pulse(ms(t, 1800), false, None), 10);
        assert_eq!(c.pulse(ms(t, 1800), false, Some(ms(t, 1400))), 10);
        // the gust does not hold (zen calms it instead, gust::motion)
        assert_eq!(c.gust(ms(t, 1800)), 1800 / crate::gust::MOTIF.frame_ms);
    }

    #[test]
    fn the_voice_chip_reads_the_pulse_time_and_holds_in_zen() {
        let t = Instant::now();
        let mut c = Clock::default();
        assert_eq!(c.pulse_ms(t, false, None), 0);
        // the wave's 120 ms and the blink's 600 ms, not a count of turns
        assert_eq!(c.pulse_ms(ms(t, 121), false, None), 121);
        assert_eq!(c.pulse_ms(ms(t, 600), false, None), 600);
        // typing (zen) from 600 ms to 1000 ms: held, then on where it was
        assert_eq!(c.pulse_ms(ms(t, 600), true, None), 600);
        assert_eq!(c.pulse_ms(ms(t, 900), true, None), 600);
        assert_eq!(c.pulse_ms(ms(t, 1100), false, Some(ms(t, 1000))), 700);
        // the pulse is the same time in ticks
        assert_eq!(c.pulse(ms(t, 1100), false, None), (700 / PULSE_MS) as u32);
    }
}
