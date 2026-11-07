//! Headlines from the team's mood: the round it crosses into another band, up or down. A move
//! inside a band is not news — the card shows it.

use super::{HeadlineDraft, HeadlineRule, RoundContext};
use crate::career::story::mood::{self, Band};

/// The team's view of the player changing band.
pub struct MoodShift;

impl HeadlineRule for MoodShift {
    fn kind(&self) -> &'static str {
        "mood_shift"
    }
    fn priority(&self) -> u8 {
        48
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let team = cx.facts.player_team.clone()?;
        // The whole season is replayed for each round: a handful of races, and the mood of one
        // round depends on all before it.
        let now = mood::replay(cx.facts).into_iter().find(|m| m.round == cx.round.number)?;
        let was = Band::of(now.was);
        if was == now.band {
            return None;
        }
        let key = match (now.band > was, now.band) {
            (true, Band::Delighted) => "mood.up.delighted",
            (true, Band::Content) => "mood.up.content",
            (true, _) => "mood.up.concerned",
            (false, Band::Content) => "mood.down.content",
            (false, Band::Concerned) => "mood.down.concerned",
            (false, _) => "mood.down.losing_patience",
        };
        let s = cx.round.race()?;
        let me = s.player()?;
        Some(HeadlineDraft {
            session_id: s.id.clone(),
            subject: me.name.clone(),
            player: true,
            key,
            values: vec![("team", team), ("driver", me.name.clone()), ("track", s.track.clone())],
        })
    }
}

/// The team letting it be known it is considering its options, the race the mood has spent
/// [`mood::WARNING_RACES`] in a row losing patience. Once per such run.
pub struct ConsideringOptions;

impl HeadlineRule for ConsideringOptions {
    fn kind(&self) -> &'static str {
        "considering_options"
    }
    fn priority(&self) -> u8 {
        58
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let team = cx.facts.player_team.clone()?;
        let rounds = mood::replay(cx.facts);
        let upto = rounds.iter().position(|m| m.round == cx.round.number)?;
        if mood::patience_run(&rounds[..=upto]) != mood::WARNING_RACES {
            return None;
        }
        let s = cx.round.race()?;
        let me = s.player()?;
        Some(HeadlineDraft {
            session_id: s.id.clone(),
            subject: me.name.clone(),
            player: true,
            key: "mood.considering",
            values: vec![("team", team), ("driver", me.name.clone()), ("track", s.track.clone())],
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/career/story/headlines/mood.rs"]
mod tests;
