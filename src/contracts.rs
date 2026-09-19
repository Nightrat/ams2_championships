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

/// Rating points clear of a back-marker's bar a driver must be before it *pays* them rather than
/// selling them the seat.
///
/// Merely clearing the bar is not enough at the back of the grid. A team there has a budget hole
/// — that is why its seat is for sale at all — so it takes sponsorship from anyone it does not
/// actively want, and only opens its wallet for a driver clearly better than the one already in
/// the car. Osella, AGS and Coloni paid nobody; they took money from everyone bar the occasional
/// talent.
///
/// Without this the slowest team is free to anyone who clears its bar, and that bar is set by its
/// *incumbent* rather than by the grid — the grid gate is zero at the back. A roster whose last
/// team runs a weak driver therefore hands out its seat to an unproven rookie, on any tuning.
///
/// Zero restores the previous behaviour: clearing the bar is enough anywhere on the grid.
///
/// Default for [`OfferParams::pay_driver_margin`].
const PAY_DRIVER_MARGIN: f32 = 10.0;

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
    /// What the season paid, stamped the moment it was marked `Final`. See [`Settlement`].
    ///
    /// `None` means the season is still running, or was finished before sealing existed —
    /// [`finances`] falls back to deriving the prize in that case, so an unsealed save behaves
    /// exactly as it did before. [`seal_finished`] is the one-time upgrade that fills it in.
    #[serde(default)]
    pub settled: Option<Settlement>,
}

/// What a season paid, fixed at the moment it closed.
///
/// Prize money is otherwise re-derived on every request from the *current* [`PrizeParams`],
/// which meant that retuning `champion_prize` or `last_place_prize` in the Config tab silently
/// re-paid every season the career had already finished. Marking a championship `Final` stamps
/// this instead: the payout becomes history, exactly like the salary agreed at signing, and
/// config can no longer reach back into a season that is over.
///
/// **Only the money is stamped.** Position and field are not, because this module records only
/// what results cannot recover — and those can be. The consequence is the intended one: the
/// Config tab cannot move a finished season, while reassigning a session to an old championship
/// still moves the standings it is shown against. The money is settled; the history is not
/// rewritten to match it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settlement {
    /// Prize money awarded, in credits, under the economy in force when the season closed.
    pub prize: i64,
    /// Unix seconds at close. Zero on a season sealed by the upgrade rather than by being
    /// finished, because there is no record of when that happened.
    #[serde(default)]
    pub at: u64,
    /// Salary drawn across the season, stamped for the same reason the prize is.
    ///
    /// The salary *rate* was frozen at signing, so config cannot reach it — but once a season
    /// pays per race ([`salary_earned`]) the total depends on how many rounds it ran, and
    /// reassigning a session away from a closed season would quietly take back wages already
    /// drawn. Closing the books fixes the money; the history stays free to move.
    ///
    /// `None` on a stamp taken before seasons paid per race. Those paid the whole salary at
    /// `Final` and there is nothing to reconstruct, so [`finances`] falls back to exactly that.
    #[serde(default)]
    pub salary: Option<i64>,
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
            settled: None,
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
    /// Rating points clear of a selling team's bar before it pays the driver instead of charging
    /// them. Zero means clearing the bar is enough, anywhere on the grid.
    pub pay_driver_margin: f32,
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
            pay_driver_margin: PAY_DRIVER_MARGIN,
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
        0,
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
    balance: i64,
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

    let offers: Vec<Offer> = eligibility
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

            // What a team at the back would have to see before it pays rather than charges. Above
            // this it wants the driver; below it, the seat is merchandise. See
            // [`PAY_DRIVER_MARGIN`].
            let sells = params.buy_in_per_point > 0 && sells_seat(rank, total, params);
            let wanted = reputation >= e.required + params.pay_driver_margin.max(0.0);

            let kind = if renewing {
                // Delivering keeps the seat, whatever the rating now says. That is what having
                // met the target is *for*; re-earning it every winter would make the objective
                // decorative — and a team that sells seats does not start charging a driver who
                // just delivered for it.
                OfferKind::Paid
            } else {
                match e.tier {
                    // A back-marker sells to anyone it does not actively want, whether or not the
                    // rating clears its bar. Clearing the bar of the slowest car on the grid is a
                    // low hurdle — at the back the grid gate is zero, so the bar is whatever its
                    // incumbent happens to be — and it used to make that seat a free gift.
                    _ if sells && !wanted => OfferKind::Pay,
                    // Above the bar or within reach of it are the same kind of deal. What
                    // separates them is the leverage below, which slides to nothing at the bar
                    // — a gradient rather than the cliff a separate "trial" kind made of it.
                    Tier::Available | Tier::OfferPossible => OfferKind::Paid,
                    // Anything quicker than the sellers that the rating cannot reach stays shut.
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
                    // Priced on the distance to the point where the team would want the driver,
                    // not to its bar — so the price slides continuously to zero exactly where the
                    // seat flips to `Paid`, rather than stepping off a cliff there.
                    OfferKind::Pay => buy_in(
                        e.required + params.pay_driver_margin.max(0.0) - reputation,
                        params,
                    ),
                    OfferKind::Paid => 0,
                },
                rank,
                required: e.required,
                expected_position: e.expected_position,
            })
        })
        .collect();

    open_a_way_in(offers, eligibility, balance, params)
}

/// Guarantees a career is never left with no seat it can take.
///
/// A grid may ask more than an unproven driver has everywhere on it — on most shipped rosters it
/// does — and the sponsorship a locked team wants can easily exceed a new career's whole
/// balance. Left alone that is a dead end: nothing earned, nothing affordable, no way to start
/// earning either.
///
/// So when nothing at all is obtainable, the cheapest seat for sale drops its price to exactly
/// what the career has. The seat still costs everything, which is the point — a way in, not a
/// gift. This replaced a rule in [`crate::driver_rating`] that handed the least demanding team
/// over free; that one could not do better because it did not know what the driver could pay.
fn open_a_way_in(
    mut offers: Vec<Offer>,
    eligibility: &[TeamEligibility],
    balance: i64,
    params: &OfferParams,
) -> Vec<Offer> {
    let obtainable = offers
        .iter()
        .any(|o| o.kind == OfferKind::Paid || o.buy_in <= balance);
    if obtainable {
        return offers;
    }

    // Cheapest seat on the market, discounted to whatever there is.
    if let Some(cheapest) = offers
        .iter_mut()
        .filter(|o| o.kind == OfferKind::Pay)
        .min_by_key(|o| o.buy_in)
    {
        cheapest.buy_in = balance.max(0);
        return offers;
    }

    // Nothing is even for sale — pay-driver seats are switched off. The least demanding team
    // takes the driver anyway, because the alternative is a career that cannot begin.
    if let Some(easiest) = eligibility
        .iter()
        .min_by(|a, b| a.required.total_cmp(&b.required))
    {
        let rank = eligibility
            .iter()
            .position(|e| e.team == easiest.team)
            .unwrap_or(0);
        let field = eligibility
            .iter()
            .map(|e| e.expected_position)
            .fold(0.0f32, f32::max);
        offers.push(Offer {
            team: easiest.team.clone(),
            kind: OfferKind::Paid,
            renewal: false,
            salary: (base_salary(rank, eligibility.len(), params)).round() as i64,
            objective: objective(
                easiest.expected_position,
                field,
                params.objective_slack,
                OfferKind::Paid,
            ),
            buy_in: 0,
            rank,
            required: easiest.required,
            expected_position: easiest.expected_position,
        });
        offers.sort_by_key(|o| o.rank);
    }
    offers
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
    /// Salary credited so far — the wage for the races run, or the whole figure at `Final` for
    /// a season with no declared calendar. See [`salary_earned`].
    pub salary: i64,
    /// The whole season's wage as it was agreed, which [`Self::salary`] is drawn against.
    ///
    /// Carried separately because the two answer different questions — what has been banked,
    /// and what the deal is worth — and a client showing "drawn 11,264 of 33,792" needs both.
    /// A capped wage means this is a ceiling: see [`salary_earned`].
    pub salary_contracted: i64,
    /// Races run so far, so a client can show what the salary has been paid against.
    pub races_run: u32,
    /// The declared calendar, or `None` for a season that never had one.
    pub planned_rounds: Option<u32>,
    /// What the season would pay out if it ended on today's standings.
    ///
    /// `None` once the season is complete, because then [`Self::prize`] is not a projection but
    /// the settled fact. Derived server-side rather than in the browser for the same reason
    /// `offerWhy()` never recomputes a rate: the prize curve belongs to [`prize`], and a second
    /// copy of it in a client would drift from this one.
    pub projected_prize: Option<i64>,
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
    /// True once the championship is `Final` — and so, since [`seal_finished`] runs whenever a
    /// career is loaded, once its payout is settled and beyond the reach of the Config tab.
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

/// Races actually run in a season: rounds holding at least one race session.
///
/// A round groups practice, qualifying and the race, so counting rounds alone would pay a wage
/// for a weekend that was only practised. Session type 5 is the race — the same test
/// [`crate::data_store::standings`] scores on, so a round that pays is a round that counted.
fn races_run(champ: &Championship, sessions: &[RecordedSession]) -> u32 {
    champ
        .rounds
        .iter()
        .filter(|r| {
            r.session_ids.iter().any(|id| {
                sessions
                    .iter()
                    .any(|s| s.id == *id && s.session_type == crate::data_store::SESSION_RACE)
            })
        })
        .count() as u32
}

/// Salary drawn so far, in credits — the wage for the races actually run.
///
/// A contract's `salary` is the figure for a whole season. Split across the declared calendar
/// it becomes a per-race wage, paid as each race is recorded rather than in one lump at the
/// end, which is what [`Championship::planned_rounds`] exists to make possible.
///
/// **Capped, and never topped up.** Running the full calendar draws the whole salary; stopping
/// short draws only what was raced, and a season that overruns its calendar earns nothing
/// beyond it. So the contracted figure is a ceiling rather than a promise — the driver is paid
/// for the races they turned up to, and the team does not pay twice for a longer season.
///
/// A season with no declared calendar keeps the old rule exactly: nothing until `Final`, then
/// the whole salary. That is every season written before this existed.
pub fn salary_earned(
    contract: &Contract,
    champ: &Championship,
    sessions: &[RecordedSession],
) -> i64 {
    // A settled season pays what it was stamped as paying. A stamp from before per-race pay
    // carries no salary, and those seasons drew the whole of it.
    if let Some(s) = &contract.settled {
        return s.salary.unwrap_or(contract.salary);
    }
    let Some(planned) = champ.planned_rounds.filter(|n| *n > 0) else {
        return if champ.status == ChampionshipStatus::Final {
            contract.salary
        } else {
            0
        };
    };
    let run = races_run(champ, sessions).min(planned);
    // Saturating because `salary` is hand-editable in the career file; rounded down, so the
    // instalments can never sum past the contracted figure.
    contract.salary.saturating_mul(run as i64) / planned as i64
}

/// What every contracted season paid, and the balance that leaves.
///
/// Prize money is credited only once the championship is `Final` — it pays on a final standings
/// position, and there is no such thing until the season is over. Salary is credited race by
/// race against the declared calendar; see [`salary_earned`] for the season that has none.
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
            salary: salary_earned(c, champ, sessions),
            salary_contracted: c.salary,
            races_run: races_run(champ, sessions),
            planned_rounds: champ.planned_rounds,
            // What today's standings would pay. A season that is over reports the real thing.
            projected_prize: match (complete, position) {
                (false, Some(p)) => Some(prize(p, table.len(), params)),
                (false, None) => Some(0),
                (true, _) => None,
            },
            // A sealed season pays what it paid. Only an unsealed one — still being raced, or
            // finished before sealing existed — is re-derived from the economy in force now.
            prize: match (complete, &c.settled, position) {
                (true, Some(s), _) => s.prize,
                (true, None, Some(p)) => prize(p, table.len(), params),
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
            // No championship left to say how many races were run, or against what calendar —
            // and so nothing to project a payout from either.
            salary: 0,
            salary_contracted: c.salary,
            races_run: 0,
            planned_rounds: None,
            projected_prize: None,
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

// ── Closing a season ─────────────────────────────────────────────────────────

/// Stamps what a season paid onto its contract, if it has one and is `Final`.
///
/// Call this on the transition *into* `Final`. It is idempotent, and deliberately so: a
/// contract that already carries a [`Settlement`] keeps it, for the same reason
/// `custom_ai::ensure_baseline` is one-time. The first stamp was taken under the economy the
/// season was actually raced under, and re-taking it later would quietly redefine what the
/// season paid — which is the exact problem sealing exists to remove.
///
/// Returns whether a stamp was written, so a caller can tell whether it has anything to persist.
pub fn settle(
    contracts: &mut [Contract],
    champ: &Championship,
    sessions: &[RecordedSession],
    driver: Option<&str>,
    params: &PrizeParams,
    now: u64,
) -> bool {
    if champ.status != ChampionshipStatus::Final {
        return false;
    }
    let player = driver.or_else(|| flagged_player(sessions));
    let Some(contract) = contracts.iter_mut().find(|c| c.champ_id == champ.id) else {
        return false;
    };
    if contract.settled.is_some() {
        return false;
    }
    // The same derivation `finances` runs, taken once and kept. A driver who cannot be
    // identified in the standings wins nothing, which is what the derivation said too.
    let table = crate::data_store::standings(champ, sessions);
    let position = player.and_then(|name| {
        table
            .iter()
            .position(|e| e.name == name)
            .map(|i| i as u32 + 1)
    });
    // Taken before the stamp exists, so it reads the derivation rather than itself.
    let salary = salary_earned(contract, champ, sessions);
    contract.settled = Some(Settlement {
        prize: position.map_or(0, |p| prize(p, table.len(), params)),
        at: now,
        salary: Some(salary),
    });
    true
}

/// Tears up a season's settlement, so that finishing it again takes a fresh one.
///
/// Reopening a finished season takes its payout back. That was true while the payout was
/// derived and it stays true now: the stamp records what a season paid *on closing*, so a
/// season that is no longer closed must not carry one.
///
/// Returns whether a stamp was removed.
pub fn unsettle(contracts: &mut [Contract], champ_id: &str) -> bool {
    contracts
        .iter_mut()
        .find(|c| c.champ_id == champ_id)
        .is_some_and(|c| c.settled.take().is_some())
}

/// Seals every `Final` season that has a contract but no [`Settlement`], at what it pays under
/// `params` right now. Returns how many were sealed — zero means there is nothing to persist.
///
/// This is the one-time upgrade for a career finished before sealing existed, and *when* it
/// runs is the whole of its correctness. Run once at load, against the config the career has
/// been running on, every figure it writes is the figure the ledger was already showing and the
/// upgrade is invisible. Run it on the request path instead and it would seal those seasons at
/// whatever the economy had been retuned to in the meantime — sealing the wrong numbers, with
/// no way back. So it belongs beside `config::load_and_upgrade`: at startup, and on the one
/// other path that loads a career, `POST /api/saves/activate`.
///
/// The stamps it writes carry `at: 0`. There is no record of when these seasons were finished,
/// and inventing "now" would date a 2023 season to the day the app was upgraded.
pub fn seal_finished(
    contracts: &mut [Contract],
    champs: &[Championship],
    sessions: &[RecordedSession],
    driver: Option<&str>,
    params: &PrizeParams,
) -> usize {
    let mut sealed = 0;
    for champ in champs {
        if settle(contracts, champ, sessions, driver, params, 0) {
            sealed += 1;
        }
    }
    sealed
}

#[cfg(test)]
#[path = "tests/contracts.rs"]
mod tests;
