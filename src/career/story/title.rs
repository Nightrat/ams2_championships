//! The title fight: who can still win the championship, who has won it, and what the player's
//! contract asks of them.
//!
//! Everything is worked out against the season's **declared calendar** ([`SeasonFacts::planned_rounds`]),
//! because "what is still to race for" has no answer without one — rounds are added as they are
//! raced, so a season cannot otherwise say how many are left. A season with no calendar has no
//! title fight, and the service says so.
//!
//! **Clinched is never claimed on a tie that could still go the other way.** Drivers are ranked
//! through the standings' own comparator (points, then the FIA countback), and a rival is out
//! only if they cannot finish ahead even by winning every race left while the leader scores
//! nothing. A tie that only the name would settle — not an FIA rule — keeps both in the fight.
//!
//! Each remaining round is taken as one race, worth the points table's best score. The points
//! table is the whole of the scoring: there are no pole or fastest-lap points.

use serde::Serialize;

use super::facts::SeasonFacts;
use super::tally::{SeasonSoFar, Standing};

/// Where a driver stands in the fight for the title.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleStatus {
    /// The title is theirs whatever happens in the races left.
    Clinched,
    /// Can still win it.
    InContention,
    /// Cannot win it any more.
    Eliminated,
}

/// One driver in the standings, with their chances.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Contender {
    pub position: u32,
    pub name: String,
    pub player: bool,
    pub points: i32,
    /// Points behind the leader; 0 for the leader.
    pub behind: i32,
    pub status: TitleStatus,
}

/// The state of the title fight after the rounds raced so far.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TitleFight {
    /// Races the season is meant to run.
    pub planned: u32,
    /// Rounds that have held a race. May pass `planned`: the calendar is a plan, not a limit.
    pub races_run: u32,
    pub races_left: u32,
    /// The most one race can score.
    pub points_per_race: i32,
    /// The most a driver can still add: every race left, won.
    pub points_left: i32,
    /// The whole standings, in order.
    pub table: Vec<Contender>,
    /// What would settle the title at the next round, if it can be settled there.
    pub clinch_next: Option<ClinchChance>,
}

impl TitleFight {
    /// The player's row, once they have raced.
    pub fn player(&self) -> Option<&Contender> {
        self.table.iter().find(|c| c.player)
    }

    /// The leader's row.
    pub fn leader(&self) -> Option<&Contender> {
        self.table.first()
    }
}

/// The leader can win the title at the next round.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClinchChance {
    pub leader: String,
    pub player: bool,
    /// What the leader has to do against each rival still able to stop it.
    pub against: Vec<Margin>,
}

/// How the leader has to score against one rival at the next round to clinch: positive `by`
/// means outscore them by that much; zero or below means they may outscore the leader by at
/// most `-by`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Margin {
    pub rival: String,
    pub by: i32,
}

/// The title fight as of `so_far`. `None` without a calendar, or with a points table that scores
/// nothing.
pub fn fight(facts: &SeasonFacts, so_far: &SeasonSoFar) -> Option<TitleFight> {
    let planned = facts.planned_rounds?;
    let per_race = facts.points_system.iter().copied().max().filter(|p| *p > 0)?;
    let races_run = so_far.rounds_raced;
    let races_left = planned.saturating_sub(races_run);
    let table = so_far.table();
    let leader_points = table.first().map_or(0, |l| l.points);
    let statuses = statuses(&table, races_left, per_race);
    let clinch_next = clinch_next(&table, &statuses, races_left, per_race);
    Some(TitleFight {
        planned,
        races_run,
        races_left,
        points_per_race: per_race,
        points_left: races_left as i32 * per_race,
        table: table
            .iter()
            .zip(statuses)
            .enumerate()
            .map(|(i, (s, status))| Contender {
                position: i as u32 + 1,
                name: s.name.clone(),
                player: s.player,
                points: s.points,
                behind: leader_points - s.points,
                status,
            })
            .collect(),
        clinch_next,
    })
}

/// Each driver's status, in table order: everyone who cannot beat the leader is out, and the
/// leader has clinched when nobody can.
fn statuses(table: &[&Standing], races_left: u32, per_race: i32) -> Vec<TitleStatus> {
    let Some(leader) = table.first() else {
        return Vec::new();
    };
    let out: Vec<bool> = table[1..]
        .iter()
        .map(|d| cannot_beat(d.points, &d.countback, leader.points, &leader.countback, races_left, per_race))
        .collect();
    let mut all = vec![if out.iter().all(|o| *o) {
        TitleStatus::Clinched
    } else {
        TitleStatus::InContention
    }];
    all.extend(out.into_iter().map(|o| {
        if o {
            TitleStatus::Eliminated
        } else {
            TitleStatus::InContention
        }
    }));
    all
}

/// Whether a driver on `points` cannot finish ahead of one on `leader_points`, even by winning
/// every race left while the other scores nothing. Level on points, it comes down to the
/// countback — with all those wins added — and an exact tie there is *not* out: only the name
/// would separate them, and that is not a rule anyone races under.
fn cannot_beat(
    points: i32,
    countback: &crate::career::data_store::Countback,
    leader_points: i32,
    leader_countback: &crate::career::data_store::Countback,
    races_left: u32,
    per_race: i32,
) -> bool {
    let best = points + races_left as i32 * per_race;
    if best != leader_points {
        return best < leader_points;
    }
    let mut best_countback = countback.clone();
    (0..races_left).for_each(|_| best_countback.record(1));
    best_countback < *leader_countback
}

/// Whether the leader can clinch at the next round, and what it takes against each rival.
///
/// It can when winning that round while everyone else scores nothing would leave nobody able
/// to catch up. The margins are strict — enough to be ahead on points alone — so the sentence
/// built from them never promises a title that a countback might still take away.
fn clinch_next(
    table: &[&Standing],
    statuses: &[TitleStatus],
    races_left: u32,
    per_race: i32,
) -> Option<ClinchChance> {
    let leader = table.first()?;
    if races_left == 0 || statuses.first() != Some(&TitleStatus::InContention) {
        return None;
    }
    let after_left = races_left - 1;
    let mut won = leader.countback.clone();
    won.record(1);
    let possible = table[1..].iter().all(|d| {
        cannot_beat(d.points, &d.countback, leader.points + per_race, &won, after_left, per_race)
    });
    if !possible {
        return None;
    }
    let against: Vec<Margin> = table[1..]
        .iter()
        .zip(&statuses[1..])
        .filter(|(_, s)| **s == TitleStatus::InContention)
        .map(|(d, _)| Margin {
            rival: d.name.clone(),
            by: d.points + after_left as i32 * per_race - leader.points + 1,
        })
        .collect();
    // Only the countback made it possible: no margin on points alone is enough, so say nothing.
    if against.iter().any(|m| m.by > per_race) {
        return None;
    }
    Some(ClinchChance {
        leader: leader.name.clone(),
        player: leader.player,
        against,
    })
}

/// How the player stands against the championship position their contract asks for.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ObjectiveStatus {
    /// The position asked for.
    pub target: u32,
    /// The player's position now; `None` before their first race.
    pub position: Option<u32>,
    pub on_target: bool,
    /// Who holds the target position now, when the player does not.
    pub holder: Option<String>,
    /// Points between the player and that holder; 0 when on target.
    pub gap: i32,
}

/// The contract's objective against the standings as of `so_far`. `None` when the season has no
/// objective.
pub fn objective(facts: &SeasonFacts, so_far: &SeasonSoFar) -> Option<ObjectiveStatus> {
    let target = facts.objective?;
    let table = so_far.table();
    let position = so_far.player_position();
    let on_target = position.is_some_and(|p| p <= target);
    let mine = table.iter().find(|s| s.player).map_or(0, |s| s.points);
    let holder = (!on_target)
        .then(|| table.get(target as usize - 1))
        .flatten()
        .filter(|s| !s.player);
    Some(ObjectiveStatus {
        target,
        position,
        on_target,
        gap: holder.map_or(0, |h| h.points - mine),
        holder: holder.map(|h| h.name.clone()),
    })
}

#[cfg(test)]
#[path = "../../tests/career/story/title.rs"]
mod tests;
