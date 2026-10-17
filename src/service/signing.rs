//! Seats and money: the offers on the table for a season, signing one, tearing it up, and the
//! career's finances.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{
    career_standing, cfg_custom_ai_dir, champ_class, champ_previews, champ_roster_seats, conflict,
    internal, not_found, now_secs, App, AppError, AppResult,
};
use crate::career::contracts::{self, Contract, Finances, Offer, Standing};
use crate::roster::custom_ai::SeatChoice;
use crate::career::data_store::Championship;

/// Every contracted season and what it paid.
#[derive(Serialize, Debug)]
pub struct FinancesView {
    /// False when the career has no contracts — the figures are then advisory only.
    pub enabled: bool,
    /// The seat held and what was done with it, which is what next season's offers turn on.
    /// Shown here so the ledger explains the renewals rather than only listing pay.
    pub standing: Standing,
    #[serde(flatten)]
    pub finances: Finances,
}

/// The seats on the table for one season, with terms.
#[derive(Serialize, Debug)]
pub struct OffersView {
    /// False when the career has no contracts — the offers are then advisory only.
    pub enabled: bool,
    /// False when the championship has no Custom AI file to rate against.
    pub rated: bool,
    /// Whether a seat may still be taken for this season.
    pub open: bool,
    pub reputation: f32,
    /// This season's car class. Shown so a client can explain why an incumbency did not carry:
    /// a seat is only held inside the series it was held in.
    pub class: String,
    /// What the career is worth, so the client can tell which buy-ins are affordable without a
    /// second request. Checked again on signing regardless.
    pub balance: i64,
    /// Teams on the grid, offering or not. `Offer::rank` is a position within this, so without
    /// it a client can say "9th fastest" but not "9th of 21" — and the teams that make no offer
    /// are exactly the ones missing from the list.
    pub teams: usize,
    /// The seat held going into this season, and what was done with it.
    pub standing: Standing,
    /// The deal already agreed for this season, if there is one.
    pub signed: Option<Contract>,
    /// Each team's picture, keyed by team name. A map beside the offers rather than a field on
    /// one, because `contracts::Offer` is the terms of a deal and a picture is not one of them;
    /// the signed contract's row wants the same picture, and it is not an offer at all.
    pub previews: HashMap<String, String>,
    /// The cars each offering team would put the player in, keyed by team name — see
    /// `custom_ai::seat_choices`. Beside the offers for the same reason as `previews`: which car
    /// is not part of the terms, it is what signing asks for alongside them.
    pub seats: HashMap<String, Vec<SeatChoice>>,
    pub offers: Vec<Offer>,
}

/// What taking an offer takes: the team, and optionally which of its cars.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct SignRequest {
    pub team: String,
    /// The car to drive, e.g. "Brabham #7" — one of [`OffersView::seats`] for this team.
    /// Optional so a caller that only names the team still signs: it then gets the first car it
    /// may take.
    #[serde(default)]
    pub seat: Option<String>,
}

/// A deal just signed, and the championship as signing left it.
#[derive(Serialize, Debug)]
pub struct Signed {
    pub contract: Contract,
    pub championship: Championship,
}

impl App {
    /// Every contracted season and what it paid. Derived in full on each call: only the agreed
    /// terms are stored, so salaries, prize money and the balance all follow the results as they
    /// stand right now.
    pub fn finances(&self) -> FinancesView {
        let data = self.data();
        let (finances, standing) = career_standing(&self.config(), &data);
        FinancesView {
            enabled: data.mode.uses_contracts(),
            standing,
            finances,
        }
    }

    /// The seats on the table for this season, with terms.
    ///
    /// Nothing here is stored: an offer is only what a team would say today, so it is rebuilt
    /// from the same eligibility the team picker uses. `open` is what the caller should act on —
    /// a season that already has a team, or that has been raced, is no longer taking offers.
    pub fn offers(&self, id: &str) -> AppResult<OffersView> {
        let data = self.data();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        let cfg = self.config();
        let career = self.active_path();
        let class = champ_class(champ);
        let signed = contracts::for_championship(&data.contracts, id).cloned();
        // A season stops taking offers once it is signed, or once it has been raced — the same
        // first-session rule that locks the team picker. A `player_team` picked directly is not
        // a commitment and does not close it: signing may still replace it, exactly as a patch
        // may before the first session.
        let open = signed.is_none() && champ.rounds.iter().all(|r| r.session_ids.is_empty());

        let (ledger, standing) = career_standing(&cfg, &data);
        let rated = self.champ_eligibility(&career, champ, &data);
        let previews = champ_previews(&self.config_path, &career, champ);
        Ok(match &rated {
            Some((rep, eligibility)) => {
                let offers = contracts::offers_for_with(
                    id,
                    &class,
                    rep.value,
                    ledger.balance,
                    eligibility,
                    &standing,
                    &cfg.offer_params(),
                );
                let roster = champ_roster_seats(&self.config_path, &career, champ);
                let seats = offers
                    .iter()
                    .map(|o| {
                        let choices = crate::roster::custom_ai::seat_choices(&roster, &o.team, rep.value);
                        (o.team.clone(), choices)
                    })
                    .collect();
                OffersView {
                    enabled: data.mode.uses_contracts(),
                    rated: true,
                    open,
                    reputation: rep.value,
                    balance: ledger.balance,
                    class: class.clone(),
                    teams: eligibility.len(),
                    offers,
                    standing,
                    signed,
                    previews,
                    seats,
                }
            }
            None => OffersView {
                enabled: data.mode.uses_contracts(),
                rated: false,
                open,
                reputation: 0.0,
                balance: ledger.balance,
                class,
                teams: 0,
                standing,
                signed,
                previews,
                seats: Default::default(),
                offers: vec![],
            },
        })
    }

    /// Takes one of the offers on the table.
    ///
    /// The caller names only the team (and optionally the car). Terms are regenerated here from
    /// the same inputs that produced the offer list, so a salary cannot be dictated by the
    /// caller, and the deal recorded is the one the grid would actually give today.
    ///
    /// This is the team picker plus a record of what was agreed: it applies the same locks
    /// [`Self::patch_championship`] does rather than working around them.
    pub fn sign(&self, id: &str, req: &SignRequest) -> AppResult<Signed> {
        let career = self.active_path();
        let mut data = self.store.write().unwrap();
        let current = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?
            .clone();

        // Only a singleplayer career signs for seats. A multiplayer one races people, and an
        // unset one predates the question — recording a deal in either would put a salary in a
        // ledger the user never opted into.
        if !data.mode.uses_contracts() {
            return Err(conflict(format!(
                "contracts belong to a singleplayer career; this one is {}.",
                data.mode.label()
            )));
        }
        if contracts::for_championship(&data.contracts, id).is_some() {
            return Err(conflict(
                "This season is already signed. Tear up the contract first to change seat.",
            ));
        }
        // The same first-session rule as the team lock: results already scored were measured
        // against the seat they were scored in.
        if current.rounds.iter().any(|r| !r.session_ids.is_empty()) {
            return Err(conflict(
                "The season has already started. Remove its assigned sessions first.",
            ));
        }
        if current.custom_ai_file.is_none() {
            return Err(conflict(
                "Assign a Custom AI Drivers file first — without a roster there are no teams \
                 to sign for.",
            ));
        }

        let Some((reputation, eligibility)) = self.champ_eligibility(&career, &current, &data)
        else {
            return Err(conflict(
                "No car performance data for this roster, so no terms can be offered.",
            ));
        };
        let cfg = self.config();
        let (ledger, standing) = career_standing(&cfg, &data);
        let offers = contracts::offers_for_with(
            id,
            &champ_class(&current),
            reputation.value,
            ledger.balance,
            &eligibility,
            &standing,
            &cfg.offer_params(),
        );
        let wanted = req.team.trim();
        let Some(offer) = offers.iter().find(|o| o.team.eq_ignore_ascii_case(wanted)) else {
            // No offer means the team is out of reach, or is not on this grid at all. The
            // requirement is the useful half of the answer, so it is quoted when there is one.
            let reason = match eligibility
                .iter()
                .find(|e| e.team.eq_ignore_ascii_case(wanted))
            {
                Some(e) => format!(
                    "{wanted} is not offering a seat: they want a driver rating of {:.0} and \
                     yours is {:.0}. Race for a slower team first.",
                    e.required, reputation.value
                ),
                None => format!("{wanted} is not a team on this grid."),
            };
            return Err(conflict(reason));
        };

        // ── Paying for a seat the rating has not earned ──────────────────────
        // The price is the server's, and so is the check: the balance is derived from results,
        // so a caller cannot talk its way into a car by naming its own figure.
        if offer.buy_in > 0 && ledger.balance < offer.buy_in {
            return Err(conflict(format!(
                "{} wants {} in sponsorship to take you, and the career is worth {}. \
                 Win some prize money first, or race for a team that will have you.",
                offer.team, offer.buy_in, ledger.balance
            )));
        }

        // ── Which car ────────────────────────────────────────────────────────
        // Chosen now and stored with the season, so every session is checked against one car
        // from the first. Only a driver rated no higher than the player may be replaced — or,
        // when that is nobody, the team's weakest. Re-derived here rather than trusted from the
        // caller, like the terms.
        let choices = crate::roster::custom_ai::seat_choices(
            &champ_roster_seats(&self.config_path, &career, &current),
            &offer.team,
            reputation.value,
        );
        let seat = match req.seat.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(wanted) => match choices.iter().find(|c| c.seat.eq_ignore_ascii_case(wanted)) {
                Some(c) => Some(c.clone()),
                None => {
                    let open: Vec<String> = choices
                        .iter()
                        .map(|c| format!("{} (in place of {})", c.seat, c.replaces))
                        .collect();
                    return Err(conflict(format!(
                        "You cannot take {wanted}: with a rating of {:.0} you may only replace a \
                         {} driver rated no higher than you, or the weakest. Open: {}.",
                        reputation.value,
                        offer.team,
                        if open.is_empty() { "none".into() } else { open.join(", ") }
                    )));
                }
            },
            None => choices.first().cloned(),
        };

        // ── The roster is fixed from here on ─────────────────────────────────
        // The deal was offered on this grid, so the season keeps a copy of it and is judged on
        // that copy from now on, whatever happens to the game's file. Taken before anything is
        // recorded: a season that should have fixed its roster and could not is not signed. A
        // legacy flat save has nowhere to keep one and signs as it always did.
        let copied = match (
            cfg_custom_ai_dir(&self.config_path),
            current.custom_ai_file.as_deref(),
        ) {
            (Some(dir), Some(file)) if crate::career::signed_rosters::rosters_dir(&career).is_some() => {
                crate::career::signed_rosters::store(&career, id, &dir.join(file)).map_err(|e| {
                    internal(format!("Not signed — could not keep this season's roster: {e}"))
                })?;
                true
            }
            _ => false,
        };

        let mut contract = Contract::from_offer(id, offer, now_secs());
        contract.bought_for = offer.buy_in;

        let champ = data
            .championships
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        champ.player_team = Some(contract.team.clone());
        champ.player_seat = seat.map(|c| crate::career::data_store::SeatLock {
            seat: c.seat,
            replaces: c.replaces,
        });
        let championship = champ.clone();
        data.contracts.push(contract.clone());
        drop(data);

        if let Err(e) = self.persist_to(&career) {
            // Nothing was signed, so nothing is fixed either.
            if copied {
                crate::career::signed_rosters::remove(&career, id);
            }
            return Err(e);
        }
        Ok(Signed {
            contract,
            championship,
        })
    }

    /// Tears up an unraced contract.
    ///
    /// Without this a mis-click would be permanent: a signed season stops taking offers, so
    /// there would be no way back to the list. Allowed on exactly the same terms as changing the
    /// team — until the season's first session.
    pub fn unsign(&self, id: &str) -> AppResult<()> {
        let mut data = self.store.write().unwrap();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        if champ.rounds.iter().any(|r| !r.session_ids.is_empty()) {
            return Err(conflict(
                "The season has already started, so its contract stands. Remove its assigned \
                 sessions first.",
            ));
        }
        if !data.contracts.iter().any(|c| c.champ_id == id) {
            return Err(not_found("no contract for this season"));
        }
        data.contracts.retain(|c| c.champ_id != id);
        // The seat went with the deal. Clearing it puts the season back exactly where signing
        // found it, so the offer list opens again.
        if let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) {
            champ.player_team = None;
            champ.player_seat = None;
        }
        drop(data);
        let career = self.active_path();
        self.persist_to(&career)?;
        // The roster was kept because of the deal, so it goes with it: the season reads the
        // game's file again until it is signed afresh, which takes a new copy.
        crate::career::signed_rosters::remove(&career, id);
        Ok(())
    }

    /// Copies the season's own roster over the file AMS2 loads, so the game races the grid the
    /// season is judged on. The live tab offers it when the two differ ([`super::LiveTeams`]).
    /// AMS2 reads a roster when a session loads, so nothing changes on track until the session
    /// is restarted.
    pub fn restore_roster(&self, id: &str) -> AppResult<()> {
        let champ = self
            .data()
            .championships
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| not_found("not found"))?;
        let dir = cfg_custom_ai_dir(&self.config_path)
            .ok_or_else(|| AppError::BadRequest("no Custom AI Drivers folder is configured".into()))?;
        let career = self.active_path();
        if crate::career::signed_rosters::stored(&career, id).is_none() {
            return Err(conflict(
                "This season keeps no roster of its own — it reads the game's file already.",
            ));
        }
        crate::career::signed_rosters::restore(&career, &dir, &champ).map_err(AppError::Internal)
    }
}
