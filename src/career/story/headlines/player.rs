//! Headlines about the player: wins, podiums, poles, firsts, streaks and setbacks.
//!
//! "First" means first of the *career* where the history can say so (a maiden win, a first
//! podium, a first pole, first points) and first of the *season* where it cannot be anything else
//! (a first retirement). Each rule reads only its own session and the tally, so a weekend that
//! makes several kinds of news makes several headlines, and the walker's caps decide which show.

use super::{points, result_of, HeadlineDraft, HeadlineRule, RoundContext};
use crate::career::story::facts::{DriverResult, SessionFacts};
use crate::format::ordinal;

/// The player's row in the round's race, with the race.
fn race<'a>(cx: &RoundContext<'a>) -> Option<(&'a SessionFacts, &'a DriverResult)> {
    let s = cx.round.race()?;
    Some((s, s.player()?))
}

/// A draft about the player in session `s`.
fn draft(s: &SessionFacts, me: &DriverResult, key: &'static str, mut values: Vec<(&'static str, String)>) -> HeadlineDraft {
    values.push(("driver", me.name.clone()));
    values.push(("track", s.track.clone()));
    HeadlineDraft {
        session_id: s.id.clone(),
        subject: me.name.clone(),
        player: true,
        key,
        values,
    }
}

fn classified_in(me: &DriverResult, places: std::ops::RangeInclusive<u32>) -> bool {
    !me.dnf && places.contains(&me.position)
}

/// A race win — the biggest news there is, and bigger still the first time.
pub struct Win;

impl HeadlineRule for Win {
    fn kind(&self) -> &'static str {
        "player_win"
    }
    fn priority(&self) -> u8 {
        100
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let (s, me) = race(cx)?;
        if !classified_in(me, 1..=1) {
            return None;
        }
        let key = if cx.before.career.wins == 0 { "win.maiden" } else { "win" };
        Some(draft(s, me, key, vec![]))
    }
}

/// The player going top of the championship, from someone else.
pub struct LeadTaken;

impl HeadlineRule for LeadTaken {
    fn kind(&self) -> &'static str {
        "player_lead"
    }
    fn priority(&self) -> u8 {
        90
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let previous = cx.before.leader().filter(|l| !l.player)?;
        cx.after.leader().filter(|l| l.player)?;
        let (s, me) = race(cx)?;
        Some(draft(
            s,
            me,
            "lead.taken",
            vec![("previous", previous.name.clone()), ("gap", points(cx.after.lead()))],
        ))
    }
}

/// Second or third. A win is [`Win`]'s news, not this.
pub struct Podium;

impl HeadlineRule for Podium {
    fn kind(&self) -> &'static str {
        "player_podium"
    }
    fn priority(&self) -> u8 {
        80
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let (s, me) = race(cx)?;
        if !classified_in(me, 2..=3) {
            return None;
        }
        let key = if cx.before.career.podiums == 0 { "podium.maiden" } else { "podium" };
        Some(draft(s, me, key, vec![("pos", ordinal(me.position))]))
    }
}

/// The first points of the career, when they come outside the podium (a podium says it louder).
pub struct FirstPoints;

impl HeadlineRule for FirstPoints {
    fn kind(&self) -> &'static str {
        "player_first_points"
    }
    fn priority(&self) -> u8 {
        75
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let (s, me) = race(cx)?;
        if cx.before.career.scored || me.points <= 0 || me.position <= 3 {
            return None;
        }
        Some(draft(s, me, "points.first", vec![("pos", ordinal(me.position))]))
    }
}

/// Pole position.
pub struct Pole;

impl HeadlineRule for Pole {
    fn kind(&self) -> &'static str {
        "player_pole"
    }
    fn priority(&self) -> u8 {
        70
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let s = cx.round.qualifying()?;
        let me = s.player()?;
        if me.position != 1 {
            return None;
        }
        let key = if cx.before.career.poles == 0 { "pole.maiden" } else { "pole" };
        Some(draft(s, me, key, vec![]))
    }
}

/// The first race of a career, or the first for a new team.
pub struct Debut;

impl HeadlineRule for Debut {
    fn kind(&self) -> &'static str {
        "player_debut"
    }
    fn priority(&self) -> u8 {
        60
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        if cx.before.season.races > 0 {
            return None;
        }
        let (s, me) = race(cx)?;
        let result = vec![("result", result_of(me))];
        if cx.before.career.starts == 0 {
            return Some(draft(s, me, "debut.career", result));
        }
        let team = cx.facts.player_team.as_deref()?;
        let last = cx.facts.history.last_team.as_deref()?;
        if team == last {
            return None;
        }
        let mut values = result;
        values.push(("team", team.to_string()));
        Some(draft(s, me, "debut.team", values))
    }
}

/// Podiums in a row reaching a mark. The second in a row is just a podium.
pub struct PodiumStreak;

/// The run lengths worth a headline.
const PODIUM_MARKS: &[u32] = &[3, 5, 8, 12];

impl HeadlineRule for PodiumStreak {
    fn kind(&self) -> &'static str {
        "player_podium_streak"
    }
    fn priority(&self) -> u8 {
        55
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        streak(cx, cx.after.season.podium_streak, cx.before.season.podium_streak, PODIUM_MARKS, "streak.podium")
    }
}

/// Points finishes in a row reaching a mark.
pub struct PointsStreak;

/// The run lengths worth a headline.
const POINTS_MARKS: &[u32] = &[3, 5, 8, 12, 20];

impl HeadlineRule for PointsStreak {
    fn kind(&self) -> &'static str {
        "player_points_streak"
    }
    fn priority(&self) -> u8 {
        45
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        streak(cx, cx.after.season.points_streak, cx.before.season.points_streak, POINTS_MARKS, "streak.points")
    }
}

/// A streak headline when the run has just reached one of `marks`.
fn streak(cx: &RoundContext, now: u32, was: u32, marks: &[u32], key: &'static str) -> Option<HeadlineDraft> {
    if !marks.iter().any(|m| was < *m && now >= *m) {
        return None;
    }
    let (s, me) = race(cx)?;
    Some(draft(s, me, key, vec![("count", now.to_string()), ("nth", ordinal(now))]))
}

/// The player's best finish of the season so far, below the podium (above it, [`Podium`] says
/// so) and not on the first race (anything is a best then).
pub struct BestResult;

impl HeadlineRule for BestResult {
    fn kind(&self) -> &'static str {
        "player_best"
    }
    fn priority(&self) -> u8 {
        50
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let (s, me) = race(cx)?;
        let best = cx.before.season.best_finish?;
        let first_points = !cx.before.career.scored && me.points > 0;
        if me.dnf || me.position >= best || me.position <= 3 || first_points {
            return None;
        }
        Some(draft(s, me, "best", vec![("pos", ordinal(me.position))]))
    }
}

/// The first retirement of the season.
pub struct FirstRetirement;

impl HeadlineRule for FirstRetirement {
    fn kind(&self) -> &'static str {
        "player_first_dnf"
    }
    fn priority(&self) -> u8 {
        40
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let (s, me) = race(cx)?;
        if !me.dnf || cx.before.season.retirements > 0 {
            return None;
        }
        Some(draft(s, me, "dnf.first", vec![]))
    }
}

#[cfg(test)]
#[path = "../../../tests/career/story/headlines/player.rs"]
mod tests;
