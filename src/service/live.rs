//! The live tab's verdicts on the session loaded in AMS2: team names for the grid, and whether
//! the grid, the car and the roster are the ones the active season is judged on.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use super::{cfg_custom_ai_dir, seats_at, App};
use crate::roster::custom_ai::{self, GridEntry, GridFit, SeatEntry, SeatProof, TeamCheck};
use crate::career::data_store::{Championship, ChampionshipStatus};

/// Team names and banners for the live timing grid.
#[derive(Serialize, Default, Debug)]
pub struct LiveTeams {
    /// Driver name -> livery/team name, from the active championship's Custom AI file.
    pub teams: HashMap<String, String>,
    /// Manual override for the player's row — their profile name won't be in the file.
    pub player_team: Option<String>,
    /// What to tell the driver about the grid on track — **including when it is right**.
    ///
    /// One field rather than a warning beside a confirmation, so the two can never both be on
    /// screen and the client is not left deciding which wins. `None` means there is nothing to
    /// check yet (no session loaded, or a career with no roster), which is why it is not the
    /// same as good news and must not be rendered as such.
    ///
    /// The live tab is the only place a grid problem can still be *fixed* — quit to the menu,
    /// set the opponent count, start again — so it is the only one that says anything when
    /// there is no problem. The sentences are server-side, like every other one about the
    /// roster: the Manage and Career tabs show the same text, and copies would drift.
    pub grid: Option<GridStatus>,
    /// How the live grid measures up, when a roster could be found. Carried alongside the
    /// sentence so the tab can show the counts without re-deriving them.
    pub fit: Option<GridFit>,
    /// Whether the player is sitting in the season's car — its stored car once there is one,
    /// otherwise any car of its team. A separate banner from `grid`, because the two are
    /// independent problems and a wrong car is the one that gets the session refused: it is
    /// said here, while the driver can still quit to the menu and pick the right livery, rather
    /// than when the session is added and it is too late to re-run it.
    pub seat: Option<GridStatus>,
    /// Set when the active season keeps its own roster (see [`crate::career::signed_rosters`]) and the
    /// file AMS2 will load is not that roster. Everything about the season is judged on its
    /// copy, so a session raced on anything else is measured against a grid the game did not
    /// field. The tab offers to copy the season's roster back, which is the only fix — and it
    /// only takes effect once the session restarts.
    pub roster: Option<RosterMismatch>,
}

/// The live tab's "the game is not loading this season's roster" banner.
#[derive(Serialize, Default, Debug)]
pub struct RosterMismatch {
    /// The season to restore — see [`App::restore_roster`].
    pub champ_id: String,
    pub text: String,
}

/// A sentence about the live grid, and whether it is good news.
#[derive(Serialize, Default, Debug)]
pub struct GridStatus {
    /// True when the grid is the roster. The banner is green rather than amber, and nothing
    /// needs doing before the session starts.
    pub ok: bool,
    pub text: String,
}

impl GridStatus {
    fn warn(text: impl Into<String>) -> Option<Self> {
        Some(Self {
            ok: false,
            text: text.into(),
        })
    }
}

impl App {
    /// Team names and banners for the session AMS2 has loaded right now, read from the active
    /// championship. See [`resolve_live_teams`]. Without a Custom AI folder there is no roster at
    /// all, and the grid falls back to the car names AMS2 reports.
    pub fn live_teams(&self) -> LiveTeams {
        let data = self.data();
        let sp = data.mode.uses_roster();
        match cfg_custom_ai_dir(&self.config_path) {
            Some(dir) => {
                let live = crate::game::shared_memory::read_live_session();
                let grid: Vec<GridEntry> = live
                    .participants
                    .iter()
                    .map(|p| GridEntry {
                        name: &p.name,
                        car_name: &p.car_name,
                        is_player: p.is_player,
                    })
                    .collect();
                // The proof the session will be held to when it is added, so the banner warns
                // about exactly what the assignment will refuse. Practice is held to none.
                let proof = if data.mode.requires_exact_car()
                    && live.session_state != crate::career::data_store::SESSION_PRACTICE
                {
                    SeatProof::Exact
                } else {
                    SeatProof::Possible
                };
                resolve_live_teams(&dir, &self.active_path(), &data.championships, &grid, sp, proof)
            }
            // Nothing can be resolved without the folder, and in a singleplayer career that is
            // the first thing to fix rather than something to discover from an empty column.
            None => LiveTeams {
                grid: sp
                    .then(|| {
                        GridStatus::warn(
                            "No Custom AI Drivers folder is set, so the grid cannot be resolved \
                             to teams and nothing can be rated. Set it in the Config tab.",
                        )
                    })
                    .flatten(),
                ..Default::default()
            },
        }
    }
}

/// Team names for the live timing grid, taken from the **active** championship.
///
/// AMS2's shared memory carries no team field, so the names come from a championship's Custom AI
/// Driver file. Which championship that is is not a guess: exactly one may be `Active` at a time —
/// patching a championship `Active` demotes any other to `Progress` — and that is the one being
/// raced, which is why the Manage tab opens on it too.
///
/// Scoring rosters against whoever happens to be on track cannot beat that, and was the earlier
/// bug here: two historic seasons of one series share most of their drivers, so a variant roster
/// carrying a few extra optional entries out-scores the season actually being driven and takes its
/// player team away with it — leaving the player's row showing the AMS2 car model.
///
/// Empty when no championship is active or the active one has no Custom AI file assigned; the grid
/// then falls back to the car names AMS2 reports.
pub fn resolve_live_teams(
    dir: &Path,
    career: &Path,
    champs: &[Championship],
    grid: &[GridEntry],
    sp: bool,
    proof: SeatProof,
) -> LiveTeams {
    let Some(champ) = champs
        .iter()
        .find(|c| c.status == ChampionshipStatus::Active)
    else {
        return LiveTeams {
            // Only a singleplayer career is judged on a roster, so only it is nagged about one.
            grid: sp
                .then(|| {
                    GridStatus::warn(
                        "No championship is Active, so this session has no season to belong \
                         to — its result will not be judged against any grid. Mark one Active \
                         in the Manage tab.",
                    )
                })
                .flatten(),
            ..Default::default()
        };
    };
    let Some(file) = champ.custom_ai_file.as_deref() else {
        return LiveTeams {
            grid: sp
                .then(|| {
                    GridStatus::warn(format!(
                        "“{}” has no Custom AI Drivers file, so team names, your rating and \
                         any contract have nothing to be measured against.",
                        champ.name
                    ))
                })
                .flatten(),
            ..Default::default()
        };
    };
    // The season's own copy once it is signed — the grid it is judged on, which is not
    // necessarily the one AMS2 is about to load. `roster` says so when they differ.
    let roster_path = crate::career::signed_rosters::roster_file(career, Some(dir), champ)
        .unwrap_or_else(|| dir.join(file));
    let installed = crate::roster::liveries::installed_livery_names(dir);
    let seats = seats_at(&roster_path, installed.as_ref());
    let roster = (crate::career::signed_rosters::matches_game(career, dir, champ) == Some(false)).then(|| {
        RosterMismatch {
            champ_id: champ.id.clone(),
            text: format!(
                "The {file} AMS2 loads is not the roster “{}” was signed on. This season is \
                 judged on its own copy, so a session raced on the game's file is measured \
                 against a grid the game did not field. Copy the season's roster to the game's \
                 Custom AI Drivers folder — this replaces the file there — then restart the \
                 session in AMS2: it reads the roster when a session loads.",
                champ.name
            ),
        }
    });
    // An empty grid is the menu, not a problem with the grid.
    let fit = (!grid.is_empty() && !seats.is_empty()).then(|| GridFit::measure(&seats, grid));
    // A roster entry that cannot spawn is said ahead of the plain grid note: it is the cause the
    // note would otherwise misreport — "above the 27 it can field" when the roster fields 26.
    let fault = (sp && fit.is_some())
        .then(|| {
            custom_ai::roster_fault(
                &seats,
                grid,
                champ.player_seat.as_ref().map(|l| l.seat.as_str()),
                file,
            )
        })
        .flatten();
    let mut seat = if sp && !grid.is_empty() {
        live_seat_status(champ, &seats, grid, proof)
    } else {
        None
    };
    // A dead entry leaves its car empty on every grid, so the car check sees one empty car too
    // many and may blame the driver for the roster's fault.
    if let (Some(s), true) = (seat.as_mut(), fault.is_some()) {
        if !s.ok {
            s.text.push_str(
                " If you did pick that car, the roster is the cause — see the grid banner.",
            );
        }
    }
    LiveTeams {
        teams: custom_ai::parse_driver_teams(&roster_path),
        player_team: champ.player_team.clone().filter(|t| !t.trim().is_empty()),
        // A multiplayer career is judged on no roster, so it is told nothing either way.
        grid: if sp {
            fault.map(|text| GridStatus { ok: false, text }).or_else(|| {
                fit.and_then(|f| {
                    f.note()
                        .map(|text| GridStatus { ok: false, text })
                        .or_else(|| f.confirmation().map(|text| GridStatus { ok: true, text }))
                })
            })
        } else {
            None
        },
        fit,
        seat,
        roster,
    }
}

/// The live seat banner — see [`LiveTeams::seat`]. The reason is the same sentence the
/// assignment refuses with, so what the banner warns about is exactly what will happen.
pub fn live_seat_status(
    champ: &Championship,
    seats: &[SeatEntry],
    grid: &[GridEntry],
    proof: SeatProof,
) -> Option<GridStatus> {
    let team = champ.player_team.as_deref().filter(|t| !t.trim().is_empty())?;
    if seats.is_empty() {
        return None;
    }
    let (check, want) = match &champ.player_seat {
        Some(lock) => (
            custom_ai::check_locked_seat(seats, grid, &lock.seat, &lock.replaces, proof),
            if lock.replaces.is_empty() {
                lock.seat.clone()
            } else {
                format!("{} in place of {}", lock.seat, lock.replaces)
            },
        ),
        None => (
            custom_ai::check_player_team(seats, grid, team),
            format!("a {team} car"),
        ),
    };
    match check {
        // Not this roster: the grid banner already says so, and nothing can be told about seats.
        TeamCheck::Skipped(_) => None,
        TeamCheck::Failed(reason) => GridStatus::warn(format!(
            "WRONG CAR — {reason}. This session cannot be added to “{}”. Quit to the menu and \
             pick {want}.",
            champ.name
        )),
        TeamCheck::Passed(_) => Some(GridStatus {
            ok: true,
            text: match &champ.player_seat {
                Some(_) => format!("Right car: {want}, the car “{}” is raced in.", champ.name),
                None => format!(
                    "Right team: {team}. Once an added session shows which {team} car you \
                     drive, “{}” is locked to that car.",
                    champ.name
                ),
            },
        }),
    }
}
