//! The turn controller (owner: voice-mode, the lead; plan §4.4): who
//! talks when. Driven by the run loop's ticks, the keys, the mic blocks,
//! the listener's words, the agent's feed events and the speaker's
//! clock; tells the app what to do ([`Act`]) and what the pane shows
//! ([`super::PaneView`]). Pure over its ports: the tests use the fakes.
//! Stub: filled by the lead.

/// What the controller asks of the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// your turn, said: send `text` to `agent` (the `sb` input, `voice: true`)
    Send { agent: String, text: String },
    /// you cut in while `agent`'s turn still runs
    Interrupt { agent: String },
    /// a faint line in the thread (`· voice mode · 14:02`)
    Note(String),
    /// voice mode closed itself (the mic failed for good)
    Leave(String),
}
