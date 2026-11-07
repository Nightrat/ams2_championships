//! The team's mood: how the player's own team sees them, as a figure from 0 to 100 that the whole
//! season feeds into.
//!
//! **Each influence is one [`MoodFactor`] in [`FACTORS`]**, with its weight as a named constant
//! beside it. [`replay`] walks the season race by race: the mood first eases back toward neutral
//! by [`REVERSION`] — a team remembers, but not for ever, and without it one good month would pin
//! the figure for the rest of the season — then every factor adds its delta. Nothing is stored;
//! like everything in the story it is the season, recounted.
//!
//! It needs a team: in a multiplayer career, or before signing, there is nobody to be pleased.
//! It is display-only for now — no money and no rating hang off it.

use serde::Serialize;

use super::facts::{SeasonFacts, SessionFacts};
use super::tally::SeasonSoFar;
use super::targets::{self, Verdict};
use super::teammate;
use super::title;

/// Where every career starts with a new team: hopeful, not yet sold.
pub const START: f32 = 60.0;
/// Share of the distance back to [`START`] the mood covers before each race.
pub const REVERSION: f32 = 0.10;

/// Mood at or above which the team is delighted.
const DELIGHTED: f32 = 75.0;
/// … content.
const CONTENT: f32 = 55.0;
/// … concerned. Below it, the team is losing patience.
const CONCERNED: f32 = 35.0;

/// Races running in the bottom band before the team lets it be known it is considering its
/// options. Only a warning: a contract runs for its season, and nobody is ever sacked.
pub const WARNING_RACES: usize = 3;

/// How many races running, up to the last of `rounds`, the team has been losing patience.
pub fn patience_run(rounds: &[RoundMood]) -> usize {
    rounds.iter().rev().take_while(|m| m.band == Band::LosingPatience).count()
}

/// Every influence on the mood. Adding one is a struct and a line here.
pub const FACTORS: &[&dyn MoodFactor] =
    &[&WeekendTarget, &CarPerformance, &TeammateDuel, &ObjectivePace, &Retirement];

/// One influence on how the team sees the player.
pub trait MoodFactor: Sync {
    /// A stable name for the factor.
    fn name(&self) -> &'static str;
    /// What this race did to the mood, if anything.
    fn delta(&self, cx: &MoodContext) -> Option<MoodDelta>;
}

/// What a factor sees: the season, the race, the tally around it and the weekend's verdict.
pub struct MoodContext<'a> {
    pub facts: &'a SeasonFacts,
    pub race: &'a SessionFacts,
    pub before: &'a SeasonSoFar,
    pub after: &'a SeasonSoFar,
    /// How the weekend target went, when one was set.
    pub verdict: Option<Verdict>,
}

/// One factor's effect on one race: how much, and why — the template key that says it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoodDelta {
    pub reason: &'static str,
    pub amount: f32,
}

/// How the team sees the player, in words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    LosingPatience,
    Concerned,
    Content,
    Delighted,
}

impl Band {
    pub fn of(mood: f32) -> Self {
        match mood {
            m if m >= DELIGHTED => Band::Delighted,
            m if m >= CONTENT => Band::Content,
            m if m >= CONCERNED => Band::Concerned,
            _ => Band::LosingPatience,
        }
    }

    /// The name the templates know it by.
    pub fn key(&self) -> &'static str {
        match self {
            Band::Delighted => "delighted",
            Band::Content => "content",
            Band::Concerned => "concerned",
            Band::LosingPatience => "losing_patience",
        }
    }
}

/// The mood after one race.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RoundMood {
    pub round: usize,
    /// Where it stood before the race.
    pub was: f32,
    pub mood: f32,
    pub band: Band,
    /// What moved it, factor by factor. Their sum is not the whole change: the drift back toward
    /// neutral comes first.
    pub deltas: Vec<MoodDelta>,
}

/// The mood after every race the player started. Empty without a team and a roster.
pub fn replay(facts: &SeasonFacts) -> Vec<RoundMood> {
    replay_with(facts, FACTORS)
}

/// [`replay`] over any list of factors — what the tests drive.
pub fn replay_with(facts: &SeasonFacts, factors: &[&dyn MoodFactor]) -> Vec<RoundMood> {
    if facts.player_team.is_none() || facts.roster.is_empty() {
        return Vec::new();
    }
    let verdicts = targets::season(facts).judged;
    let mut mood = START;
    let mut before = SeasonSoFar::start(facts);
    let mut out = Vec::new();
    for round in &facts.rounds {
        let mut after = before.clone();
        after.record(round);
        if let Some(race) = round.race().filter(|r| r.player().is_some()) {
            let cx = MoodContext {
                facts,
                race,
                before: &before,
                after: &after,
                verdict: verdicts.iter().find(|j| j.target.round == round.number).map(|j| j.verdict),
            };
            let deltas: Vec<MoodDelta> = factors.iter().filter_map(|f| f.delta(&cx)).collect();
            let was = mood;
            mood += (START - mood) * REVERSION;
            mood = (mood + deltas.iter().map(|d| d.amount).sum::<f32>()).clamp(0.0, 100.0);
            out.push(RoundMood {
                round: round.number,
                was,
                mood,
                band: Band::of(mood),
                deltas,
            });
        }
        before = after;
    }
    out
}

/// Meeting the weekend target pleases the team; missing it does the opposite. The biggest single
/// influence, because a target is exactly what the team said it wanted.
pub struct WeekendTarget;

const TARGET_MET: f32 = 6.0;
const TARGET_MISSED: f32 = -6.0;

impl MoodFactor for WeekendTarget {
    fn name(&self) -> &'static str {
        "weekend_target"
    }
    fn delta(&self, cx: &MoodContext) -> Option<MoodDelta> {
        match cx.verdict? {
            Verdict::Met => Some(MoodDelta { reason: "mood.f.target_met", amount: TARGET_MET }),
            Verdict::Missed => Some(MoodDelta { reason: "mood.f.target_missed", amount: TARGET_MISSED }),
            Verdict::Void => None,
        }
    }
}

/// Finishing ahead of where the car should, or behind it — graded, where a target is pass or
/// fail. It is what a team most wants from a driver, and the one factor every weekend has: a
/// one-car team has no teammate to beat, and a back-marker is set no top-N target because there
/// is no place behind its expected one to ask for.
pub struct CarPerformance;

/// Mood per place gained on the car's expected finish.
const PER_PLACE: f32 = 0.25;
/// Most one result can add: a giant-killing pleases, but it is one race.
const CAR_MOST_UP: f32 = 5.0;
/// Most one result can take away.
const CAR_MOST_DOWN: f32 = -4.0;

impl MoodFactor for CarPerformance {
    fn name(&self) -> &'static str {
        "car_performance"
    }
    fn delta(&self, cx: &MoodContext) -> Option<MoodDelta> {
        let me = cx.race.player().filter(|m| !m.dnf)?;
        let team = cx.facts.player_team.as_deref()?;
        // A roster can run more cars than the grid fielded; the car cannot be expected to finish
        // behind the last place there was.
        let expected = cx.facts.expected.get(team)?.min(cx.race.results.len() as f32);
        let amount = ((expected - me.position as f32) * PER_PLACE).clamp(CAR_MOST_DOWN, CAR_MOST_UP);
        match amount {
            a if a >= 0.5 => Some(MoodDelta { reason: "mood.f.beat_car", amount: a }),
            a if a <= -0.5 => Some(MoodDelta { reason: "mood.f.below_car", amount: a }),
            _ => None,
        }
    }
}

/// Finishing ahead of the other car, or behind it. Averaged over the teammates who started, so a
/// three-entry team does not count double.
pub struct TeammateDuel;

const TEAMMATE: f32 = 4.0;

impl MoodFactor for TeammateDuel {
    fn name(&self) -> &'static str {
        "teammate_duel"
    }
    fn delta(&self, cx: &MoodContext) -> Option<MoodDelta> {
        let me = cx.race.player()?;
        let duels: Vec<f32> = teammate::teammates(cx.facts)
            .into_iter()
            .filter_map(|name| cx.race.driver(name))
            .filter_map(|them| me.beats(them))
            .map(|won| if won { 1.0 } else { -1.0 })
            .collect();
        if duels.is_empty() {
            return None;
        }
        let amount = TEAMMATE * duels.iter().sum::<f32>() / duels.len() as f32;
        match amount {
            a if a > 0.0 => Some(MoodDelta { reason: "mood.f.beat_teammate", amount: a }),
            a if a < 0.0 => Some(MoodDelta { reason: "mood.f.lost_teammate", amount: a }),
            _ => None,
        }
    }
}

/// Being on course for the contract's objective is a quiet reassurance every race; being off it a
/// steady pressure, heavier once the gap is more than a race's worth of points.
pub struct ObjectivePace;

const ON_OBJECTIVE: f32 = 2.0;
const OFF_OBJECTIVE: f32 = -1.0;
const FAR_OFF_OBJECTIVE: f32 = -3.0;

impl MoodFactor for ObjectivePace {
    fn name(&self) -> &'static str {
        "objective_pace"
    }
    fn delta(&self, cx: &MoodContext) -> Option<MoodDelta> {
        let status = title::objective(cx.facts, cx.after)?;
        let per_race = cx.facts.points_system.iter().copied().max().unwrap_or(0);
        Some(if status.on_target {
            MoodDelta { reason: "mood.f.on_objective", amount: ON_OBJECTIVE }
        } else if status.gap > per_race {
            MoodDelta { reason: "mood.f.far_off_objective", amount: FAR_OFF_OBJECTIVE }
        } else {
            MoodDelta { reason: "mood.f.off_objective", amount: OFF_OBJECTIVE }
        })
    }
}

/// A retirement costs a little. Only a little: nothing recorded says whether the driver crashed
/// or the engine let go, and a team does not blame its driver for the second.
pub struct Retirement;

const RETIRED: f32 = -2.0;

impl MoodFactor for Retirement {
    fn name(&self) -> &'static str {
        "retirement"
    }
    fn delta(&self, cx: &MoodContext) -> Option<MoodDelta> {
        cx.race
            .player()
            .filter(|me| me.dnf)
            .map(|_| MoodDelta { reason: "mood.f.retired", amount: RETIRED })
    }
}

#[cfg(test)]
#[path = "../../tests/career/story/mood.rs"]
mod tests;
