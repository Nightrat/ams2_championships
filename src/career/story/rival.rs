//! The rival: the driver the season keeps putting next to the player.
//!
//! Nobody picks a rival from a menu. Each race, every other driver earns weight for finishing
//! close to the player and for being close in the standings; weight fades race by race, so the
//! recent races count most. The heaviest becomes the rival — but only once the season has had a
//! couple of races, and a rival is replaced only when a challenger has out-weighed them by a
//! margin for several races running, so the name does not flip with every result.
//!
//! Nothing is stored: [`Rivalry`] is replayed round by round as part of the season's tally,
//! which is also what lets the news feed see the moment the rival changes.
//!
//! Teammates are left out — they are measured against the player already, and the car makes
//! them close by default. In multiplayer the rival may well be a person.

use std::collections::BTreeMap;

use serde::Serialize;

use super::facts::{RoundFacts, SeasonFacts};
use super::tally::{driver_key, Standing};
use super::teammate;

/// How much of a driver's weight survives each race: the recent races count most.
const DECAY: f32 = 0.75;
/// Finishing this many places or fewer from the player earns weight, more the closer it was.
const PROXIMITY_PLACES: u32 = 3;
/// Most weight being level on points earns in one race; it fades to nothing two races' worth of
/// points apart.
const STANDINGS_WEIGHT: f32 = 2.0;
/// Races the season must have run before anyone is called a rival — one race is a coincidence.
const MIN_RACES: u32 = 2;
/// How far a challenger must out-weigh the rival to count as overtaking them.
const SWITCH_MARGIN: f32 = 1.0;
/// Races running a challenger must stay ahead by that margin before taking over.
const SWITCH_RACES: u32 = 2;

/// The player against one driver, over the races both started.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Duel {
    pub races: u32,
    /// Races the player finished ahead.
    pub player: u32,
    /// Races the other driver finished ahead. Two retirements count for neither.
    pub rival: u32,
}

/// One driver who could be the rival.
#[derive(Clone, Debug, Default, PartialEq)]
struct Candidate {
    name: String,
    team: Option<String>,
    weight: f32,
    duel: Duel,
}

/// The rival as of some round.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Rival {
    pub name: String,
    pub team: Option<String>,
    /// The round they became the rival.
    pub since_round: usize,
    pub duel: Duel,
}

/// The rivalry replayed up to some round. Lives in the season's tally.
#[derive(Clone, Debug, Default)]
pub struct Rivalry {
    candidates: BTreeMap<String, Candidate>,
    /// The rival's key and the round they took over.
    current: Option<(String, usize)>,
    /// A driver out-weighing the rival, and for how many races running.
    challenger: Option<(String, u32)>,
    races: u32,
    /// Roster names of the player's teammates, who are never the rival.
    teammates: Vec<String>,
    /// The most one race scores, which scales how close "close in the standings" is.
    per_race: i32,
}

impl Rivalry {
    pub fn new(facts: &SeasonFacts) -> Self {
        Self {
            teammates: teammate::teammates(facts).into_iter().map(String::from).collect(),
            per_race: facts.points_system.iter().copied().max().unwrap_or(0),
            ..Self::default()
        }
    }

    /// Adds one round, with the standings as they stand after it. A round with no race, or one
    /// the player did not start, changes nothing.
    pub fn record(&mut self, round: &RoundFacts, standings: &BTreeMap<String, Standing>) {
        let Some(race) = round.race() else {
            return;
        };
        let Some(me) = race.player() else {
            return;
        };
        self.races += 1;
        self.candidates.values_mut().for_each(|c| c.weight *= DECAY);
        let my_points = standings.get(&driver_key(me)).map_or(0, |s| s.points);
        for r in race.results.iter().filter(|r| !r.is_player) {
            if r.roster.as_ref().is_some_and(|d| self.teammates.contains(&d.name)) {
                continue;
            }
            let c = self.candidates.entry(driver_key(r)).or_default();
            c.name = r.name.clone();
            c.team = r.roster.as_ref().map(|d| d.team.clone()).or(c.team.take());
            c.weight += proximity(me.position, r.position, me.dnf || r.dnf);
            if let (Some(theirs), true) = (standings.get(&driver_key(r)), self.per_race > 0) {
                let gap = (my_points - theirs.points).abs() as f32;
                c.weight += STANDINGS_WEIGHT * (1.0 - gap / (2.0 * self.per_race as f32)).max(0.0);
            }
            c.duel.races += 1;
            match me.beats(r) {
                Some(true) => c.duel.player += 1,
                Some(false) => c.duel.rival += 1,
                None => {}
            }
        }
        self.settle(round.number);
    }

    /// Decides who the rival is after a race.
    fn settle(&mut self, round: usize) {
        if self.races < MIN_RACES {
            return;
        }
        // Heaviest first; the name settles an exact tie so the answer never depends on map order.
        let Some((top, top_weight)) = self
            .candidates
            .iter()
            .filter(|(_, c)| c.weight > 0.0)
            .max_by(|a, b| a.1.weight.total_cmp(&b.1.weight).then(b.0.cmp(a.0)))
            .map(|(k, c)| (k.clone(), c.weight))
        else {
            return;
        };
        let Some((current, _)) = &self.current else {
            self.current = Some((top, round));
            return;
        };
        if *current == top {
            self.challenger = None;
            return;
        }
        let current_weight = self.candidates.get(current).map_or(0.0, |c| c.weight);
        if top_weight < current_weight + SWITCH_MARGIN {
            self.challenger = None;
            return;
        }
        let runs = match &self.challenger {
            Some((who, n)) if *who == top => n + 1,
            _ => 1,
        };
        if runs >= SWITCH_RACES {
            self.current = Some((top, round));
            self.challenger = None;
        } else {
            self.challenger = Some((top, runs));
        }
    }

    /// The rival, if the season has one yet.
    pub fn current(&self) -> Option<Rival> {
        let (key, since) = self.current.as_ref()?;
        let c = self.candidates.get(key)?;
        Some(Rival {
            name: c.name.clone(),
            team: c.team.clone(),
            since_round: *since,
            duel: c.duel.clone(),
        })
    }
}

/// Weight for finishing near the player: 3 for the next place, 2 for two apart, 1 for three,
/// nothing further away or when either retired.
fn proximity(mine: u32, theirs: u32, either_out: bool) -> f32 {
    if either_out {
        return 0.0;
    }
    (PROXIMITY_PLACES + 1).saturating_sub(mine.abs_diff(theirs)) as f32
}

#[cfg(test)]
#[path = "../../tests/career/story/rival.rs"]
mod tests;
