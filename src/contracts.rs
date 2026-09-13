//! Seat offers and the contracts they become.
//!
//! The driver rating in [`crate::driver_rating`] answers "may this driver have this seat".
//! This module answers the two questions that follow: *on what terms*, and *what was agreed*.
//!
//! Almost nothing here is persisted, for the same reason the rating persists nothing — an offer
//! is a function of the career as it currently stands, so regenerating it each time keeps it
//! honest when sessions are reassigned or the rating tuning changes. The single exception is
//! [`Contract`]: terms that were *accepted* are history, and re-deriving them later would
//! silently rewrite a deal the driver made under conditions that no longer hold.
//!
//! Everything an offer is built from already exists. [`TeamEligibility`] carries the team's
//! requirement and what its car should finish, its position in the list is the team's rank, and
//! the reputation carries how far clear of that bar the driver sits. This module only turns
//! those into money and a target.

use serde::{Deserialize, Serialize};

use crate::data_store::{Championship, ChampionshipStatus, RecordedSession};
use crate::driver_rating::{TeamEligibility, Tier};

/// Per-season pay for the quickest car on the grid, in credits.
///
/// Default for [`OfferParams::top_salary`].
const TOP_SALARY: i64 = 4_000_000;

/// Per-season pay for the slowest car on the grid, in credits. The spread between this and
/// [`TOP_SALARY`] is what makes climbing the ladder worth anything.
///
/// Forty times, to match the real spread: a 2025 front-runner earns somewhere around $65m and a
/// rookie at the back $1–2m. Twenty times, as this was first set, made the bottom of the grid a
/// far softer landing than it is.
///
/// Default for [`OfferParams::floor_salary`].
const FLOOR_SALARY: i64 = 100_000;

/// Positions of grace added to an earned seat's target. A team that got its driver fairly asks
/// for what the car should manage plus a little; one that was paid to take them does not.
///
/// Default for [`OfferParams::objective_slack`].
const OBJECTIVE_SLACK: u32 = 2;

/// Rating points clear of a team's bar that earn the full pay rise below. A driver exactly on
/// the bar gets the base rate; one this far above gets the best the team pays.
const LEVERAGE_RANGE: f32 = 20.0;

/// Largest pay rise leverage may win, as a fraction of the team's base rate.
const LEVERAGE_BONUS: f32 = 0.25;

/// What a bought seat pays relative to an earned one. The team is being paid to have the
/// driver; it does not also pay them what it would pay someone it rated.
const PAY_DRIVER_RATE: f32 = 0.6;

/// Bound on the per-team salary jitter, as a fraction either way. Exists so two teams sitting
/// beside each other on the grid do not read as interchangeable; deliberately too small to
/// change which offer is worth taking.
const SALARY_JITTER: f32 = 0.08;

/// Sponsorship demanded per rating point short of a team's bar.
///
/// Default for [`OfferParams::buy_in_per_point`]; zero there switches pay-driver seats off
/// entirely, and a locked team then makes no offer at all.
const BUY_IN_PER_POINT: i64 = 150_000;

/// Share of the grid, counted from the back, whose teams will take a driver's sponsorship in
/// exchange for a seat.
///
/// A pay driver is a back-of-the-grid phenomenon, because the money is what keeps a skint team
/// running. Osella spent 1986 — a season on this app's own reference grid — asking its drivers
/// to bring sponsorship after its state tobacco backing left; AGS and Coloni ran the same way,
/// as Haas did with Uralkali and Williams with Latifi. A front-running team has no budget hole
/// to plug and loses more by fielding a slow driver than any cheque covers, so no amount of
/// money opens that seat.
///
/// Default for [`OfferParams::pay_driver_share`].
const PAY_DRIVER_SHARE: f32 = 1.0 / 3.0;

/// Pay rise a renewal carries once the driver has delivered, as a fraction of the team's rate.
/// Reached at [`LOYALTY_TENURE`] seasons served and flat after that.
///
/// Default for [`OfferParams::loyalty_bonus`].
const LOYALTY_BONUS: f32 = 0.3;

/// Seasons at one team that earn the full [`OfferParams::loyalty_bonus`].
const LOYALTY_TENURE: u32 = 3;

/// Which way the money flows. That is the whole of it: either the team wants the driver, or the
/// driver wants the seat.
///
/// There used to be four kinds. `Provisional` — a trial — meant something while deals could run
/// for several seasons: it was the one-year prove-it deal against a multi-year one. Once every
/// deal became a single season it was just a firm offer that paid less, and the pay cut it
/// carried is better expressed by the leverage a rating earns, which already slides to nothing
/// at the team's bar. `Renewal` was never a different *kind* of contract either — it is how an
/// offer was come by, not what it is, so it lives on as [`Offer::renewal`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferKind {
    /// The team pays the driver. Every seat that is not bought.
    Paid,
    /// The driver pays the team — sponsorship for a seat the rating has not earned, and only
    /// ever from the back of the grid. Carries a [`Offer::buy_in`].
    Pay,
}

/// One season's terms, as a team would put them to the driver.
///
/// Every deal runs for exactly one championship. Teams do not offer multi-year contracts: a
/// season is the unit a career is measured in here, and a deal spanning seasons would have to
/// survive the roster changing under it — the next championship may run an entirely different
/// Custom AI file, where the team may not exist at all.
///
/// Derived, never stored. Regenerating this after the rating moves is the intended behaviour:
/// until it is signed, an offer is only what the team would say today.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Offer {
    pub team: String,
    pub kind: OfferKind,
    /// True when this is the driver's current team re-signing them rather than the open market.
    ///
    /// Not a kind of contract — the terms are an ordinary paid deal — but it is how the offer
    /// came about, and it is the only reason a team out of the driver's reach is offering at
    /// all. It carries the loyalty rise, and it is what having met a target buys.
    pub renewal: bool,
    /// Credits for the season.
    pub salary: i64,
    /// Championship position the team expects, or `None` when the car is slow enough that
    /// asking for one would be meaningless.
    pub objective: Option<u32>,
    /// The team's place in the pace order, 0 = quickest car. Carried so a caller can sort by
    /// prestige rather than by money, which stop being the same order once leverage applies.
    pub rank: usize,
    /// Sponsorship demanded to take a seat the rating has not earned. Zero on every
    /// [`OfferKind::Paid`] offer.
    pub buy_in: i64,
    /// The team's requirement, carried through from [`TeamEligibility::required`].
    pub required: f32,
    /// Where the car is expected to finish — the figure the objective is built from.
    pub expected_position: f32,
}

/// An accepted offer. The only thing in this module written to the save file.
///
/// Joined to everything else by `champ_id`: the championship already records which season this
/// was, which roster it ran against and which team the player drove for, so a contract adds
/// only the terms that were agreed and cannot be recovered from results. One row, one season —
/// see [`Offer`] for why deals never span championships.
///
/// A save written while multi-year deals existed may still carry a `seasons` field. Serde
/// ignores it, which is the intended outcome: it never meant anything mechanically.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    /// The championship this contract covers — the season, in career terms.
    pub champ_id: String,
    pub team: String,
    /// Unix seconds at signing.
    #[serde(default)]
    pub signed_at: u64,
    /// Credits for the season.
    pub salary: i64,
    #[serde(default)]
    pub objective: Option<u32>,
    /// Credits paid to take a seat the rating had not earned. Zero when the seat was won on
    /// merit, which is every contract until buy-ins exist.
    #[serde(default)]
    pub bought_for: i64,
}

impl Contract {
    /// Freezes an offer into the record of a signing.
    pub fn from_offer(champ_id: &str, offer: &Offer, signed_at: u64) -> Contract {
        Contract {
            champ_id: champ_id.to_string(),
            team: offer.team.clone(),
            signed_at,
            salary: offer.salary,
            objective: offer.objective,
            bought_for: 0,
        }
    }
}

/// The contract covering a championship, if one was signed for it.
pub fn for_championship<'a>(contracts: &'a [Contract], champ_id: &str) -> Option<&'a Contract> {
    contracts.iter().find(|c| c.champ_id == champ_id)
}

/// The tunable half of offer generation, mirroring [`crate::driver_rating::RatingParams`].
///
/// [`Default`] is the shipped economy; a config may replace it wholesale without this module
/// knowing where the numbers came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OfferParams {
    /// Per-season pay for the quickest car, in credits.
    pub top_salary: i64,
    /// Per-season pay for the slowest car, in credits.
    pub floor_salary: i64,
    /// Positions of grace a firm offer's target carries.
    pub objective_slack: u32,
    /// Sponsorship demanded per rating point short of a locked team's bar. Zero switches
    /// pay-driver seats off: a team the rating cannot reach then simply makes no offer.
    pub buy_in_per_point: i64,
    /// Share of the grid, from the back, that will sell a seat for sponsorship. Zero has the
    /// same effect as a zero rate — nobody sells.
    pub pay_driver_share: f32,
    /// Pay rise a renewal carries once the driver has delivered, as a fraction of the rate.
    pub loyalty_bonus: f32,
}

impl Default for OfferParams {
    fn default() -> Self {
        OfferParams {
            top_salary: TOP_SALARY,
            floor_salary: FLOOR_SALARY,
            objective_slack: OBJECTIVE_SLACK,
            buy_in_per_point: BUY_IN_PER_POINT,
            pay_driver_share: PAY_DRIVER_SHARE,
            loyalty_bonus: LOYALTY_BONUS,
        }
    }
}

/// Where the driver stands with the grid: the seat they hold and what they did with it.
///
/// Derived from the ledger by [`standing`], never stored. [`Default`] is a driver with no
/// completed season behind them, which is what every offer was built from before renewals
/// existed — so `offers_for` under it behaves exactly as it did.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Standing {
    /// The team of the most recent *completed* season. A season still being raced does not make
    /// someone an incumbent — the deal it renews has not run its course.
    pub incumbent: Option<String>,
    /// The class that seat was in. An incumbency is only worth anything inside its own series,
    /// so this has to match the championship being offered on before the team counts as held.
    pub class: String,
    /// Whether that season's objective was met. `None` when the deal set no target, which is
    /// treated as nothing to fail: a team that asked for nothing has no grounds to drop anyone.
    pub delivered: Option<bool>,
    /// Consecutive completed seasons at the incumbent, most recent last.
    pub tenure: u32,
}

/// The driver's standing, read off a ledger [`finances`] has already built.
///
/// Takes the ledger rather than the raw career because every input it needs — which seasons are
/// complete, in what order, and whether each target was met — is exactly what a ledger row is.
pub fn standing(ledger: &Finances) -> Standing {
    let done: Vec<&SeasonLedger> = ledger.seasons.iter().filter(|s| s.complete).collect();
    let Some(last) = done.last() else {
        return Standing::default();
    };
    // Only an unbroken run at the same team *in the same series* counts. A driver who left and
    // came back is a returning signing, and one whose last two seasons were in different classes
    // never served two years anywhere — a 1967 Ferrari and a 1990 one share nothing but a name.
    let tenure = done
        .iter()
        .rev()
        .take_while(|s| s.team == last.team && s.class == last.class)
        .count() as u32;
    Standing {
        incumbent: Some(last.team.clone()),
        class: last.class.clone(),
        delivered: last.objective_met,
        tenure,
    }
}

/// FNV-1a over the given parts, with a separator so `("ab", "c")` and `("a", "bc")` differ.
///
/// Hand-rolled rather than taken from `DefaultHasher`, whose algorithm is explicitly allowed to
/// change between Rust releases. A team's terms must not move because the toolchain did.
fn hash64(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A stable −1..+1 figure for one team in one championship, used only to vary salaries.
fn jitter(champ_id: &str, team: &str) -> f32 {
    2.0 * ((hash64(&[champ_id, team]) % 1000) as f32 / 999.0) - 1.0
}

/// Interpolates from `top` down to `floor` across `share` in 0..1.
///
/// Geometric rather than linear, because money in motorsport steps by ratio and not by a fixed
/// sum: the gap between the front two cars dwarfs the gap between the back two, and the same is
/// true of a championship payout. Both ends are floored at 1 so a config of zero cannot turn the
/// curve into a division by zero or a `NaN`.
fn geometric(top: i64, floor: i64, share: f32) -> f32 {
    let top = top.max(1) as f32;
    let floor = floor.max(1) as f32;
    top * (floor / top).powf(share.clamp(0.0, 1.0))
}

/// Base pay for a team at `rank` of `total`, before leverage and jitter.
///
/// A single team on the grid is paid the top rate — there is nothing to rank it against.
fn base_salary(rank: usize, total: usize, params: &OfferParams) -> f32 {
    if total <= 1 {
        return params.top_salary.max(1) as f32;
    }
    geometric(
        params.top_salary,
        params.floor_salary,
        rank as f32 / (total - 1) as f32,
    )
}

/// Championship position a team asks for, or `None` when the car is too slow for a target to
/// mean anything.
///
/// Built from what the car should manage rather than from an invented number, so a midfield
/// team asks for a midfield result. Once the target plus its slack reaches the back of the grid
/// it is vacuous — every finish clears it — and a team that cannot ask for anything is better
/// modelled as not asking.
fn objective(expected_position: f32, field: f32, slack: u32, kind: OfferKind) -> Option<u32> {
    let target = expected_position.round().max(1.0) as u32;
    // Only an earned seat carries slack. A bought one is the team's way of saying the results
    // had better arrive.
    let target = if kind == OfferKind::Paid {
        target + slack
    } else {
        target
    };
    if (target as f32) >= field {
        None
    } else {
        Some(target)
    }
}

/// Whether a team is short enough of money to sell its seat.
///
/// The bottom [`OfferParams::pay_driver_share`] of the grid, by car pace. Everything ahead of
/// that is closed to a driver who has not earned it, at any price — see [`PAY_DRIVER_SHARE`].
fn sells_seat(rank: usize, total: usize, params: &OfferParams) -> bool {
    let share = params.pay_driver_share.clamp(0.0, 1.0);
    if total == 0 || share <= 0.0 {
        return false;
    }
    // At least one team, so a share this small still means something on a short grid.
    let sellers = ((total as f32 * share).round() as usize).max(1);
    rank + sellers >= total
}

/// Sponsorship a team wants before it will take a driver the rating says it should not.
///
/// Purely the shortfall: the money is what plugs the team's budget hole, and every team that
/// sells is at the back of the grid already, so there is no car quality left to price in. An
/// earlier version added a season's wage on top, which made the *quickest* car the most
/// expensive to buy into — backwards, since those seats are not for sale at all.
fn buy_in(shortfall: f32, params: &OfferParams) -> i64 {
    if params.buy_in_per_point <= 0 {
        return 0;
    }
    (shortfall.max(0.0) * params.buy_in_per_point as f32) as i64
}

/// Every seat the driver could take this season, with the terms attached, for a driver with no
/// completed season behind them. See [`offers_for_with`].
pub fn offers_for(
    champ_id: &str,
    reputation: f32,
    eligibility: &[TeamEligibility],
    params: &OfferParams,
) -> Vec<Offer> {
    // The class is irrelevant without a standing to scope: a driver with no completed season
    // behind them holds no seat in any series.
    offers_for_with(
        champ_id,
        "",
        reputation,
        eligibility,
        &Standing::default(),
        params,
    )
}

/// Every seat the driver could take this season, with the terms attached.
///
/// `eligibility` comes straight from [`crate::driver_rating::team_eligibility_with`] and is
/// already ordered fastest car first, which is the rank each offer reports.
///
/// A team the rating cannot reach makes no offer, with one exception: the back of the grid,
/// where a team short of money will take a driver's sponsorship instead — see [`sells_seat`].
/// The reason a team refuses is already in the eligibility list the caller holds.
///
/// `standing` is what turns a career into a negotiating position: the incumbent re-signs a
/// driver who delivered whatever the rating now says, and drops one who did not back to whatever
/// they can earn on merit.
///
/// `champ_id` seeds the salary jitter, so one season's offers are stable however often they are
/// regenerated, and a different season reads differently.
pub fn offers_for_with(
    champ_id: &str,
    class: &str,
    reputation: f32,
    eligibility: &[TeamEligibility],
    standing: &Standing,
    params: &OfferParams,
) -> Vec<Offer> {
    let total = eligibility.len();
    // The back of the grid, in finishing positions rather than teams — what an objective has to
    // beat to be worth stating.
    let field = eligibility
        .iter()
        .map(|e| e.expected_position)
        .fold(0.0f32, f32::max);

    eligibility
        .iter()
        .enumerate()
        .filter_map(|(rank, e)| {
            // The seat already held, by a driver whose last season is behind them. `delivered`
            // is `Some(false)` only when a target was actually set and actually missed — a deal
            // that asked for nothing gives the team no grounds to drop anyone.
            // A seat is only held inside the series it was held in. Team names are not unique
            // across classes — "Ferrari" is in seven of the eight shipped rosters — so matching
            // on the name alone would renew a 1967 drive into a 1990 car, granted regardless of
            // rating because delivering keeps the seat. An empty class matches nothing.
            let same_series = !class.is_empty() && standing.class == class;
            let held = same_series && standing.incumbent.as_deref() == Some(e.team.as_str());
            let renewing = held && standing.delivered != Some(false);

            let kind = if renewing {
                // Delivering keeps the seat, whatever the rating now says. That is what having
                // met the target is *for*; re-earning it every winter would make the objective
                // decorative.
                OfferKind::Paid
            } else {
                match e.tier {
                    // Above the bar or within reach of it are the same kind of deal. What
                    // separates them is the leverage below, which slides to nothing at the bar
                    // — a gradient rather than the cliff a separate "trial" kind made of it.
                    Tier::Available | Tier::OfferPossible => OfferKind::Paid,
                    // Only a team that needs the money will take it, and only the back of the
                    // grid does. Anything quicker that the rating cannot reach stays shut.
                    Tier::Locked
                        if params.buy_in_per_point > 0 && sells_seat(rank, total, params) =>
                    {
                        OfferKind::Pay
                    }
                    Tier::Locked => return None,
                }
            };

            // How far clear of the bar the driver stands, 0..1. Zero at or below it, so a seat
            // taken from within the offer margin earns the team's base rate and no more.
            let surplus = ((reputation - e.required) / LEVERAGE_RANGE).clamp(0.0, 1.0);
            // Seasons served, as a fraction of what earns the full loyalty rise.
            let loyalty = if renewing && standing.delivered == Some(true) {
                (standing.tenure as f32 / LOYALTY_TENURE as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let rate = match kind {
                OfferKind::Paid => 1.0 + LEVERAGE_BONUS * surplus + params.loyalty_bonus * loyalty,
                // A bought seat is paid at a reduced rate: the team is being paid to have the
                // driver, not paying for one it rates.
                OfferKind::Pay => PAY_DRIVER_RATE,
            };
            let scale = 1.0 + SALARY_JITTER * jitter(champ_id, &e.team);
            let salary = (base_salary(rank, total, params) * rate * scale).round() as i64;

            Some(Offer {
                team: e.team.clone(),
                kind,
                renewal: renewing,
                salary: salary.max(1),
                objective: objective(e.expected_position, field, params.objective_slack, kind),
                buy_in: match kind {
                    OfferKind::Pay => buy_in(e.required - reputation, params),
                    OfferKind::Paid => 0,
                },
                rank,
                required: e.required,
                expected_position: e.expected_position,
            })
        })
        .collect()
}

// ── What the career has earned ───────────────────────────────────────────────

/// Credits for winning a championship. Default for [`PrizeParams::champion_prize`].
const CHAMPION_PRIZE: i64 = 2_000_000;

/// Credits for finishing last of the drivers who scored. Default for
/// [`PrizeParams::floor_prize`].
const FLOOR_PRIZE: i64 = 50_000;

/// The tunable half of prize money. Separate from [`OfferParams`] because the two answer
/// different questions — what a team will pay to have you, and what the series pays out on
/// results — and a career may reasonably want one generous and the other mean.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrizeParams {
    /// Credits for winning a championship.
    pub champion_prize: i64,
    /// Credits for finishing last among the classified drivers.
    pub floor_prize: i64,
}

impl Default for PrizeParams {
    fn default() -> Self {
        PrizeParams {
            champion_prize: CHAMPION_PRIZE,
            floor_prize: FLOOR_PRIZE,
        }
    }
}

/// Payout for finishing `position` of `field` in a championship.
///
/// Scaled by the size of the field rather than by position alone: fifth of six is nearly last
/// and fifth of twenty-six is a good season, and a flat table would pay them the same.
pub fn prize(position: u32, field: usize, params: &PrizeParams) -> i64 {
    if position == 0 {
        return 0;
    }
    if field <= 1 {
        return params.champion_prize.max(0);
    }
    let share = (position - 1) as f32 / (field - 1) as f32;
    geometric(params.champion_prize, params.floor_prize, share).round() as i64
}

/// One contracted season, and what it paid.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SeasonLedger {
    pub champ_id: String,
    /// The championship's name, or empty when the contract outlived the championship it names.
    pub name: String,
    pub team: String,
    /// The car class this season was run in — the stem of its Custom AI file. Empty when the
    /// championship has no roster, or has been deleted. A team name only identifies a team
    /// *within* a class: "Ferrari" appears in seven of the eight shipped rosters.
    pub class: String,
    /// Salary credited. Zero until the season is complete — see [`finances`].
    pub salary: i64,
    /// Prize money credited. Zero until the season is complete.
    pub prize: i64,
    /// Credits paid to take the seat.
    pub bought_for: i64,
    /// Where the driver finished in the standings, if they scored at all.
    pub position: Option<u32>,
    /// Drivers in those standings.
    pub field: usize,
    /// What the team asked for, carried from the contract.
    pub objective: Option<u32>,
    /// Whether it was met. `None` while the season is unfinished, or when there was no target.
    pub objective_met: Option<bool>,
    /// True once the championship is `Final`.
    pub complete: bool,
}

/// Every contracted season and what the career is worth.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Finances {
    /// `starting` plus `earned`, less `spent`. May be negative — see [`finances`].
    pub balance: i64,
    /// What the career began with, before it had raced anything. Recorded on the save at
    /// creation rather than read from config on each request: a career that started with a
    /// million still started with a million after the setting is changed.
    pub starting: i64,
    pub earned: i64,
    pub spent: i64,
    /// Contracted seasons in championship order, with any orphaned contracts last.
    pub seasons: Vec<SeasonLedger>,
}

/// The driver the recorder flagged as the player, across every session it wrote.
///
/// This is [`crate::driver_rating::infer_player_name`]'s first and strongest branch, repeated
/// here rather than reached for because the rating's version needs a roster to fall back on and
/// finances must work for a class that has no Custom AI file.
fn flagged_player(sessions: &[RecordedSession]) -> Option<&str> {
    sessions
        .iter()
        .flat_map(|s| &s.results)
        .find(|r| r.is_player)
        .map(|r| r.name.as_str())
}

/// What every contracted season paid, and the balance that leaves.
///
/// Salary and prize money are credited only once the championship is `Final`. A season in
/// progress has no payout because there is nothing to pro-rate against — rounds are added as
/// they are raced, so a career never declares how long a season is meant to be, and "half way
/// through" is not a figure this data can produce.
///
/// `driver` names the player; when `None` the recorder's own `is_player` flag decides. A career
/// whose player cannot be identified still reports its salaries and its spending, and simply
/// wins no prize money — a wrong name would be worse than a missing one.
///
/// The balance may go negative. Reassigning a session to an older championship can change
/// standings that have already been paid out, and every figure here is re-derived, so money
/// already spent can end up unaffordable. That is the same property the rating has by design;
/// showing it as debt is better than pretending a ledger here is immutable.
pub fn finances(
    contracts: &[Contract],
    champs: &[Championship],
    sessions: &[RecordedSession],
    driver: Option<&str>,
    starting: i64,
    params: &PrizeParams,
) -> Finances {
    let player = driver.or_else(|| flagged_player(sessions));

    // Championship order is the order the rest of the app shows seasons in.
    let mut seasons: Vec<SeasonLedger> = Vec::new();
    for champ in champs {
        let Some(c) = for_championship(contracts, &champ.id) else {
            continue;
        };
        let complete = champ.status == ChampionshipStatus::Final;
        let table = crate::data_store::standings(champ, sessions);
        let position = player.and_then(|name| {
            table
                .iter()
                .position(|e| e.name == name)
                .map(|i| i as u32 + 1)
        });
        seasons.push(SeasonLedger {
            champ_id: c.champ_id.clone(),
            name: champ.name.clone(),
            team: c.team.clone(),
            class: champ
                .custom_ai_file
                .as_deref()
                .map(crate::custom_ai::class_of_file)
                .unwrap_or_default()
                .to_string(),
            salary: if complete { c.salary } else { 0 },
            prize: match (complete, position) {
                (true, Some(p)) => prize(p, table.len(), params),
                _ => 0,
            },
            bought_for: c.bought_for,
            position,
            field: table.len(),
            objective: c.objective,
            objective_met: match (complete, c.objective, position) {
                (true, Some(target), Some(p)) => Some(p <= target),
                _ => None,
            },
            complete,
        });
    }

    // A contract whose championship has been deleted still spent real money, so it keeps a row
    // rather than quietly dropping off the balance.
    for c in contracts {
        if champs.iter().any(|ch| ch.id == c.champ_id) {
            continue;
        }
        seasons.push(SeasonLedger {
            champ_id: c.champ_id.clone(),
            name: String::new(),
            team: c.team.clone(),
            // No championship left to name a class, so this season is in no series at all.
            class: String::new(),
            salary: 0,
            prize: 0,
            bought_for: c.bought_for,
            position: None,
            field: 0,
            objective: c.objective,
            objective_met: None,
            complete: false,
        });
    }

    let earned: i64 = seasons.iter().map(|s| s.salary + s.prize).sum();
    let spent: i64 = seasons.iter().map(|s| s.bought_for).sum();
    Finances {
        balance: starting + earned - spent,
        starting,
        earned,
        spent,
        seasons,
    }
}

#[cfg(test)]
#[path = "tests/contracts.rs"]
mod tests;
