//! Headlines about the rest of the grid: who leads, who wins, who surprises.
//!
//! The player's own wins and lead changes are [`super::player`]'s news, so these rules look at
//! everyone else — a feed that only ever talked about the player would not be a paddock.

use super::{points, HeadlineDraft, HeadlineRule, RoundContext};
use crate::career::story::tally::driver_key;
use crate::career::story::facts::{DriverResult, SessionFacts};
use crate::format::ordinal;

/// The winner of the round's race, when it is not the player.
fn ai_winner<'a>(cx: &RoundContext<'a>) -> Option<(&'a SessionFacts, &'a DriverResult)> {
    let s = cx.round.race()?;
    let w = s.results.iter().find(|r| !r.dnf && r.position == 1 && !r.is_player)?;
    Some((s, w))
}

fn draft(s: &SessionFacts, subject: &str, key: &'static str, mut values: Vec<(&'static str, String)>) -> HeadlineDraft {
    values.push(("driver", subject.to_string()));
    values.push(("track", s.track.clone()));
    HeadlineDraft {
        session_id: s.id.clone(),
        subject: subject.to_string(),
        player: false,
        key,
        values,
    }
}

/// Someone other than the player taking over the championship lead.
pub struct LeadChange;

impl HeadlineRule for LeadChange {
    fn kind(&self) -> &'static str {
        "lead_change"
    }
    fn priority(&self) -> u8 {
        65
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let previous = cx.before.leader()?;
        let now = cx.after.leader().filter(|l| !l.player)?;
        if now.name == previous.name && now.player == previous.player {
            return None;
        }
        let s = cx.round.race()?;
        Some(draft(
            s,
            &now.name,
            "lead.taken",
            vec![("previous", previous.name.clone()), ("gap", points(cx.after.lead()))],
        ))
    }
}

/// A driver's first win of the season, once the season has had a race to win.
pub struct FirstWinOfSeason;

impl HeadlineRule for FirstWinOfSeason {
    fn kind(&self) -> &'static str {
        "first_win_of_season"
    }
    fn priority(&self) -> u8 {
        35
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        if cx.before.races == 0 {
            return None;
        }
        let (s, w) = ai_winner(cx)?;
        if cx.before.wins(&driver_key(w)) > 0 {
            return None;
        }
        Some(draft(s, &w.name, "win.first_of_season", vec![]))
    }
}

/// A race won by someone who still is not leading the championship.
pub struct WinnerNotLeader;

impl HeadlineRule for WinnerNotLeader {
    fn kind(&self) -> &'static str {
        "winner_not_leader"
    }
    fn priority(&self) -> u8 {
        30
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let (s, w) = ai_winner(cx)?;
        let leader = cx.after.leader()?;
        if !leader.player && leader.name == w.name {
            return None;
        }
        Some(draft(
            s,
            &w.name,
            "win.not_leader",
            vec![("leader", leader.name.clone()), ("gap", points(cx.after.lead()))],
        ))
    }
}

/// A driver finishing far ahead of where their car is expected to — the biggest such surprise
/// of the race, if any is big enough. Needs the season's expected positions, so a season with no
/// roster has no upsets.
pub struct Upset;

/// How many places better than expected a finish must be to count as an upset. Expected positions
/// are a team's average, so a few places either way is noise.
const UPSET_PLACES: f32 = 5.0;

impl HeadlineRule for Upset {
    fn kind(&self) -> &'static str {
        "upset"
    }
    fn priority(&self) -> u8 {
        25
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let s = cx.round.race()?;
        let (r, team, expected) = s
            .results
            .iter()
            .filter(|r| !r.dnf && !r.is_player)
            .filter_map(|r| {
                let team = &r.roster.as_ref()?.team;
                let expected = *cx.facts.expected.get(team)?;
                (expected - r.position as f32 >= UPSET_PLACES).then_some((r, team, expected))
            })
            // Biggest margin; the better finish, then the name, settle a tie.
            .max_by(|a, b| {
                let margin = |x: &(&DriverResult, &String, f32)| x.2 - x.0.position as f32;
                margin(a)
                    .total_cmp(&margin(b))
                    .then(b.0.position.cmp(&a.0.position))
                    .then(b.0.name.cmp(&a.0.name))
            })?;
        Some(draft(
            s,
            &r.name,
            "upset",
            vec![
                ("team", team.clone()),
                ("pos", ordinal(r.position)),
                ("expected", ordinal(expected.round().max(1.0) as u32)),
            ],
        ))
    }
}

#[cfg(test)]
#[path = "../../../tests/career/story/headlines/field.rs"]
mod tests;
