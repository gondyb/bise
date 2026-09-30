//! Tier 5, the checker (design §4): the seam between the hub's gate
//! (1a, `daemon/gate.rs`) and the `checker` role (1d).
//!
//! The hub keeps one [`Runner`] for its whole life, in an `Arc`: it calls
//! [`Runner::sync`] before `judge` on each gate line (true: the checker
//! changed, every repo's `Cache` is cleared), [`Runner::checker`] to know
//! which one is on, [`Runner::check`] on its own thread for a
//! `Verdict::Check` (blocking, at most ~5 s), and [`Runner::take_notice`]
//! after each check (a line for main's feed).
//!
//! This is 1a's stub: the checker is off, so every tier-5 call is a card
//! (`card_when_off`). 1d fills the bodies.

use super::{CacheKey, Call, Checker, Part};

/// What the checker judges: the parts left at tier 5.
#[derive(Clone, Debug)]
pub struct CheckReq {
    pub call: Call,
    pub parts: Vec<Part>,
    pub keys: Vec<CacheKey>,
    /// The user's words behind the task, uncut (the checker cuts them).
    pub task: String,
    /// Left `None` by the hub: the checker reads a script it runs itself.
    pub script: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckOut {
    /// Runs; `cache`: the keys the hub caches for this repo.
    Allow { cache: Vec<CacheKey> },
    /// A card: `reason` in the designer's words, `detail` the scores
    /// (the debug log and ctrl+o, never the card's words).
    Card { reason: String, detail: String },
}

/// The checker of this hub: which one, its error count and cool-down.
#[derive(Debug, Default)]
pub struct Runner {}

impl Runner {
    pub fn new(_home: &bise_home::Home) -> Runner {
        Runner {}
    }

    /// Re-read what picks the checker (config.toml, auth.json) when it
    /// changed; true when the checker changed.
    pub fn sync(&self) -> bool {
        false
    }

    /// Which checker is on.
    pub fn checker(&self) -> Checker {
        Checker::Off
    }

    /// Judge the parts left (blocking).
    pub fn check(&self, _req: &CheckReq) -> CheckOut {
        CheckOut::Card {
            reason: "i couldn't check this one, so i'm asking.".into(),
            detail: "stub".into(),
        }
    }

    /// The one line for main's feed after 3 errors in a row.
    pub fn take_notice(&self) -> Option<String> {
        None
    }
}
