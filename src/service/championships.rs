//! Championships, their rounds and the sessions assigned to them — and the rules that guard
//! each change: what a career's mode allows, what the first session locks, and which car a
//! season is raced in.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    bad, cfg_custom_ai_dir, champ_roster_file, champ_roster_seats, conflict, double_option,
    not_found, now_secs, App, AppResult,
};
use crate::roster::custom_ai::{self, GridEntry, GridFit, SeatEntry};
use crate::career::data_store::{
    CareerMode, CareerResponse, Championship, ChampionshipStatus, LapChartEntry, RecordedSession,
    Round, SeatLock,
};

/// What creating a championship takes.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct NewChampionship {
    pub name: String,
    #[serde(default)]
    pub points_system: Vec<i32>,
    #[serde(default)]
    pub manufacturer_scoring: bool,
    /// The roster this season runs against. Required in singleplayer — a season is defined by
    /// the grid it is raced on — and refused in multiplayer.
    #[serde(default)]
    pub custom_ai_file: Option<String>,
    /// How many races the season is meant to run. Required wherever contracts exist, because it
    /// is the denominator a salary is paid out against — see `contracts::salary_earned`.
    /// Optional elsewhere: a career with no contracts has no wage to split.
    #[serde(default)]
    pub planned_rounds: Option<u32>,
}

/// A change to a championship. Every field left `None` is left alone.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ChampionshipPatch {
    pub name: Option<String>,
    pub status: Option<ChampionshipStatus>,
    pub points_system: Option<Vec<i32>>,
    pub manufacturer_scoring: Option<bool>,
    /// The declared calendar. Not locked by the first session the way the roster and the seat
    /// are: under a capped, never-topped-up wage, changing it only resizes the instalments still
    /// to come and re-derives what has been drawn. It is also the one way a season created
    /// before calendars existed can start paying per race.
    pub planned_rounds: Option<u32>,
    // Outer Option = key present or not (leave unchanged if absent);
    // inner Option = explicit null clears the assignment.
    #[serde(default, deserialize_with = "double_option")]
    pub custom_ai_file: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub player_team: Option<Option<String>>,
}

/// Driver rating plus which teams it opens, for the team picker.
#[derive(Serialize, Debug)]
pub struct TeamEligibilityView {
    /// False when the config checkbox is off — tiers are then advisory only.
    pub enforced: bool,
    /// Whether the picker should drop locked teams rather than showing what they ask.
    pub hide_locked: bool,
    /// False when the championship has no Custom AI file to rate against.
    pub rated: bool,
    pub reputation: crate::career::driver_rating::Reputation,
    pub teams: Vec<crate::career::driver_rating::TeamEligibility>,
}

/// How each of a season's recorded sessions lined up with the roster it is raced on.
#[derive(Serialize, Debug)]
pub struct GridCheck {
    /// False when there is nothing to check against — no folder, no roster, or a file that
    /// would not parse. The tab then says why rather than reporting all clear.
    pub checked: bool,
    /// Why it could not be checked, when it could not.
    pub reason: Option<String>,
    /// Cars the roster can field, which is the grid size to race it at.
    pub seats: usize,
    /// One line for the season as a whole, or `None` when every session was clean.
    pub summary: Option<String>,
    pub sessions: Vec<GridCheckSession>,
}

#[derive(Serialize, Debug)]
pub struct GridCheckSession {
    pub id: String,
    pub track: String,
    pub session_type: u32,
    pub fit: GridFit,
    pub note: Option<String>,
}

/// Which recorded sessions may join a championship. `enforced` is false unless a Custom AI file
/// *and* a player team are both set; the session picker only hides anything when it is true.
#[derive(Serialize, Debug, Default)]
pub struct SessionEligibility {
    pub enforced: bool,
    /// Session id -> why it would be refused.
    pub blocked: HashMap<String, String>,
}

/// What the session picker offers for one championship — see [`App::available_sessions`].
#[derive(Serialize, Debug)]
pub struct AvailableSessions {
    pub sessions: Vec<SessionListing>,
    /// Unassigned sessions left out because adding them would be refused.
    pub hidden: usize,
}

/// A recorded session as the session lists show it.
#[derive(Serialize, Debug, Clone)]
pub struct SessionListing {
    #[serde(flatten)]
    pub session: RecordedSession,
    /// Whoever was classified P1 — see [`RecordedSession::winner`].
    pub winner: Option<String>,
}

impl App {
    /// Every recorded session, assigned or not, with its winner.
    pub fn sessions(&self) -> Vec<SessionListing> {
        self.data()
            .sessions
            .iter()
            .map(|s| SessionListing {
                winner: s.winner().map(str::to_string),
                session: s.clone(),
            })
            .collect()
    }

    /// Sessions no round of any championship holds — what "Delete unassigned sessions" removes.
    pub fn unassigned_sessions(&self) -> Vec<SessionListing> {
        let assigned: HashSet<String> = self
            .data()
            .championships
            .iter()
            .flat_map(|c| c.rounds.iter())
            .flat_map(|r| r.session_ids.iter().cloned())
            .collect();
        self.sessions()
            .into_iter()
            .filter(|s| !assigned.contains(&s.session.id))
            .collect()
    }

    /// The sessions the picker offers for a round of this championship: every unassigned one,
    /// minus those [`Self::session_eligibility`] would refuse — the same verdict adding gives, so
    /// the picker never offers a session the add would turn down. `hidden` says how many were
    /// left out, so the picker can say so rather than look short.
    pub fn available_sessions(&self, id: &str) -> AppResult<AvailableSessions> {
        let eligibility = self.session_eligibility(id)?;
        let mut hidden = 0;
        let sessions = self
            .unassigned_sessions()
            .into_iter()
            .filter(|s| {
                let blocked = eligibility.enforced && eligibility.blocked.contains_key(&s.session.id);
                hidden += usize::from(blocked);
                !blocked
            })
            .collect();
        Ok(AvailableSessions { sessions, hidden })
    }

    /// Every championship, in the order they are listed — see [`crate::career::data_store::display_order`].
    pub fn championships(&self) -> Vec<Championship> {
        let mut champs = self.data().championships.clone();
        champs.sort_by(|a, b| {
            crate::career::data_store::display_order((&a.status, &a.name), (&b.status, &b.name))
        });
        champs
    }

    /// Standings, constructor standings and career stats, all derived from the raw sessions.
    /// Championships come in the order they are listed, like [`Self::championships`].
    pub fn career_view(&self) -> CareerResponse {
        let data = self.data();
        let cfg = self.config();
        let ai_dir = cfg.custom_ai_dir.as_deref().map(std::path::PathBuf::from);
        let mut career = crate::career::data_store::compute_career_full(
            &data.championships,
            &data.sessions,
            ai_dir.as_deref(),
            &self.active_path(),
        );
        career.championships.sort_by(|a, b| {
            crate::career::data_store::display_order((&a.status, &a.name), (&b.status, &b.name))
        });
        career
    }

    /// Settles a career that predates career modes.
    ///
    /// The *only* transition there is. A mode is picked when a career is created and kept,
    /// because the two kinds allow different things: switching an established career would leave
    /// it holding seasons it could not have made. `Unset` is the exception because those saves
    /// were never asked, so the UI asks once and this records the answer.
    pub fn set_career_mode(&self, mode: CareerMode) -> AppResult<CareerMode> {
        if mode == CareerMode::Unset {
            return Err(bad("choose singleplayer or multiplayer"));
        }
        {
            let mut data = self.store.write().unwrap();
            if data.mode != CareerMode::Unset {
                return Err(conflict(format!(
                    "this career is already {} and cannot be changed. \
                     Create a new career to race the other way.",
                    data.mode.label()
                )));
            }
            data.mode = mode;
        }
        self.persist()?;
        Ok(mode)
    }

    /// One session's lap chart, fetched when it is opened.
    ///
    /// Charts are not in [`CareerResponse`] and not in the store: they are the bulk of a career
    /// and are looked at one session at a time. Empty rather than an error for a session with no
    /// chart — a practice session never has one, and that is not an error.
    pub fn lap_chart(&self, session_id: &str) -> Vec<LapChartEntry> {
        let stored = crate::career::lap_charts::read(&self.active_path(), session_id);
        if !stored.is_empty() {
            return stored;
        }
        // A legacy flat save has nowhere to keep a chart, so its charts are still in the career.
        self.data()
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.lap_chart.clone())
            .unwrap_or_default()
    }

    /// Removes every session not assigned to any round, with its lap chart. Returns how many.
    pub fn purge_unassigned_sessions(&self) -> AppResult<usize> {
        let mut data = self.store.write().unwrap();
        let assigned: HashSet<String> = data
            .championships
            .iter()
            .flat_map(|c| c.rounds.iter())
            .flat_map(|r| r.session_ids.iter().cloned())
            .collect();
        let before = data.sessions.len();
        // Their charts go with them: a chart nothing points at any more is just a stray file.
        let career = self.active_path();
        for s in data.sessions.iter().filter(|s| !assigned.contains(&s.id)) {
            crate::career::lap_charts::remove(&career, &s.id);
        }
        data.sessions.retain(|s| assigned.contains(&s.id));
        let removed = before - data.sessions.len();
        drop(data);
        self.persist()?;
        Ok(removed)
    }

    /// Creates a championship, after checking what the career's mode allows.
    pub fn create_championship(&self, req: NewChampionship) -> AppResult<Championship> {
        // ── What this career's mode allows ───────────────────────────────────
        let mode = self.data().mode;
        let roster = req.custom_ai_file.filter(|f| !f.trim().is_empty());
        if !mode.uses_roster() && roster.is_some() {
            return Err(conflict(
                "a multiplayer career races people, not a Custom AI roster.",
            ));
        }
        if mode == CareerMode::Singleplayer && roster.is_none() {
            return Err(bad(
                "choose a Custom AI Drivers file — a singleplayer season is defined by the \
                 grid it is raced on.",
            ));
        }
        // The calendar has to be declared up front because nothing else can supply it: rounds
        // are added as they are raced, so a season in progress cannot say how far through it
        // is. Without it a wage has no denominator and can only be paid in one lump at the end.
        let planned = req.planned_rounds.filter(|n| *n > 0);
        if mode.uses_contracts() && planned.is_none() {
            return Err(bad(
                "set how many races the season runs — a salary is paid out across them.",
            ));
        }
        // One season at a time: the next drive is offered on the strength of the last one, so
        // the last one has to be over before there is anything to offer against.
        if mode.one_season_at_a_time() {
            let unfinished: Vec<String> = self
                .data()
                .championships
                .iter()
                .filter(|c| c.status != ChampionshipStatus::Final)
                .map(|c| c.name.clone())
                .collect();
            if !unfinished.is_empty() {
                return Err(conflict(format!(
                    "finish the current season first — {} is still running. \
                     Mark it Final on the Manage tab.",
                    unfinished.join(", ")
                )));
            }
        }

        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        let champ = Championship {
            id,
            name: req.name,
            status: mode.new_season_status(),
            points_system: if req.points_system.is_empty() {
                vec![25, 18, 15, 12, 10, 8, 6, 4, 2, 1]
            } else {
                req.points_system
            },
            manufacturer_scoring: req.manufacturer_scoring,
            rounds: vec![],
            session_ids: vec![],
            player_seat: None,
            custom_ai_file: roster,
            // The seat is taken by signing, never by creating — see `App::sign`.
            player_team: None,
            planned_rounds: planned,
        };

        self.store.write().unwrap().championships.push(champ.clone());
        self.persist()?;
        Ok(champ)
    }

    /// Applies a change to a championship, refusing any the career's mode or the season's
    /// progress does not allow. A refused change leaves the championship untouched.
    pub fn patch_championship(&self, id: &str, req: ChampionshipPatch) -> AppResult<Championship> {
        let mut data = self.store.write().unwrap();
        let Some(current) = data.championships.iter().find(|c| c.id == id) else {
            return Err(not_found("not found"));
        };

        // Whether this season was already over when the request arrived. Taken as a value
        // rather than read off `current` later, because the mutations below need `data` mutably
        // and would otherwise be held back by that borrow.
        let was_final = current.status == ChampionshipStatus::Final;
        // Same reason: what the car stored on the season was chosen against.
        let was_seated = (current.player_team.clone(), current.custom_ai_file.clone());

        // The championship as it would be after this request, used by both checks below so a
        // rejection leaves it untouched.
        let mut prospective = current.clone();
        if let Some(caf) = req.custom_ai_file.clone() {
            prospective.custom_ai_file = caf;
            if prospective.custom_ai_file.is_none() {
                prospective.player_team = None;
            }
        }
        if let Some(pt) = req.player_team.clone() {
            prospective.player_team = if prospective.custom_ai_file.is_some() {
                pt
            } else {
                None
            };
        }
        // Whether the championship is under way. The first assigned session commits both the
        // roster and the seat: the results already scored were measured against that grid.
        let started = current.rounds.iter().any(|r| !r.session_ids.is_empty());
        let mode = data.mode;

        // ── What this career's mode allows ───────────────────────────────────
        if !mode.uses_roster()
            && (prospective.custom_ai_file != current.custom_ai_file
                || prospective.player_team != current.player_team)
        {
            return Err(conflict("a multiplayer career has no roster and no team."));
        }
        // In singleplayer the seat comes from a signed contract and nowhere else, so the team
        // picker is closed. `App::sign` is the way in; `App::unsign` is the way out.
        if mode.uses_contracts() && prospective.player_team != current.player_team {
            return Err(conflict(
                "sign for a team instead — a singleplayer seat comes from a contract.",
            ));
        }
        // `Progress` distinguishes an unfinished season from *the* current one, and singleplayer
        // never has two to tell apart. Its whole state machine is the season being raced, then
        // the season being over.
        if !mode.uses_progress_state() && req.status.as_ref() == Some(&ChampionshipStatus::Progress)
        {
            return Err(conflict(
                "a singleplayer season is either the one being raced or finished.",
            ));
        }
        // A finished singleplayer season has paid out, and the next one was created on the
        // strength of it being over. Reopening it would unpick both.
        if mode.final_is_terminal()
            && current.status == ChampionshipStatus::Final
            && req.status.as_ref().is_some_and(|s| *s != ChampionshipStatus::Final)
        {
            return Err(conflict("a finished season stays finished."));
        }

        // ── The Custom AI file is committed once the championship is under way ─
        // Every expected position, team requirement and rating figure is relative to this
        // file's grid, so swapping it mid-season would silently re-score the rounds already
        // run. Like the team lock below, this is a championship integrity rule rather than a
        // rating one, so the Config switch does not disable it.
        if prospective.custom_ai_file != current.custom_ai_file && started {
            return Err(conflict(
                "The Custom AI Drivers file is locked once a championship has its first \
                 session. Remove the assigned sessions first to change it.",
            ));
        }
        // Signing also commits it: the contract was offered on that grid, and the season keeps a
        // copy of it (see `signed_rosters`) that a different file would contradict.
        if prospective.custom_ai_file != current.custom_ai_file
            && crate::career::contracts::for_championship(&data.contracts, id).is_some()
        {
            return Err(conflict(
                "The Custom AI Drivers file is locked once a contract is signed. Tear up the \
                 contract first to change it.",
            ));
        }

        let claimed = prospective
            .player_team
            .clone()
            .filter(|t| !t.trim().is_empty());
        // Only a *change* of team is gated; leaving an existing one alone must keep working even
        // if the rating has since dropped, or the roster has changed underneath it.
        let changed = claimed.as_deref() != current.player_team.as_deref();

        // ── The team is committed once the championship is under way ─────────
        // Swapping seats mid-season would rewrite the meaning of results already scored, so the
        // first assigned session locks it in. This is a championship integrity rule rather than
        // a rating one, so the Config switch does not disable it.
        if changed && started {
            return Err(conflict(
                "The team is locked once a championship has its first session. \
                 Remove the assigned sessions first to change it.",
            ));
        }

        // ── Team eligibility enforcement ─────────────────────────────────────
        // Claiming a seat the driver rating has not earned is refused, unless the user has
        // switched enforcement off in Config.
        if self.config().enforce_team_eligibility {
            if let (true, Some(team)) = (changed, claimed) {
                let refused = self
                    .champ_eligibility(&self.active_path(), &prospective, &data)
                    .filter(|(_, elig)| !crate::career::driver_rating::is_allowed(elig, &team))
                    .map(|(rep, elig)| {
                        let need = elig
                            .iter()
                            .find(|e| e.team.eq_ignore_ascii_case(team.trim()))
                            .map(|e| e.required)
                            .unwrap_or(100.0);
                        format!(
                            "{team} needs a driver rating of {need:.0}; yours is {:.0}. \
                             Race for a slower team first, or turn off team enforcement in Config.",
                            rep.value
                        )
                    });
                if let Some(reason) = refused {
                    return Err(conflict(reason));
                }
            }
        }

        // Only one championship may be Active at a time.
        if req.status == Some(ChampionshipStatus::Active) {
            for c in data.championships.iter_mut() {
                if c.id != id && c.status == ChampionshipStatus::Active {
                    c.status = ChampionshipStatus::Progress;
                }
            }
        }
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            return Err(not_found("not found"));
        };
        if let Some(name) = req.name {
            champ.name = name;
        }
        if let Some(status) = req.status {
            champ.status = status;
        }
        if let Some(ps) = req.points_system {
            champ.points_system = ps;
        }
        if let Some(ms) = req.manufacturer_scoring {
            champ.manufacturer_scoring = ms;
        }
        // Floored at one race: a calendar of zero is not a season, and it is the denominator a
        // wage is divided by.
        if let Some(pr) = req.planned_rounds {
            champ.planned_rounds = Some(pr.max(1));
        }
        if let Some(caf) = req.custom_ai_file {
            champ.custom_ai_file = caf;
            // A player team is only meaningful against a Custom AI roster — it is what the seat
            // inference checks it against — so unassigning the file also clears the team.
            if champ.custom_ai_file.is_none() {
                champ.player_team = None;
            }
        }
        if let Some(pt) = req.player_team {
            champ.player_team = if champ.custom_ai_file.is_some() {
                pt
            } else {
                None
            };
        }
        // A car belongs to a team on a roster; changing either leaves it meaningless. Both are
        // locked once the season is raced, so this only ever clears a season with no sessions.
        if (&champ.player_team, &champ.custom_ai_file) != (&was_seated.0, &was_seated.1) {
            champ.player_seat = None;
        }
        let result = champ.clone();

        // ── Closing a season settles what it paid ────────────────────────────
        // Prize money stops being derived the moment the season is over, so that retuning the
        // economy afterwards cannot reach back into seasons the career has already finished.
        // Reopening one tears the settlement up again — the same rule read the other way, and
        // the reason this keys off the *transition* rather than the resulting status.
        let closed = result.status == ChampionshipStatus::Final;
        if closed != was_final {
            let prizes = self.config().prize_params();
            let d = &mut *data;
            if closed {
                if let Some(champ) = d.championships.iter().find(|c| c.id == id) {
                    crate::career::contracts::settle(
                        &mut d.contracts,
                        champ,
                        &d.sessions,
                        None,
                        &prizes,
                        now_secs(),
                    );
                }
            } else {
                crate::career::contracts::unsettle(&mut d.contracts, id);
            }
        }
        drop(data);
        self.persist()?;
        Ok(result)
    }

    /// Deletes a championship, and the roster copy it kept.
    pub fn delete_championship(&self, id: &str) -> AppResult<()> {
        let mut data = self.store.write().unwrap();
        let before = data.championships.len();
        data.championships.retain(|c| c.id != id);
        if data.championships.len() == before {
            return Err(not_found("not found"));
        }
        drop(data);
        let career = self.active_path();
        self.persist_to(&career)?;
        // A season's roster belongs to the season; with nothing left to point at it, it is a
        // stray file.
        crate::career::signed_rosters::remove(&career, id);
        Ok(())
    }

    /// Adds a new empty round.
    pub fn add_round(&self, id: &str) -> AppResult<Championship> {
        let mut data = self.store.write().unwrap();
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            return Err(not_found("not found"));
        };
        champ.rounds.push(Round::default());
        let result = champ.clone();
        drop(data);
        self.persist()?;
        Ok(result)
    }

    /// Removes a round, and re-derives the season's car from the sessions left.
    pub fn remove_round(&self, id: &str, ridx: usize) -> AppResult<Championship> {
        let career = self.active_path();
        let mut data = self.store.write().unwrap();
        let d = &mut *data;
        let Some(champ) = d.championships.iter_mut().find(|c| c.id == id) else {
            return Err(not_found("not found"));
        };
        if ridx >= champ.rounds.len() {
            return Err(not_found("round not found"));
        }
        champ.rounds.remove(ridx);
        reseat(champ, &d.sessions, &d.contracts, &self.config_path, &career);
        let result = champ.clone();
        drop(data);
        self.persist()?;
        Ok(result)
    }

    /// Adds a recorded session to a round, if the season's seat rules allow it.
    ///
    /// Player-team enforcement applies only when the championship has a Custom AI file, which is
    /// also the only way a player team can be set. Without that roster there is nothing to infer
    /// the player's seat from, so the session is accepted unchecked.
    ///
    /// Once the season's sessions have shown which of the team's cars the player drives, that
    /// car is stored on the season (`player_seat`) and every later session must be in it — the
    /// team name alone would accept the team-mate's car too.
    pub fn add_session_to_round(&self, id: &str, ridx: usize, sid: &str) -> AppResult<Championship> {
        let career = self.active_path();
        let mut data = self.store.write().unwrap();
        let Some(champ) = data.championships.iter().find(|c| c.id == id) else {
            return Err(not_found("not found"));
        };
        if ridx >= champ.rounds.len() {
            return Err(not_found("round not found"));
        }

        let seats = champ_roster_seats(&self.config_path, &career, champ);
        let (rejection, lock) = check_session_seat(
            champ,
            &data.sessions,
            &seats,
            sid,
            data.mode.requires_exact_car(),
        );
        if let Some(reason) = rejection {
            return Err(conflict(reason));
        }

        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            return Err(not_found("not found"));
        };
        let round = &mut champ.rounds[ridx];
        if !round.session_ids.contains(&sid.to_string()) {
            round.session_ids.push(sid.to_string());
        }
        if champ.player_seat.is_none() {
            champ.player_seat = lock;
        }
        let result = champ.clone();
        drop(data);
        self.persist()?;
        Ok(result)
    }

    /// Removes a session from a round, and re-derives the season's car from the sessions left.
    pub fn remove_session_from_round(
        &self,
        id: &str,
        ridx: usize,
        sid: &str,
    ) -> AppResult<Championship> {
        let career = self.active_path();
        let mut data = self.store.write().unwrap();
        let d = &mut *data;
        let Some(champ) = d.championships.iter_mut().find(|c| c.id == id) else {
            return Err(not_found("not found"));
        };
        if ridx >= champ.rounds.len() {
            return Err(not_found("round not found"));
        }
        champ.rounds[ridx].session_ids.retain(|s| s != sid);
        reseat(champ, &d.sessions, &d.contracts, &self.config_path, &career);
        let result = champ.clone();
        drop(data);
        self.persist()?;
        Ok(result)
    }

    /// Captures the live session by hand, outside the recorder's automatic triggers.
    pub fn record_current_session(&self) -> AppResult<()> {
        crate::game::recorder::capture_current(&self.store, &self.active_path())
            .map_err(super::AppError::Conflict)
    }

    /// Distinct team names from the championship's roster, for the "player team" picker.
    pub fn championship_teams(&self, id: &str) -> AppResult<Vec<String>> {
        let data = self.data();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        Ok(champ_roster_file(&self.config_path, &self.active_path(), champ)
            .map(|file| custom_ai::list_teams(&file))
            .unwrap_or_default())
    }

    /// Driver rating plus which teams it opens in this championship.
    pub fn team_eligibility(&self, id: &str) -> AppResult<TeamEligibilityView> {
        let data = self.data();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        let cfg = self.config();
        let enforced = cfg.enforce_team_eligibility;
        let hide_locked = cfg.hide_locked_teams;
        Ok(match self.champ_eligibility(&self.active_path(), champ, &data) {
            Some((reputation, teams)) => TeamEligibilityView {
                enforced,
                hide_locked,
                rated: true,
                reputation,
                teams,
            },
            None => TeamEligibilityView {
                enforced,
                hide_locked,
                rated: false,
                reputation: Default::default(),
                teams: vec![],
            },
        })
    }

    /// How each of this season's recorded sessions lined up with the roster it is raced on.
    ///
    /// Its own call rather than a field on the championship: it reads every assigned session's
    /// grid against the roster, which is work the Manage tab should pay for only when it is
    /// showing a season.
    pub fn grid_check(&self, id: &str) -> AppResult<GridCheck> {
        let data = self.data();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;

        let dir = cfg_custom_ai_dir(&self.config_path);
        let mut body = GridCheck {
            checked: false,
            reason: None,
            seats: 0,
            summary: None,
            sessions: Vec::new(),
        };
        let seats = match (&dir, champ.custom_ai_file.as_deref()) {
            (None, _) => {
                body.reason = Some("No Custom AI Drivers folder is set in Config.".into());
                Vec::new()
            }
            (Some(_), None) => {
                body.reason = Some(
                    "This season has no Custom AI Drivers file, so there is no grid to check against."
                        .into(),
                );
                Vec::new()
            }
            (Some(_), Some(_)) => champ_roster_seats(&self.config_path, &self.active_path(), champ),
        };
        if body.reason.is_none() && seats.is_empty() {
            body.reason =
                Some("That Custom AI Drivers file lists no cars this install can field.".into());
        }
        body.checked = !seats.is_empty();
        // Cars, not roster entries: a seat with a stand-in or a second livery is still one car,
        // and this number has to be the one `GridFit` measures against or the panel and the
        // notes under it would state different grid sizes.
        body.seats = custom_ai::car_count(&seats);

        if body.checked {
            let assigned: Vec<&str> = champ
                .rounds
                .iter()
                .flat_map(|r| r.session_ids.iter().map(String::as_str))
                .collect();
            let mut flagged = 0usize;
            let mut rated = 0usize;
            for sid in assigned {
                let Some(s) = data.sessions.iter().find(|s| s.id == sid) else {
                    continue;
                };
                // Practice is excluded for the same reason the career view excludes it: nothing
                // is derived from it, so a short practice grid is a choice, not a mistake.
                if s.session_type == crate::career::data_store::SESSION_PRACTICE {
                    continue;
                }
                rated += 1;
                let grid: Vec<GridEntry> = s
                    .results
                    .iter()
                    .map(|r| GridEntry {
                        name: &r.name,
                        car_name: &r.car_name,
                        is_player: r.is_player,
                    })
                    .collect();
                let fit = GridFit::measure(&seats, &grid);
                let note = fit.note();
                if note.is_some() {
                    flagged += 1;
                }
                body.sessions.push(GridCheckSession {
                    id: s.id.clone(),
                    track: s.track.clone(),
                    session_type: s.session_type,
                    fit,
                    note,
                });
            }
            body.summary = (flagged > 0).then(|| {
                format!(
                    "{flagged} of {rated} recorded session{} {} not raced on the full {}-car \
                     roster, so what {} say about your driving is measured against a grid that \
                     was not on track.",
                    if rated == 1 { "" } else { "s" },
                    if flagged == 1 { "was" } else { "were" },
                    body.seats,
                    if flagged == 1 { "it can" } else { "they can" },
                )
            });
        }
        Ok(body)
    }

    /// Which recorded sessions may join this championship — the same verdict
    /// [`Self::add_session_to_round`] gives, so the picker never offers a session that adding
    /// would refuse.
    pub fn session_eligibility(&self, id: &str) -> AppResult<SessionEligibility> {
        let data = self.data();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        let mut out = SessionEligibility::default();
        if let (Some(_), Some(_), Some(_)) = (
            cfg_custom_ai_dir(&self.config_path),
            champ.custom_ai_file.as_deref(),
            champ.player_team.as_deref().filter(|t| !t.trim().is_empty()),
        ) {
            let seats = champ_roster_seats(&self.config_path, &self.active_path(), champ);
            out.enforced = true;
            for s in &data.sessions {
                if let (Some(reason), _) = check_session_seat(
                    champ,
                    &data.sessions,
                    &seats,
                    &s.id,
                    data.mode.requires_exact_car(),
                ) {
                    out.blocked.insert(s.id.clone(), reason);
                }
            }
        }
        Ok(out)
    }
}

/// Whether session `sid` may be added to `champ`, and the car to store on the season if adding
/// it is what settles one.
///
/// Three rules, strictest first: a season with a stored car only takes sessions in that car; one
/// without only takes sessions in its declared team; and a session that would leave no single
/// car of the team consistent with the rest of the season is refused, because the season would
/// then have been raced in two. Nothing is checked without a declared team and a roster — the
/// same rule `check_player_team` has always had.
///
/// `exact_car` is the career's [`CareerMode::requires_exact_car`]: a stored car must then be the
/// *only* car the grid leaves empty, not merely one of them. Practice is exempt — a short practice
/// grid is a choice, and nothing is paid or rated on it.
pub fn check_session_seat(
    champ: &Championship,
    sessions: &[RecordedSession],
    seats: &[SeatEntry],
    sid: &str,
    exact_car: bool,
) -> (Option<String>, Option<SeatLock>) {
    use crate::roster::custom_ai::{SeatEvidence, SeatProof, TeamCheck};
    use crate::career::data_store::{settle_player_seat, SESSION_PRACTICE};

    let Some(team) = champ.player_team.as_deref().filter(|t| !t.trim().is_empty()) else {
        return (None, None);
    };
    let Some(session) = sessions.iter().find(|s| s.id == sid) else {
        return (None, None);
    };
    if seats.is_empty() {
        return (None, None);
    }
    let grid = session.grid();
    if let Some(lock) = &champ.player_seat {
        let proof = if exact_car && session.session_type != SESSION_PRACTICE {
            SeatProof::Exact
        } else {
            SeatProof::Possible
        };
        return match custom_ai::check_locked_seat(seats, &grid, &lock.seat, &lock.replaces, proof) {
            TeamCheck::Failed(reason) => (Some(reason), None),
            _ => (None, None),
        };
    }
    if let TeamCheck::Failed(reason) = custom_ai::check_player_team(seats, &grid, team) {
        return (Some(reason), None);
    }
    match settle_player_seat(champ, sessions, seats, Some(session)) {
        SeatEvidence::Settled(s) => (
            None,
            Some(SeatLock {
                replaces: custom_ai::replaced_driver(seats, &s.seat),
                seat: s.seat,
            }),
        ),
        // Refused only when this session is what breaks it. A season that already disagreed
        // with itself before this existed is left to the team check rather than blocked.
        SeatEvidence::Contradicted
            if settle_player_seat(champ, sessions, seats, None) != SeatEvidence::Contradicted =>
        {
            (
                Some(format!(
                    "the other sessions of this season were driven in a different {team} car \
                     than this one - a season is raced in one car"
                )),
                None,
            )
        }
        _ => (None, None),
    }
}

/// Re-derives a season's stored car after sessions have left it, so a car taken from a session
/// assigned by mistake does not outlive that session.
///
/// A season with nothing assigned is back where it started and holds no car. Without a readable
/// roster the car is kept rather than cleared: an unset folder is not evidence the car was wrong.
/// A car chosen when **signing** is never touched — it was not derived from sessions, so losing
/// them says nothing about it; tearing up the contract is what releases it.
fn reseat(
    champ: &mut Championship,
    sessions: &[RecordedSession],
    contracts: &[crate::career::contracts::Contract],
    config_path: &Path,
    career: &Path,
) {
    if crate::career::contracts::for_championship(contracts, &champ.id).is_some() {
        return;
    }
    if champ.rounds.iter().all(|r| r.session_ids.is_empty()) {
        champ.player_seat = None;
        return;
    }
    let seats = champ_roster_seats(config_path, career, champ);
    if !seats.is_empty() {
        champ.player_seat = crate::career::data_store::derive_seat_lock(champ, sessions, &seats);
    }
}
