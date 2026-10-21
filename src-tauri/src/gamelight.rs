//! League lighting: what the keyboard shows during a match (the customisable setup) and how
//! a game snapshot plus the playing effects turn into a frame.

use serde::{Deserialize, Serialize};

/// The moments an effect can play for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Event {
    Kill,
    Death,
    Assist,
    FirstBlood,
    Multikill,
    Ace,
    Dragon,
    Herald,
    Baron,
    Turret,
    Inhibitor,
    LevelUp,
    Win,
    Lose,
}
