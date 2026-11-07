//! Headlines from the rivalry: a new rival emerging, and the head-to-head with them changing
//! hands. Both compare the rivalry before the round with after it, so each fires on the round it
//! happened and not again.

use super::{HeadlineDraft, HeadlineRule, RoundContext};
use crate::career::story::facts::SessionFacts;

/// The player's name and the round's race, which every rivalry headline is about.
fn race<'a>(cx: &RoundContext<'a>) -> Option<(&'a SessionFacts, String)> {
    let s = cx.round.race()?;
    Some((s, s.player()?.name.clone()))
}

/// Someone becoming the player's rival — the first, or a new one.
pub struct NewRival;

impl HeadlineRule for NewRival {
    fn kind(&self) -> &'static str {
        "new_rival"
    }
    fn priority(&self) -> u8 {
        52
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let now = cx.after.rivalry.current()?;
        let was = cx.before.rivalry.current();
        if was.as_ref().is_some_and(|w| w.name == now.name) {
            return None;
        }
        let (s, player) = race(cx)?;
        let mut values = vec![("driver", now.name.clone()), ("player", player)];
        let key = match was {
            Some(w) => {
                values.push(("previous", w.name));
                "rival.replaced"
            }
            None => "rival.new",
        };
        Some(HeadlineDraft {
            session_id: s.id.clone(),
            subject: now.name,
            player: true,
            key,
            values,
        })
    }
}

/// The head-to-head with the same rival changing hands.
pub struct RivalDuelTurns;

impl HeadlineRule for RivalDuelTurns {
    fn kind(&self) -> &'static str {
        "rival_duel"
    }
    fn priority(&self) -> u8 {
        42
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let was = cx.before.rivalry.current()?;
        let now = cx.after.rivalry.current().filter(|n| n.name == was.name)?;
        let lead = |d: &crate::career::story::rival::Duel| d.player.cmp(&d.rival);
        use std::cmp::Ordering::*;
        let key = match (lead(&was.duel), lead(&now.duel)) {
            (Less | Equal, Greater) => "rival.duel.ahead",
            (Greater | Equal, Less) => "rival.duel.behind",
            _ => return None,
        };
        let (s, player) = race(cx)?;
        Some(HeadlineDraft {
            session_id: s.id.clone(),
            subject: now.name.clone(),
            player: true,
            key,
            values: vec![
                ("driver", now.name.clone()),
                ("player", player),
                ("won", now.duel.player.to_string()),
                ("lost", now.duel.rival.to_string()),
                ("track", s.track.clone()),
            ],
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/career/story/headlines/rival.rs"]
mod tests;
