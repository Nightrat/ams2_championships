//! The teammate head-to-head: the player against the other car in the same garage.
//!
//! The one comparison in racing that the car cannot explain away — both drivers had the same
//! machinery, so whoever is ahead is ahead on merit. It is therefore the first yardstick a team
//! judges a driver by, and the first one the story shows.
//!
//! A **teammate** is a driver who holds a seat in the player's team for the season: a regular
//! roster entry of [`SeasonFacts::player_team`]. Not a per-track stand-in (they are there for one
//! weekend, and nobody is measured against a substitute), and not the driver the player replaced
//! (the player is in their car).
//!
//! Only sessions both drove count, so a teammate the grid left out of a session costs neither side
//! anything.

use serde::Serialize;

use super::facts::{DriverResult, SeasonFacts, SessionKind};

/// A figure for the player and the same figure for the teammate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Pair<T> {
    pub player: T,
    pub teammate: T,
}

/// The player against one teammate, over the sessions both drove.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HeadToHead {
    /// The teammate's name as the roster gives it.
    pub teammate: String,
    /// Qualifying sessions both drove.
    pub qualifyings: u32,
    /// Who out-qualified whom.
    pub qualifying: Pair<u32>,
    /// Races both started.
    pub races: u32,
    /// Who finished ahead. A race both retired from counts for neither, so the two need not add
    /// up to [`Self::races`].
    pub race: Pair<u32>,
    /// Points scored in the races both started.
    pub points: Pair<i32>,
    /// Retirements in the races both started.
    pub retirements: Pair<u32>,
    /// Average classified finish in the races both started; `None` with no classified finish.
    pub average_finish: Pair<Option<f32>>,
}

/// The player against each teammate, in roster order. Empty when the season has no team, or the
/// team fields nobody but the player.
pub fn head_to_head(facts: &SeasonFacts) -> Vec<HeadToHead> {
    teammates(facts)
        .into_iter()
        .map(|name| against(facts, name))
        .collect()
}

/// The teammates' roster names, in file order. A driver listed for two cars of the team (a
/// second livery) is one teammate.
pub fn teammates(facts: &SeasonFacts) -> Vec<&str> {
    let Some(team) = facts.player_team.as_deref() else {
        return Vec::new();
    };
    let mut names: Vec<&str> = Vec::new();
    for d in &facts.roster {
        let replaced = facts.replaced.as_deref() == Some(d.name.as_str());
        if d.team == team && !d.stand_in && !replaced && !names.contains(&d.name.as_str()) {
            names.push(&d.name);
        }
    }
    names
}

fn against(facts: &SeasonFacts, teammate: &str) -> HeadToHead {
    let mut h = Tally::default();
    for s in facts.sessions() {
        let (Some(me), Some(them)) = (s.player(), s.driver(teammate)) else {
            continue;
        };
        match s.kind {
            SessionKind::Qualifying => h.qualifying(me, them),
            SessionKind::Race => h.race(me, them),
        }
    }
    h.finish(teammate)
}

/// Running totals while walking the season.
#[derive(Default)]
struct Tally {
    qualifyings: u32,
    qualifying: Pair<u32>,
    races: u32,
    race: Pair<u32>,
    points: Pair<i32>,
    retirements: Pair<u32>,
    finishes: Pair<Vec<u32>>,
}

impl Tally {
    fn qualifying(&mut self, me: &DriverResult, them: &DriverResult) {
        self.qualifyings += 1;
        match me.position.cmp(&them.position) {
            std::cmp::Ordering::Less => self.qualifying.player += 1,
            std::cmp::Ordering::Greater => self.qualifying.teammate += 1,
            std::cmp::Ordering::Equal => {}
        }
    }

    fn race(&mut self, me: &DriverResult, them: &DriverResult) {
        self.races += 1;
        self.points.player += me.points;
        self.points.teammate += them.points;
        for (r, retirements, finishes) in [
            (me, &mut self.retirements.player, &mut self.finishes.player),
            (them, &mut self.retirements.teammate, &mut self.finishes.teammate),
        ] {
            if r.dnf {
                *retirements += 1;
            } else {
                finishes.push(r.position);
            }
        }
        match me.beats(them) {
            Some(true) => self.race.player += 1,
            Some(false) => self.race.teammate += 1,
            None => {}
        }
    }

    fn finish(self, teammate: &str) -> HeadToHead {
        HeadToHead {
            teammate: teammate.to_string(),
            qualifyings: self.qualifyings,
            qualifying: self.qualifying,
            races: self.races,
            race: self.race,
            points: self.points,
            retirements: self.retirements,
            average_finish: Pair {
                player: average(&self.finishes.player),
                teammate: average(&self.finishes.teammate),
            },
        }
    }
}

fn average(positions: &[u32]) -> Option<f32> {
    if positions.is_empty() {
        return None;
    }
    Some(positions.iter().sum::<u32>() as f32 / positions.len() as f32)
}

#[cfg(test)]
#[path = "../../tests/career/story/teammate.rs"]
mod tests;
