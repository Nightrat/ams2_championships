//! Driver rating and team eligibility.
//!
//! Turns recorded results into a 0–100 reputation, then into a per-team eligibility tier that
//! gates which team the player may claim for a championship.
//!
//! The central problem is that raw finishing positions are confounded by machinery: P3 in an
//! Osella is a better drive than P3 in a Williams, so a naive "win races to unlock top teams"
//! rule would reward the player for already owning a fast car. Every score here is therefore
//! measured against what the *car* was expected to do, using the pace scalars in the
//! championship's Custom AI Driver file.
//!
//! Nothing about the user's setup is assumed. AI difficulty, lobby size and field composition
//! are never constants — they are read from, or cancelled out of, each session individually, so
//! the same code works for someone racing twenty humans at a different difficulty.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::custom_ai::{name_key, GridEntry, PlayerSeat, SeatEntry};
use crate::data_store::{Championship, RecordedSession, SessionResult};

/// Marker AMS2 appends to lobby-fill AI names. It appears only in multiplayer — single-player
/// grids built from a Custom AI Driver file carry the roster's real names — which makes it a
/// reliable multiplayer detector needing no configuration.
const AI_SUFFIX: &str = "(AI)";

/// Races older than this many entries count half as much, so the rating tracks current form.
/// It also makes the rating self-correct after an AI-difficulty change: scores shift, and the
/// old ones fade out within roughly two half-lives.
const RECENCY_HALF_LIFE: f32 = 10.0;

/// Pulls a small sample toward neutral so a couple of lucky results cannot unlock a top seat.
const SHRINKAGE: f32 = 5.0;

/// Largest reputation swing multiplayer results may contribute. Human-relative scoring is the
/// only difficulty-proof signal available there, but a two-driver lobby is nearly a coin flip,
/// so its influence is capped rather than blended in freely.
const MP_BONUS_CAP: f32 = 5.0;

/// How far below a team's bar the player may sit and still be told a seat is within reach.
const OFFER_MARGIN: f32 = 10.0;

/// Allowance against the incumbent's `race_skill`, in the same 0–1 units.
const INCUMBENT_MARGIN: f32 = 0.05;

/// The sessions a rating is allowed to see: those committed to a round of some championship.
///
/// A recorded session is not yet part of a career — the recorder captures every practice,
/// qualifying and race the moment AMS2 ends one, including throwaway runs and restarts.
/// Assigning it to a championship is the deliberate act that makes it count, so everything
/// downstream rates only what the user has actually claimed.
pub fn assigned_sessions(
    champs: &[Championship],
    sessions: &[RecordedSession],
) -> Vec<RecordedSession> {
    let assigned: HashSet<&str> = champs
        .iter()
        .flat_map(|c| c.rounds.iter())
        .flat_map(|r| r.session_ids.iter().map(|s| s.as_str()))
        .collect();
    sessions
        .iter()
        .filter(|s| assigned.contains(s.id.as_str()))
        .cloned()
        .collect()
}

/// True when the session was run online. See [`AI_SUFFIX`].
pub fn is_multiplayer(session: &RecordedSession) -> bool {
    session.results.iter().any(|r| r.name.contains(AI_SUFFIX))
}

/// True when this driver retired rather than being classified.
///
/// Only meaningful for races. The stored `dnf` flag means "completed fewer laps than the
/// leader", which also catches being *lapped* — punishing a slow car twice — and in qualifying
/// fires for anyone who simply ran a shorter run plan. This threshold separates a retirement
/// from a lapped finisher.
pub fn retired(r: &SessionResult, leader_laps: u32) -> bool {
    leader_laps > 0 && (r.laps_completed as f32) < 0.9 * leader_laps as f32
}

fn leader_laps(session: &RecordedSession) -> u32 {
    session
        .results
        .iter()
        .map(|r| r.laps_completed)
        .max()
        .unwrap_or(0)
}

/// Expected finishing position for each team, derived from car pace alone.
///
/// Seats are ordered by their team's `pace_delta_pct` and a team's expectation is the mean rank
/// of its seats, so a two-car team at the front expects roughly P1.5 rather than P1.
pub fn expected_positions(
    pace: &HashMap<String, f32>,
    seats: &[SeatEntry],
) -> HashMap<String, f32> {
    let mut distinct: Vec<(&str, &str)> = Vec::new();
    for s in seats {
        if !distinct.iter().any(|(seat, _)| *seat == s.seat) {
            distinct.push((&s.seat, &s.team));
        }
    }
    distinct.sort_by(|a, b| {
        let (x, y) = (pace.get(a.1).copied(), pace.get(b.1).copied());
        // Teams with no pace figure sort last rather than poisoning the order.
        x.unwrap_or(f32::MAX)
            .partial_cmp(&y.unwrap_or(f32::MAX))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks: HashMap<&str, Vec<f32>> = HashMap::new();
    for (i, (_, team)) in distinct.iter().enumerate() {
        ranks.entry(team).or_default().push(i as f32 + 1.0);
    }
    ranks
        .into_iter()
        .map(|(team, v)| (team.to_string(), v.iter().sum::<f32>() / v.len() as f32))
        .collect()
}

/// Performance in one session as a fraction of the grid, relative to what the car should manage.
///
/// Expressed in grid fractions rather than as a share of the available headroom: the latter
/// saturates, making a win from an expected P21 indistinguishable from a win from an expected
/// P7. Here the first is worth far more, which is the point.
fn positional_score(expected: f32, actual: f32, field: f32) -> f32 {
    if field <= 1.0 {
        return 0.0;
    }
    ((expected - actual) / (field - 1.0)).clamp(-1.0, 1.0)
}

/// The row to rate in a session.
///
/// With an explicit `target` the lookup is by name only — rating a specific driver must never be
/// hijacked by the `is_player` flag pointing at someone else. Without one, the subject is the
/// human: the `is_player` flag where present, otherwise the one car on a single-player grid that
/// is absent from the roster. That last resort cannot work online, where no human is in the
/// roster, so multiplayer sessions need the flag or a name.
fn player_row<'a>(
    session: &'a RecordedSession,
    seats: &[SeatEntry],
    target: Option<&str>,
) -> Option<&'a SessionResult> {
    if let Some(name) = target {
        return session.results.iter().find(|r| r.name == name);
    }
    if let Some(r) = session.results.iter().find(|r| r.is_player) {
        return Some(r);
    }
    if is_multiplayer(session) {
        return None;
    }
    let mut strangers = session.results.iter().filter(|r| {
        !seats
            .iter()
            .any(|e| name_key(&e.driver) == name_key(&r.name))
    });
    match (strangers.next(), strangers.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// Best guess at the human's name, used to attribute multiplayer rows.
///
/// Takes the `is_player` flag where present, otherwise the most frequent roster-absent name
/// across single-player sessions — which recovers the name from older recordings and then makes
/// it usable for the online ones.
pub fn infer_player_name(sessions: &[RecordedSession], seats: &[SeatEntry]) -> Option<String> {
    if let Some(r) = sessions
        .iter()
        .flat_map(|s| &s.results)
        .find(|r| r.is_player)
    {
        return Some(r.name.clone());
    }
    let mut tally: HashMap<&str, usize> = HashMap::new();
    for s in sessions.iter().filter(|s| !is_multiplayer(s)) {
        if let Some(r) = player_row(s, seats, None) {
            *tally.entry(r.name.as_str()).or_default() += 1;
        }
    }
    tally
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(name, _)| name.to_string())
}

/// Which team the player was driving for in a session, preferring the declared team whenever the
/// seat inference cannot narrow it to one.
fn session_team(
    session: &RecordedSession,
    seats: &[SeatEntry],
    expected: &HashMap<String, f32>,
    declared: Option<&str>,
) -> Option<String> {
    let grid: Vec<GridEntry> = session
        .results
        .iter()
        .map(|r| GridEntry {
            name: &r.name,
            car_name: &r.car_name,
            is_player: r.is_player,
        })
        .collect();
    match crate::custom_ai::infer_player_seat(seats, &grid) {
        PlayerSeat::Derived(seat) => Some(seat.team),
        PlayerSeat::Candidates(v) => {
            if let Some(d) = declared {
                if let Some(hit) = v.iter().find(|s| s.team.eq_ignore_ascii_case(d.trim())) {
                    return Some(hit.team.clone());
                }
            }
            v.into_iter()
                .find(|s| expected.contains_key(&s.team))
                .map(|s| s.team)
        }
        _ => None,
    }
}

/// A driver's rating and the evidence behind it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Reputation {
    /// 0–100. Performance at the AI difficulty actually raced, not an absolute skill measure.
    pub value: f32,
    /// Car-relative race pace, −1..+1.
    pub pace: f32,
    /// Car-relative qualifying pace, −1..+1.
    pub quali: f32,
    /// Share of race starts that were classified finishes, 0..1.
    pub finish_rate: f32,
    /// Reputation points contributed by online racing, within ±[`MP_BONUS_CAP`].
    pub mp_bonus: f32,
    pub sp_races: u32,
    pub mp_races: u32,
    /// Online record against other humans.
    pub mp_wins: u32,
    pub mp_losses: u32,
}

/// Recency-weighted mean of `(score, weight)` pairs given newest-first.
fn weighted_mean(entries: &[(f32, f32)]) -> f32 {
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, (score, w)) in entries.iter().enumerate() {
        let recency = 0.5f32.powf(i as f32 / RECENCY_HALF_LIFE);
        num += score * w * recency;
        den += w * recency;
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// One car class's roster and pace figures, as needed to score a session run in it.
///
/// A rating spanning several classes needs one of these per class: the expected finishing
/// position of a car only means anything relative to its own field.
pub struct RatingContext {
    /// Class name, matching the Custom AI file's stem (e.g. `F-Classic_Gen1`).
    pub class: String,
    pub seats: Vec<SeatEntry>,
    pub expected: HashMap<String, f32>,
}

impl RatingContext {
    pub fn new(class: &str, seats: Vec<SeatEntry>, pace: &HashMap<String, f32>) -> Self {
        let expected = expected_positions(pace, &seats);
        RatingContext {
            class: class.to_string(),
            seats,
            expected,
        }
    }

    /// True when a session was run in this class. AMS2 exposes aero variants as separate
    /// classes (`…_HD`) that share one roster, so those count here too.
    fn covers(&self, session: &RecordedSession) -> bool {
        session.car_class == self.class
            || session.car_class.starts_with(&format!("{}_", self.class))
    }
}

/// Computes the player's reputation from every recorded session in one class.
pub fn compute_reputation(
    sessions: &[RecordedSession],
    seats: &[SeatEntry],
    pace: &HashMap<String, f32>,
    declared_team: Option<&str>,
) -> Reputation {
    let name = infer_player_name(sessions, seats);
    compute_reputation_inner(name.as_deref(), sessions, seats, pace, declared_team)
}

/// Like [`compute_reputation`] but for a named driver, so every recorded human can be rated
/// rather than only the one identified as the player.
pub fn compute_reputation_for(
    driver: &str,
    sessions: &[RecordedSession],
    seats: &[SeatEntry],
    pace: &HashMap<String, f32>,
    declared_team: Option<&str>,
) -> Reputation {
    compute_reputation_inner(Some(driver), sessions, seats, pace, declared_team)
}

/// A driver's career rating across every class they have raced.
///
/// Scores from different classes combine directly because each race is already expressed as a
/// fraction of its own grid relative to what its own car should manage — the measure is
/// car-neutral, and therefore class-neutral too. Sessions in a class with no Custom AI file are
/// skipped: without a roster there is no expectation to measure against.
pub fn compute_reputation_global(
    driver: Option<&str>,
    sessions: &[RecordedSession],
    contexts: &[RatingContext],
    declared_team: Option<&str>,
) -> Reputation {
    let name = driver.map(|d| d.to_string()).or_else(|| {
        contexts.iter().find_map(|c| {
            let own: Vec<RecordedSession> =
                sessions.iter().filter(|s| c.covers(s)).cloned().collect();
            infer_player_name(&own, &c.seats)
        })
    });
    accumulate(name.as_deref(), sessions, contexts, declared_team)
}

/// Human drivers across every class — everyone who is neither a roster entry in any of them nor
/// an AMS2 lobby fill-in.
pub fn recorded_players_global(
    sessions: &[RecordedSession],
    contexts: &[RatingContext],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in sessions {
        if !contexts.iter().any(|c| c.covers(s)) {
            continue;
        }
        for r in &s.results {
            if r.name.contains(AI_SUFFIX) || out.iter().any(|n| n == &r.name) {
                continue;
            }
            let in_a_roster = contexts.iter().any(|c| {
                c.seats
                    .iter()
                    .any(|e| name_key(&e.driver) == name_key(&r.name))
            });
            if !in_a_roster {
                out.push(r.name.clone());
            }
        }
    }
    out.sort();
    out
}

fn compute_reputation_inner(
    player_name: Option<&str>,
    sessions: &[RecordedSession],
    seats: &[SeatEntry],
    pace: &HashMap<String, f32>,
    declared_team: Option<&str>,
) -> Reputation {
    // A single-class rating is the global one over a roster that covers everything it is given.
    let expected = expected_positions(pace, seats);
    let ctx = RatingContext {
        class: String::new(),
        seats: seats.to_vec(),
        expected,
    };
    accumulate_covering_all(player_name, sessions, &ctx, declared_team)
}

fn accumulate_covering_all(
    player_name: Option<&str>,
    sessions: &[RecordedSession],
    ctx: &RatingContext,
    declared_team: Option<&str>,
) -> Reputation {
    accumulate_with(player_name, sessions, declared_team, |_| Some(ctx))
}

fn accumulate(
    player_name: Option<&str>,
    sessions: &[RecordedSession],
    contexts: &[RatingContext],
    declared_team: Option<&str>,
) -> Reputation {
    accumulate_with(player_name, sessions, declared_team, |s| {
        contexts.iter().find(|c| c.covers(s))
    })
}

fn accumulate_with<'a>(
    player_name: Option<&str>,
    sessions: &[RecordedSession],
    declared_team: Option<&str>,
    context_for: impl Fn(&RecordedSession) -> Option<&'a RatingContext>,
) -> Reputation {
    let mut ordered: Vec<&RecordedSession> = sessions.iter().collect();
    ordered.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));

    let mut races: Vec<(f32, f32)> = Vec::new();
    let mut qualis: Vec<(f32, f32)> = Vec::new();
    let mut mps: Vec<(f32, f32)> = Vec::new();
    let (mut starts, mut finishes) = (0u32, 0u32);
    let (mut mp_wins, mut mp_losses) = (0u32, 0u32);

    for s in ordered {
        // A session in a class with no Custom AI file has no roster and no car pace, so there
        // is nothing to measure it against.
        let Some(ctx) = context_for(s) else { continue };
        let Some(me) = player_row(s, &ctx.seats, player_name) else {
            continue;
        };
        let field = s.results.len() as f32;

        if is_multiplayer(s) {
            // Absolute position online depends on an unrecorded difficulty slider, so only the
            // result against other humans is trustworthy. Weighting by the number of human
            // opponents lets a large lobby count for more without assuming a lobby size.
            let humans: Vec<&SessionResult> = s
                .results
                .iter()
                .filter(|r| !r.name.contains(AI_SUFFIX))
                .collect();
            if humans.len() < 2 || s.session_type != 5 {
                continue;
            }
            let beaten = humans
                .iter()
                .filter(|r| r.race_position > me.race_position)
                .count();
            let opponents = humans.len() - 1;
            let score = 2.0 * (beaten as f32 / opponents as f32) - 1.0;
            mps.push((score, opponents as f32));
            if score > 0.0 {
                mp_wins += 1;
            } else {
                mp_losses += 1;
            }
            continue;
        }

        let Some(team) = session_team(s, &ctx.seats, &ctx.expected, declared_team) else {
            continue;
        };
        let Some(&exp) = ctx.expected.get(&team) else {
            continue;
        };

        match s.session_type {
            5 => {
                starts += 1;
                // Retirements say nothing about pace; they feed the reliability term instead.
                if retired(me, leader_laps(s)) {
                    continue;
                }
                finishes += 1;
                races.push((positional_score(exp, me.race_position as f32, field), 1.0));
            }
            // Qualifying is scored on position alone — lap counts there reflect run plans.
            3 => qualis.push((positional_score(exp, me.race_position as f32, field), 1.0)),
            _ => {}
        }
    }

    let pace_score = weighted_mean(&races);
    let quali_score = weighted_mean(&qualis);
    let finish_rate = if starts > 0 {
        finishes as f32 / starts as f32
    } else {
        0.0
    };

    // Drop qualifying's share onto race pace when no qualifying has been recorded, so its
    // absence does not silently drag the rating toward neutral.
    let raw = if qualis.is_empty() {
        0.85 * pace_score + 0.15 * (2.0 * finish_rate - 1.0)
    } else {
        0.55 * pace_score + 0.30 * quali_score + 0.15 * (2.0 * finish_rate - 1.0)
    };

    let n = races.len() as f32;
    let shrunk = raw * n / (n + SHRINKAGE);
    let base = 50.0 * (1.0 + shrunk);
    let mp_bonus = if mps.is_empty() {
        0.0
    } else {
        (MP_BONUS_CAP * weighted_mean(&mps)).clamp(-MP_BONUS_CAP, MP_BONUS_CAP)
    };

    Reputation {
        value: (base + mp_bonus).clamp(0.0, 100.0),
        pace: pace_score,
        quali: quali_score,
        finish_rate,
        mp_bonus,
        sp_races: starts,
        mp_races: mps.len() as u32,
        mp_wins,
        mp_losses,
    }
}

/// How available a team's seat is to the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Both gates cleared — the seat can be claimed.
    Available,
    /// Within [`OFFER_MARGIN`] of the bar; a few more good results will open it.
    OfferPossible,
    /// Out of reach for now.
    Locked,
}

/// A team with the reputation it demands and whether the player currently meets it.
#[derive(Clone, Debug, Serialize)]
pub struct TeamEligibility {
    pub team: String,
    pub tier: Tier,
    /// Reputation needed, 0–100.
    pub required: f32,
    /// Expected finishing position for this car.
    pub expected_position: f32,
    /// Weaker incumbent's `race_skill`, if the file declares one.
    pub incumbent_skill: Option<f32>,
}

/// Reputation a team demands, given its 0-based rank among `total` teams ordered by car pace and
/// its weaker incumbent's `race_skill`.
///
/// Two gates combine and the stricter wins: how far up the grid a rating reaches, and whether it
/// would beat the driver already in the seat. That is why a midfield car staffed by two greats
/// can ask for more than a quicker one with a weak line-up.
pub fn required_rating(rank: usize, total: f32, incumbent_skill: Option<f32>) -> f32 {
    let grid = if total > 0.0 {
        100.0 * (1.0 - (rank as f32 + 1.0) / total)
    } else {
        0.0
    };
    let incumbent = incumbent_skill
        .map(|s| 100.0 * (s - INCUMBENT_MARGIN))
        .unwrap_or(0.0);
    grid.max(incumbent).clamp(0.0, 100.0)
}

/// Teams ordered by car pace, each with the reputation it demands. Mirrors the ordering used by
/// [`team_eligibility`] so callers can show the requirement without computing a rating first.
pub fn team_requirements(
    expected: &HashMap<String, f32>,
    skills: &HashMap<String, f32>,
) -> Vec<(String, f32)> {
    let mut teams: Vec<(&String, &f32)> = expected.iter().collect();
    teams.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
    let total = teams.len() as f32;
    teams
        .into_iter()
        .enumerate()
        .map(|(i, (team, _))| {
            (
                team.clone(),
                required_rating(i, total, skills.get(team).copied()),
            )
        })
        .collect()
}

/// Human drivers appearing in `sessions` — everyone who is neither a Custom AI roster entry nor
/// an AMS2 lobby fill-in. Sorted, deduplicated.
pub fn recorded_players(sessions: &[RecordedSession], seats: &[SeatEntry]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in sessions {
        for r in &s.results {
            if r.name.contains(AI_SUFFIX) || out.iter().any(|n| n == &r.name) {
                continue;
            }
            if seats
                .iter()
                .any(|e| name_key(&e.driver) == name_key(&r.name))
            {
                continue;
            }
            out.push(r.name.clone());
        }
    }
    out.sort();
    out
}

/// Ranks every team by car pace and works out what each demands.
///
/// Two gates combine into a single requirement: how far up the grid the player's reputation
/// reaches, and whether they would beat the team's weaker incumbent. The stricter one wins, so
/// a fast car staffed by two greats stays shut longer than its pace alone implies.
pub fn team_eligibility(
    reputation: f32,
    expected: &HashMap<String, f32>,
    skills: &HashMap<String, f32>,
) -> Vec<TeamEligibility> {
    let mut teams: Vec<(&String, &f32)> = expected.iter().collect();
    teams.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
    let total = teams.len() as f32;

    let mut out: Vec<TeamEligibility> = teams
        .into_iter()
        .enumerate()
        .map(|(i, (team, exp))| {
            let required = required_rating(i, total, skills.get(team).copied());
            let incumbent_skill = skills.get(team).copied();
            let tier = if reputation >= required {
                Tier::Available
            } else if reputation >= required - OFFER_MARGIN {
                Tier::OfferPossible
            } else {
                Tier::Locked
            };
            TeamEligibility {
                team: team.clone(),
                tier,
                required,
                expected_position: *exp,
                incumbent_skill,
            }
        })
        .collect();

    // A grid whose slowest car is still staffed by capable drivers can demand more than a new
    // driver has, locking every seat and leaving no way into the sport. The least demanding
    // team is therefore always open, whatever the rating.
    if !out.iter().any(|e| e.tier == Tier::Available) {
        let floor = out.iter().map(|e| e.required).fold(f32::MAX, f32::min);
        for e in out.iter_mut().filter(|e| e.required <= floor) {
            e.tier = Tier::Available;
        }
    }
    out
}

/// Whether `team` may be claimed. Unknown teams are allowed — the rating cannot judge a team it
/// has no pace data for, and must not block on ignorance.
pub fn is_allowed(eligibility: &[TeamEligibility], team: &str) -> bool {
    let t = team.trim();
    match eligibility.iter().find(|e| e.team.eq_ignore_ascii_case(t)) {
        Some(e) => e.tier != Tier::Locked,
        None => true,
    }
}

#[cfg(test)]
#[path = "tests/driver_rating.rs"]
mod tests;
