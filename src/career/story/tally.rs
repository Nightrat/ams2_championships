//! Where a season stands after a given round: the standings, the player's own season, and the
//! player's career up to that point.
//!
//! Built a round at a time by whoever walks a season — the news feed keeps one before and one
//! after each round — and read by every rule that asks "what had happened by then". It holds
//! nothing a rule could not recount from the facts; it exists so that no rule has to.

use std::collections::BTreeMap;

use super::facts::{DriverResult, PlayerHistory, RoundFacts, SeasonFacts, SessionKind};
use super::rival::Rivalry;
use crate::career::data_store::{standing_order, Countback};

/// The running tally of a season, as of some round.
#[derive(Clone, Debug, Default)]
pub struct SeasonSoFar {
    /// The player's career up to here: what came before the season, plus the season so far.
    pub career: PlayerHistory,
    /// The player's season up to here.
    pub season: PlayerSeason,
    /// Races run so far, whether or not the player was in them.
    pub races: u32,
    /// Rounds that held a race — what a calendar counts, the same test the salary uses.
    pub rounds_raced: u32,
    /// Points, wins and countback per driver, keyed by [`driver_key`].
    standings: BTreeMap<String, Standing>,
    /// Who the player's rival is, replayed with everything else.
    pub rivalry: Rivalry,
}

/// The player's season so far.
#[derive(Clone, Debug, Default)]
pub struct PlayerSeason {
    pub races: u32,
    pub best_finish: Option<u32>,
    /// Consecutive races scored in, up to the latest the player started.
    pub points_streak: u32,
    /// Consecutive podiums, the same way.
    pub podium_streak: u32,
    pub retirements: u32,
}

/// One driver's championship so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Standing {
    /// As the latest result spelled it.
    pub name: String,
    pub points: i32,
    pub player: bool,
    /// Classified finishes, for the FIA countback — see [`Countback`].
    pub(crate) countback: Countback,
}

impl Standing {
    pub fn wins(&self) -> u32 {
        self.countback.wins()
    }
}

/// How the tally knows a driver: the player is one entry whatever their profile is called that
/// weekend, everyone else is their name.
pub fn driver_key(r: &DriverResult) -> String {
    if r.is_player {
        "\u{0}player".to_string()
    } else {
        r.name.clone()
    }
}

impl SeasonSoFar {
    /// The tally before the season's first round: nothing yet, on top of the player's history.
    pub fn start(facts: &SeasonFacts) -> Self {
        Self {
            career: facts.history.clone(),
            rivalry: Rivalry::new(facts),
            ..Self::default()
        }
    }

    /// The tally after every round of `facts`.
    pub fn whole(facts: &SeasonFacts) -> Self {
        let mut so_far = Self::start(facts);
        facts.rounds.iter().for_each(|r| so_far.record(r));
        so_far
    }

    /// Adds one round.
    pub fn record(&mut self, round: &RoundFacts) {
        self.rounds_raced += u32::from(round.race().is_some());
        for s in &round.sessions {
            self.career.record(s);
            if s.kind != SessionKind::Race {
                continue;
            }
            self.races += 1;
            for r in &s.results {
                let st = self.standings.entry(driver_key(r)).or_default();
                st.name = r.name.clone();
                st.player = r.is_player;
                st.points += r.points;
                if !r.dnf {
                    st.countback.record(r.position);
                }
            }
            if let Some(me) = s.player() {
                let p = &mut self.season;
                p.races += 1;
                p.retirements += u32::from(me.dnf);
                let podium = !me.dnf && (1..=3).contains(&me.position);
                p.points_streak = if me.points > 0 { p.points_streak + 1 } else { 0 };
                p.podium_streak = if podium { p.podium_streak + 1 } else { 0 };
                if !me.dnf {
                    p.best_finish = Some(p.best_finish.map_or(me.position, |b| b.min(me.position)));
                }
            }
        }
        self.rivalry.record(round, &self.standings);
    }

    /// The standings in order — through the same comparator as the Career tab's table, so the
    /// two can never disagree about who is where.
    pub fn table(&self) -> Vec<&Standing> {
        let mut table: Vec<&Standing> = self.standings.values().collect();
        table.sort_by(|a, b| standing_order((a.points, &a.countback, &a.name), (b.points, &b.countback, &b.name)));
        table
    }

    /// The player's place in the standings, 1-based.
    pub fn player_position(&self) -> Option<u32> {
        self.table().iter().position(|s| s.player).map(|i| i as u32 + 1)
    }

    /// The championship leader, when one driver is ahead alone on points. A tie for the lead
    /// is no leader: the feed does not announce a lead the countback alone decides.
    pub fn leader(&self) -> Option<&Standing> {
        match self.table().as_slice() {
            [first, second, ..] if first.points == second.points => None,
            [first, ..] if first.points > 0 => Some(first),
            _ => None,
        }
    }

    /// How far the leader is ahead of second place; 0 without a leader.
    pub fn lead(&self) -> i32 {
        match self.table().as_slice() {
            [first, second, ..] if self.leader().is_some() => first.points - second.points,
            [first] if self.leader().is_some() => first.points,
            _ => 0,
        }
    }

    /// Wins so far for the driver keyed `key`.
    pub fn wins(&self, key: &str) -> u32 {
        self.standings.get(key).map_or(0, |s| s.wins())
    }
}

#[cfg(test)]
#[path = "../../tests/career/story/tally.rs"]
mod tests;
