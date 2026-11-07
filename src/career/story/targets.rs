//! Weekend targets: what the team expects from the player at each race, and whether they got it.
//!
//! A season objective is judged once, at the end; a target is judged every weekend, which is what
//! gives each race its own stakes. **Each kind of target is one [`TargetKind`] in [`TARGETS`]**:
//! it says whether it fits the weekend, sets the target, and judges it. For each race round the
//! kinds that fit are collected and one is picked by a seed of the season and round, so the
//! targets vary, are always fair, and never reroll.
//!
//! A target is set from what was known **before** the round — the tally up to the previous race —
//! and judged on the round's race. It is about the player's own team, so it needs a team and a
//! roster: a multiplayer career has none.
//!
//! No money and no rating hang off a target. The team's mood (a later phase) reads the verdicts.

use serde::Serialize;

use super::facts::{SeasonFacts, SessionFacts};
use super::tally::SeasonSoFar;
use super::teammate;
use super::templates;
use crate::career::seed::hash64;
use crate::format::ordinal;

/// Places of slack on top of the car's expected finish. A target the car only meets on a perfect
/// day is not a target, it is a wish; with this much room the driver who does the car justice
/// meets it.
const TARGET_SLACK: u32 = 2;

/// Every kind of target. Adding one is a struct and a line here.
pub const TARGETS: &[&dyn TargetKind] = &[&TopN, &BeatTeammate, &BringItHome];

/// One kind of weekend target.
pub trait TargetKind: Sync {
    /// A stable name for the kind.
    fn id(&self) -> &'static str;
    /// This weekend's target of this kind, if the kind fits the weekend.
    fn set(&self, cx: &TargetContext) -> Option<Target>;
    /// Whether `target` was met in `race`.
    fn judge(&self, target: &Target, race: &SessionFacts, facts: &SeasonFacts) -> Verdict;
}

/// What a target is set from: the season, and how it stood before the round.
pub struct TargetContext<'a> {
    pub facts: &'a SeasonFacts,
    pub before: &'a SeasonSoFar,
    /// The round the target is for.
    pub round: usize,
}

/// One weekend's target.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Target {
    /// Which [`TargetKind`] set it.
    pub kind: &'static str,
    pub round: usize,
    /// The place to finish at or above, for a target that has one.
    pub place: Option<u32>,
    /// What the team asks, in a sentence.
    pub text: String,
}

/// How a weekend went against its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Met,
    Missed,
    /// Could not be judged: the player did not start, or what it was about never happened (the
    /// teammate sat the race out).
    Void,
}

/// A raced round's target and its verdict.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JudgedTarget {
    pub target: Target,
    pub verdict: Verdict,
}

/// Every raced round's target with its verdict, and the target for the next round.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SeasonTargets {
    pub judged: Vec<JudgedTarget>,
    pub next: Option<Target>,
}

/// The season's targets. Empty without a team and a roster.
pub fn season(facts: &SeasonFacts) -> SeasonTargets {
    season_with(facts, TARGETS)
}

/// [`season`] over any list of kinds — what the tests drive.
pub fn season_with(facts: &SeasonFacts, kinds: &[&dyn TargetKind]) -> SeasonTargets {
    let mut out = SeasonTargets::default();
    if facts.player_team.is_none() || facts.roster.is_empty() {
        return out;
    }
    let mut so_far = SeasonSoFar::start(facts);
    let mut last_raced = 0;
    for round in &facts.rounds {
        if let Some(race) = round.race() {
            last_raced = round.number;
            if let Some((kind, target)) = pick(facts, &so_far, round.number, kinds) {
                out.judged.push(JudgedTarget {
                    verdict: kind.judge(&target, race, facts),
                    target,
                });
            }
        }
        so_far.record(round);
    }
    // The next round is the first after the last race that has not been raced — one already
    // added and waiting, or the one still to be added.
    let next_round = facts
        .rounds
        .iter()
        .find(|r| r.number > last_raced && r.race().is_none())
        .map_or(facts.rounds.len() + 1, |r| r.number);
    out.next = pick(facts, &so_far, next_round, kinds).map(|(_, t)| t);
    out
}

/// The target for `round`: one of the kinds that fit, chosen by the season and the round.
fn pick<'k>(
    facts: &SeasonFacts,
    before: &SeasonSoFar,
    round: usize,
    kinds: &[&'k dyn TargetKind],
) -> Option<(&'k dyn TargetKind, Target)> {
    let cx = TargetContext { facts, before, round };
    let mut fitting: Vec<(&dyn TargetKind, Target)> =
        kinds.iter().filter_map(|k| Some((*k, k.set(&cx)?))).collect();
    if fitting.is_empty() {
        return None;
    }
    let seed = hash64(&[&facts.champ_id, "target", &round.to_string()]);
    let i = (seed % fitting.len() as u64) as usize;
    Some(fitting.swap_remove(i))
}

fn target(kind: &dyn TargetKind, cx: &TargetContext, place: Option<u32>, key: &str, values: &[(&str, String)]) -> Target {
    Target {
        kind: kind.id(),
        round: cx.round,
        place,
        text: templates::render(key, values, 0),
    }
}

/// How many cars the coming race is likely to field: the last race's grid, or the roster's
/// drivers before the first.
fn field(cx: &TargetContext) -> u32 {
    let last = cx
        .facts
        .rounds
        .iter()
        .rev()
        .filter(|r| r.number < cx.round)
        .find_map(|r| r.race())
        .map(|s| s.results.len() as u32);
    last.unwrap_or_else(|| cx.facts.roster.iter().filter(|d| !d.stand_in).count() as u32)
}

/// Finish at or above where the car should, with [`TARGET_SLACK`] places of room. Fits only when
/// the car has an expected position and the target would still leave cars behind it — asking a
/// back-marker for "the top 26" of 26 is asking it to finish.
pub struct TopN;

impl TargetKind for TopN {
    fn id(&self) -> &'static str {
        "top_n"
    }
    fn set(&self, cx: &TargetContext) -> Option<Target> {
        let team = cx.facts.player_team.as_deref()?;
        let expected = *cx.facts.expected.get(team)?;
        let place = expected.ceil().max(1.0) as u32 + TARGET_SLACK;
        if place >= field(cx) {
            return None;
        }
        Some(target(self, cx, Some(place), "target.top", &[("pos", ordinal(place))]))
    }
    fn judge(&self, target: &Target, race: &SessionFacts, _: &SeasonFacts) -> Verdict {
        let (Some(me), Some(place)) = (race.player(), target.place) else {
            return Verdict::Void;
        };
        if !me.dnf && me.position <= place {
            Verdict::Met
        } else {
            Verdict::Missed
        }
    }
}

/// Finish ahead of the other car. Fits whenever the team runs one — same machinery, so it is the
/// fairest target there is.
pub struct BeatTeammate;

impl TargetKind for BeatTeammate {
    fn id(&self) -> &'static str {
        "beat_teammate"
    }
    fn set(&self, cx: &TargetContext) -> Option<Target> {
        let mates = teammate::teammates(cx.facts);
        if mates.is_empty() {
            return None;
        }
        Some(target(self, cx, None, "target.teammate", &[("teammate", mates.join(" and "))]))
    }
    /// Met by finishing ahead of every teammate who started; void when none did.
    fn judge(&self, _: &Target, race: &SessionFacts, facts: &SeasonFacts) -> Verdict {
        let Some(me) = race.player() else {
            return Verdict::Void;
        };
        let duels: Vec<Option<bool>> = teammate::teammates(facts)
            .into_iter()
            .filter_map(|name| race.driver(name))
            .map(|them| me.beats(them))
            .collect();
        if duels.is_empty() {
            Verdict::Void
        } else if duels.iter().all(|d| *d == Some(true)) {
            Verdict::Met
        } else {
            Verdict::Missed
        }
    }
}

/// Bring the car home. Fits only straight after a retirement: a team that has just lost a car
/// wants the points the next one can bring, before anything else.
pub struct BringItHome;

impl TargetKind for BringItHome {
    fn id(&self) -> &'static str {
        "bring_it_home"
    }
    fn set(&self, cx: &TargetContext) -> Option<Target> {
        let last = cx
            .facts
            .rounds
            .iter()
            .rev()
            .filter(|r| r.number < cx.round)
            .find_map(|r| r.race()?.player())?;
        if !last.dnf {
            return None;
        }
        Some(target(self, cx, None, "target.finish", &[]))
    }
    fn judge(&self, _: &Target, race: &SessionFacts, _: &SeasonFacts) -> Verdict {
        match race.player() {
            Some(me) if !me.dnf => Verdict::Met,
            Some(_) => Verdict::Missed,
            None => Verdict::Void,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/career/story/targets.rs"]
mod tests;
