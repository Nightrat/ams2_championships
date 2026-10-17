//! The application layer: every operation the app offers, as typed calls on [`App`].
//!
//! The domain modules (`data_store`, `driver_rating`, `contracts`, `custom_ai`, …) know the rules
//! of one subject each. This module is what strings them together into the things a user can
//! actually *do* — create a season, sign for a team, add a session to a round — and enforces the
//! rules that span several of them on the way: what a career's mode allows, what the first
//! session locks, which roster a season reads.
//!
//! **Nothing here knows about HTTP.** Every method answers with a typed value or an [`AppError`],
//! and the server maps those to JSON and status codes. That is the whole point of the split: a
//! second front end (a native GUI) calls the same methods and gets the same rules, rather than
//! re-implementing them against the JSON API and drifting from it.
//!
//! The view types are `Serialize` because they are also exactly what the HTTP routes answer with
//! — the wire format is defined here, once, and both front ends read the same shapes.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::RwLockReadGuard;

use crate::career::data_store::{persist, CareerData, Championship, SavePath, SharedStore};
use crate::game::spotter::Focus;

mod career_saves;
mod championships;
mod live;
mod live_frame;
mod performance;
mod settings;
mod signing;

pub use career_saves::SavesView;
pub use championships::{
    check_session_seat, ChampionshipPatch, GridCheck, GridCheckSession, NewChampionship, SessionListing,
    SessionEligibility, TeamEligibilityView,
};
pub use live::{live_seat_status, resolve_live_teams, GridStatus, LiveTeams, RosterMismatch};
pub use live_frame::{on_track, session_bests, LiveFrame, RideSeen, SessionBests};
pub use performance::{
    CarPerformance, CarPerformanceClass, CarRow, DriverAttrEdit, DriverPerformance,
    DriverPerformanceClass, PlayerRating, TeamScalarsEdit,
};
pub use settings::{
    class_year_rows, ClassYearRow, ConfigPatch, ConfigPatched, ConfigView, SpotterPatch,
    SpotterStatus,
};
pub use signing::{FinancesView, OffersView, SignRequest, Signed};

/// Why an operation was refused. The variant says what *kind* of refusal it is, so each front
/// end can present it its own way — the server as a status code, a GUI as a dialog — while the
/// sentence itself is written once, here.
#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    /// The request itself is malformed or incomplete.
    BadRequest(String),
    /// The thing named does not exist.
    NotFound(String),
    /// The request is well-formed but the career's state does not allow it.
    Conflict(String),
    /// A file exists but is in a format that cannot be handled.
    Unsupported(String),
    /// Something failed that the caller could not have prevented — usually a write.
    Internal(String),
}

impl AppError {
    pub fn message(&self) -> &str {
        match self {
            AppError::BadRequest(m)
            | AppError::NotFound(m)
            | AppError::Conflict(m)
            | AppError::Unsupported(m)
            | AppError::Internal(m) => m,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

fn bad(msg: impl Into<String>) -> AppError {
    AppError::BadRequest(msg.into())
}
fn not_found(msg: impl Into<String>) -> AppError {
    AppError::NotFound(msg.into())
}
fn conflict(msg: impl Into<String>) -> AppError {
    AppError::Conflict(msg.into())
}
fn internal(msg: impl Into<String>) -> AppError {
    AppError::Internal(msg.into())
}

/// Everything the app's operations need to reach: the career in memory, where it lives, and
/// the folders and settings around it.
///
/// Cheap to clone — the store, the active path and the spotter focus are shared handles, so every
/// clone sees (and moves) the same career. The recorder and spotter threads hold clones of the
/// same handles, which is why switching saves replaces the store's *contents* and never the `Arc`.
#[derive(Clone)]
pub struct App {
    pub store: SharedStore,
    /// The active save — swappable at runtime by [`App::activate_save`].
    pub data_path: SavePath,
    /// The saves folder, resolved once at startup.
    pub saves_dir: PathBuf,
    /// Track radar layouts, shared by every save.
    pub layouts_dir: PathBuf,
    pub config_path: PathBuf,
    pub spotter_focus: Focus,
    /// The ride-height record of the run being driven — see [`App::live_frame`].
    ride: std::sync::Arc<std::sync::Mutex<live_frame::RideRecord>>,
}

impl App {
    pub fn new(
        store: SharedStore,
        data_path: SavePath,
        saves_dir: PathBuf,
        layouts_dir: PathBuf,
        config_path: PathBuf,
        spotter_focus: Focus,
    ) -> Self {
        Self {
            store,
            data_path,
            saves_dir,
            layouts_dir,
            config_path,
            spotter_focus,
            ride: Default::default(),
        }
    }
    /// The currently active save file. Cloned out of the shared lock so the guard is never held
    /// across a file write. Empty when there is no active career.
    pub fn active_path(&self) -> PathBuf {
        self.data_path.read().map(|p| p.clone()).unwrap_or_default()
    }

    /// Read access to the career as it stands. Hold the guard briefly: the recorder thread needs
    /// the write lock to file a session.
    pub fn data(&self) -> RwLockReadGuard<'_, CareerData> {
        self.store.read().unwrap()
    }

    pub fn config(&self) -> crate::config::Config {
        crate::config::load_or_create(&self.config_path)
    }

    /// Persists the store to `path`, as an [`AppError::Internal`] if the write was refused.
    ///
    /// A change that reached memory but not the file is not a change the user may be told
    /// succeeded — that is precisely the silent failure the guard in [`persist`] exists to
    /// surface.
    fn persist_to(&self, path: &PathBuf) -> AppResult<()> {
        persist(&self.store, path).map_err(AppError::Internal)
    }

    /// [`Self::persist_to`] the active save.
    fn persist(&self) -> AppResult<()> {
        self.persist_to(&self.active_path())
    }

    /// The configured Custom AI Drivers folder, if one is set and non-empty.
    pub fn custom_ai_dir(&self) -> Option<PathBuf> {
        cfg_custom_ai_dir(&self.config_path)
    }

    /// The rating tuning this career runs on: its own stamped copy, falling back to config only
    /// for a save written before stamping existed and not yet loaded through
    /// [`Self::seal_career`].
    ///
    /// Every rating figure in the app goes through here rather than reading config directly, so
    /// that there is one answer to "which numbers is this career being judged on".
    pub fn career_rating_params(&self, data: &CareerData) -> crate::career::driver_rating::RatingParams {
        data.rating_params
            .unwrap_or_else(|| self.config().rating_params())
    }

    /// Brings a freshly loaded career up to date: keeps a copy of the roster of any season signed
    /// before rosters were fixed, stores the car of seasons already under way, stamps the rating
    /// tuning it runs on if it has none, stamps the payout onto any season finished before
    /// settlements existed, and moves inline lap charts out. Writes the career back if any of
    /// that changed anything.
    ///
    /// Called from exactly two places — startup, and [`Self::activate_save`] — because those are
    /// the only two that *load* a career, and when this runs is the whole of its correctness.
    /// Every upgrade takes its figures from the config the career has been running on, so they
    /// preserve what it was already doing; run any on the request path and it would capture
    /// whatever the settings had since been retuned to. See [`crate::career::contracts::seal_finished`].
    pub fn seal_career(&self, path: &PathBuf) -> Result<(), String> {
        seal_career(&self.store, path, &self.config_path)
    }
}

/// [`App::seal_career`] without an `App`, for callers holding the pieces.
pub fn seal_career(store: &SharedStore, path: &PathBuf, config_path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Ok(()); // No active career — nothing loaded, nothing to seal.
    }
    let cfg = crate::config::load_or_create(config_path);
    let kept = keep_signed_rosters(store, config_path, path);
    if kept > 0 {
        println!("Kept the roster of {kept} season(s) signed before rosters were fixed.");
    }
    let seated = seat_unseated(store, config_path, path);
    if seated > 0 {
        println!("Recorded the car driven in {seated} season(s) already under way.");
    }
    let (stamped_rating, sealed, charts) = {
        // The guard is released before `persist` takes its own read lock: holding a write lock
        // across the write would deadlock on the same RwLock.
        let mut data = store.write().unwrap();
        let d = &mut *data;
        // A career created before the tuning was stamped adopts what it has been judged on all
        // along, so no rating moves on the upgrade — and none moves afterwards either.
        let stamped_rating = d.rating_params.is_none();
        if stamped_rating {
            d.rating_params = Some(cfg.rating_params());
        }
        (
            stamped_rating,
            crate::career::contracts::seal_finished(
                &mut d.contracts,
                &d.championships,
                &d.sessions,
                None,
                &cfg.prize_params(),
            ),
            // A career written before lap charts were split out still carries them inline, and
            // they are most of its bulk. Moving them out is the same shape of one-time upgrade
            // as the two above, and belongs in the same place for the same reason.
            crate::career::lap_charts::externalize(path, &mut d.sessions),
        )
    };
    if sealed > 0 {
        println!("Sealed {sealed} finished season(s) at their current payout.");
    }
    if stamped_rating {
        println!("Recorded this career's driver rating settings from config.json.");
    }
    if charts > 0 {
        println!("Moved {charts} lap chart(s) out of the career file into laps/.");
    }
    if !stamped_rating && sealed == 0 && charts == 0 && seated == 0 {
        return Ok(());
    }
    persist(store, path)
}

/// Gives every signed season that has no roster of its own a copy of the game's file — seasons
/// signed before rosters were fixed at signing. Returns how many it copied.
///
/// The one-time upgrade beside the others in [`seal_career`], and correct for the same reason:
/// the game's file is what those seasons have been judged on until now, so copying it moves
/// nothing, and from here on nothing moves them either. A season whose file is missing, or a
/// flat save with nowhere to keep a copy, is left reading the game's file as before.
fn keep_signed_rosters(store: &SharedStore, config_path: &Path, career: &Path) -> usize {
    use crate::career::signed_rosters;
    let Some(dir) = cfg_custom_ai_dir(config_path) else {
        return 0;
    };
    if signed_rosters::rosters_dir(career).is_none() {
        return 0;
    }
    let data = store.read().unwrap();
    data.championships
        .iter()
        .filter(|c| crate::career::contracts::for_championship(&data.contracts, &c.id).is_some())
        .filter(|c| signed_rosters::stored(career, &c.id).is_none())
        .filter_map(|c| {
            let game = dir.join(c.custom_ai_file.as_deref()?);
            match signed_rosters::store(career, &c.id, &game) {
                Ok(()) => Some(()),
                Err(e) => {
                    eprintln!("Could not keep the roster of “{}”: {e}", c.name);
                    None
                }
            }
        })
        .count()
}

/// Stores the car on every season that has sessions and a declared team but no car yet — seasons
/// raced before `player_seat` existed, or whose sessions have only now become conclusive. Returns
/// how many it stored. A season whose sessions disagree is left alone: it is enforced on the team
/// as before, rather than being locked to one of the two cars it was raced in.
fn seat_unseated(store: &SharedStore, config_path: &Path, career: &Path) -> usize {
    let Some(dir) = cfg_custom_ai_dir(config_path) else {
        return 0;
    };
    let installed = crate::roster::liveries::installed_livery_names(&dir);
    let mut data = store.write().unwrap();
    let d = &mut *data;
    let mut seated = 0;
    for champ in d.championships.iter_mut() {
        if champ.player_seat.is_some() {
            continue;
        }
        let Some(file) = crate::career::signed_rosters::roster_file(career, Some(&dir), champ) else {
            continue;
        };
        let seats = seats_at(&file, installed.as_ref());
        if let Some(lock) = crate::career::data_store::derive_seat_lock(champ, &d.sessions, &seats) {
            champ.player_seat = Some(lock);
            seated += 1;
        }
    }
    seated
}

// ── Shared helpers ───────────────────────────────────────────────────────────────────────────

/// Seconds since the Unix epoch — when a contract was signed, and when a season was closed.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// The configured Custom AI Drivers folder, if one is set and non-empty.
fn cfg_custom_ai_dir(config_path: &Path) -> Option<PathBuf> {
    crate::config::load_or_create(config_path)
        .custom_ai_dir
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from)
}

/// The Custom AI Drivers folder and the class season-year overrides, from **one** config read.
///
/// The two always travel together: every caller that lists classes also orders and labels them
/// by season, and reading the file twice for the two halves of one answer is how they could
/// disagree across a concurrent `PATCH /api/config`.
fn cfg_custom_ai(config_path: &Path) -> Option<(PathBuf, BTreeMap<String, u16>)> {
    let cfg = crate::config::load_or_create(config_path);
    let dir = cfg
        .custom_ai_dir
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)?;
    Some((dir, cfg.class_years()))
}

/// Everything needed to rate and rank one car class.
struct ClassData {
    perf: crate::roster::custom_ai::ClassPerformance,
    ctx: crate::career::driver_rating::RatingContext,
    /// Weaker incumbent's `race_skill` per team.
    skills: HashMap<String, f32>,
    /// Whether the class has a baseline recorded to reset back to.
    has_baseline: bool,
    /// Each team's preview picture, as a path under the `Overrides` folder — see
    /// [`crate::roster::liveries::InstalledLiveries::previews_for`]. A team whose liveries carry none is
    /// absent, which is the common case for a class with no livery mod.
    previews: HashMap<String, String>,
}

/// Loads every readable car class from the configured Custom AI Drivers folder.
///
/// Ratings are measured against where a car *should* finish, which is a function of how many
/// cars are ahead of it — so the seat list has to be the grid AMS2 will actually field. Entries
/// whose livery the game does not own never start, and counting them stretches every expected
/// position down the order, making the whole field look easier to beat than it was.
fn load_classes(config_path: &Path) -> Vec<ClassData> {
    use crate::{career::driver_rating, roster::custom_ai};
    let Some((dir, years)) = cfg_custom_ai(config_path) else {
        return vec![];
    };
    // One scan for every class: the manifests are per car model, not per class.
    let installed = crate::roster::liveries::installed_liveries(&dir);
    let names = installed.as_ref().map(|l| l.names());
    custom_ai::class_performance_with(&dir, &years)
        .into_iter()
        .map(|perf| {
            let file = format!("{}.xml", perf.class);
            let path = dir.join(&file);
            let pace: HashMap<String, f32> = perf
                .cars
                .iter()
                .map(|c| (c.team.clone(), c.pace_delta_pct))
                .collect();
            let ctx = driver_rating::RatingContext::new(
                &perf.class,
                roster_seats_with(&dir, &file, names.as_ref()),
                &pace,
            );
            ClassData {
                skills: custom_ai::parse_team_skills(&path),
                has_baseline: custom_ai::has_baseline(&path),
                previews: team_previews(installed.as_ref(), &path),
                ctx,
                perf,
            }
        })
        .collect()
}

/// The preview picture to show for each team in one class: the first of its liveries that has
/// one.
///
/// The whole class is resolved in one call, because a livery name alone does not say which car
/// model declared it — only the rest of the roster does. See
/// [`crate::roster::liveries::InstalledLiveries::previews_for`].
fn team_previews(
    installed: Option<&crate::roster::liveries::InstalledLiveries>,
    class_file: &Path,
) -> HashMap<String, String> {
    let Some(installed) = installed else {
        return Default::default();
    };
    let teams = crate::roster::custom_ai::parse_team_liveries(class_file);
    let roster: Vec<String> = teams.values().flatten().cloned().collect();
    let previews = installed.previews_for(&roster);
    teams
        .into_iter()
        .filter_map(|(team, liveries)| {
            let found = liveries.iter().find_map(|l| previews.get(l))?;
            Some((team, found.clone()))
        })
        .collect()
}

/// Resolves `<class>.xml` inside the configured Custom AI Drivers folder.
///
/// A class name reaches us as a file name, so it goes through the same gate as a save name.
fn class_file(config_path: &Path, class: &str) -> AppResult<PathBuf> {
    let dir = cfg_custom_ai_dir(config_path)
        .ok_or_else(|| bad("no Custom AI Drivers folder is configured"))?;
    let name = crate::career::saves::sanitize_name(class).ok_or_else(|| bad("invalid class name"))?;
    let file = dir.join(format!("{name}.xml"));
    if !file.is_file() {
        return Err(not_found("no such car class file"));
    }
    Ok(file)
}

/// Seats a roster actually offers: the Custom AI file's entries minus any whose livery AMS2 does
/// not own.
///
/// Everything that reasons about the grid goes through here — both enforcement paths and the
/// rating contexts. A phantom seat can never be occupied by an AI, so leaving it in makes it look
/// free in every session forever (corrupting the elimination `infer_player_seat` relies on) and
/// inflates the field size every expected finishing position is derived from.
///
/// Takes the installed set rather than reading it, so a caller looping over every class scans the
/// manifests once instead of once per class.
fn roster_seats_with(
    dir: &Path,
    file: &str,
    installed: Option<&std::collections::HashSet<String>>,
) -> Vec<crate::roster::custom_ai::SeatEntry> {
    seats_at(&dir.join(file), installed)
}

/// [`roster_seats_with`] for a roster file named by its full path — the game's, or a season's own
/// copy. The liveries are always the install's: they are what AMS2 can spawn, whichever file
/// names them.
fn seats_at(
    file: &Path,
    installed: Option<&std::collections::HashSet<String>>,
) -> Vec<crate::roster::custom_ai::SeatEntry> {
    let seats = crate::roster::custom_ai::parse_seats(file);
    crate::roster::custom_ai::without_phantom_seats(seats, installed)
}

/// The file a championship's roster is read from: the copy the season keeps once a contract is
/// signed, otherwise the game's file. **Every** read of one season's roster goes through here —
/// see [`crate::career::signed_rosters`].
fn champ_roster_file(config_path: &Path, career: &Path, champ: &Championship) -> Option<PathBuf> {
    crate::career::signed_rosters::roster_file(career, cfg_custom_ai_dir(config_path).as_deref(), champ)
}

/// The seats a championship's own roster offers; empty without a roster to read.
fn champ_roster_seats(
    config_path: &Path,
    career: &Path,
    champ: &Championship,
) -> Vec<crate::roster::custom_ai::SeatEntry> {
    let Some(file) = champ_roster_file(config_path, career, champ) else {
        return Vec::new();
    };
    let installed =
        cfg_custom_ai_dir(config_path).and_then(|d| crate::roster::liveries::installed_livery_names(&d));
    seats_at(&file, installed.as_ref())
}

/// Every signed season's own roster as a rating context, keyed by championship id.
///
/// Each covers only that season's sessions, and callers put them **ahead of** the class contexts
/// from [`load_classes`]: a session is scored against the first context that covers it, so a
/// signed season's races are judged on the roster it was signed on, whatever the game's file
/// has become since, while every other session in the class reads the game's file.
fn season_contexts(
    config_path: &Path,
    career: &Path,
    data: &CareerData,
) -> Vec<(String, crate::career::driver_rating::RatingContext)> {
    use crate::{
        career::{driver_rating, signed_rosters},
        roster::custom_ai,
    };
    let installed =
        cfg_custom_ai_dir(config_path).and_then(|d| crate::roster::liveries::installed_livery_names(&d));
    data.championships
        .iter()
        .filter_map(|champ| {
            let copy = signed_rosters::stored(career, &champ.id)?;
            let class = champ_class(champ);
            let perf = custom_ai::class_performance_of(&class, &copy, &Default::default());
            let pace: HashMap<String, f32> = perf
                .cars
                .iter()
                .map(|c| (c.team.clone(), c.pace_delta_pct))
                .collect();
            let sessions = champ
                .rounds
                .iter()
                .flat_map(|r| r.session_ids.iter().cloned())
                .collect();
            let ctx = driver_rating::RatingContext::for_season(
                &class,
                seats_at(&copy, installed.as_ref()),
                &pace,
                sessions,
            );
            Some((champ.id.clone(), ctx))
        })
        .collect()
}

/// The contexts a career-wide rating is computed over: every signed season's own roster first,
/// then one per class from the game's files. See [`season_contexts`].
fn rating_contexts(
    seasons: Vec<(String, crate::career::driver_rating::RatingContext)>,
    classes: Vec<ClassData>,
) -> Vec<crate::career::driver_rating::RatingContext> {
    seasons
        .into_iter()
        .map(|(_, ctx)| ctx)
        .chain(classes.into_iter().map(|c| c.ctx))
        .collect()
}

impl App {
    /// Driver rating and per-team eligibility for a championship.
    ///
    /// `None` when the championship has no Custom AI file, or no folder is configured — without
    /// a roster there are no teams, no car pace figures, and nothing to rate against.
    ///
    /// Takes the whole career rather than its sessions and championships, because the tuning to
    /// judge them on is part of the career too — see [`Self::career_rating_params`]. Passing the
    /// pieces left a caller free to forget the third one.
    fn champ_eligibility(
        &self,
        career: &Path,
        champ: &Championship,
        data: &CareerData,
    ) -> Option<(
        crate::career::driver_rating::Reputation,
        Vec<crate::career::driver_rating::TeamEligibility>,
    )> {
        use crate::career::driver_rating;
        let config_path = &self.config_path;

        let file = champ.custom_ai_file.as_deref()?;
        let class = Path::new(file).file_stem()?.to_str()?;
        let classes = load_classes(config_path);
        let seasons = season_contexts(config_path, career, data);
        // A signed season's bars come from the roster it was signed on, like its results do;
        // anything else reads the game's file.
        let (expected, skills) = match seasons.iter().find(|(id, _)| id == &champ.id) {
            Some((_, ctx)) => (
                ctx.expected.clone(),
                crate::roster::custom_ai::parse_team_skills(&champ_roster_file(
                    config_path,
                    career,
                    champ,
                )?),
            ),
            None => {
                let own = classes.iter().find(|c| c.perf.class == class)?;
                (own.ctx.expected.clone(), own.skills.clone())
            }
        };

        // The rating spans the driver's whole career — a seat is earned by racing, not by racing
        // this particular car — while the requirement comes from this class's own grid. Only
        // sessions committed to a championship count toward it.
        let contexts = rating_contexts(seasons, classes);
        let rated = driver_rating::assigned_sessions(&data.championships, &data.sessions);
        let params = self.career_rating_params(data);
        let reputation = driver_rating::compute_reputation_global_with(
            None,
            &rated,
            &contexts,
            champ.player_team.as_deref(),
            &params,
        );
        let eligibility =
            driver_rating::team_eligibility_with(&params, reputation.value, &expected, &skills);
        Some((reputation, eligibility))
    }
}

/// The car class a championship runs in — its Custom AI file's stem, empty when it has none.
///
/// A team name only identifies a team *within* a class, so this is what scopes an incumbency:
/// "Ferrari" appears in seven of the eight shipped rosters, spanning thirty years.
fn champ_class(champ: &Championship) -> String {
    champ
        .custom_ai_file
        .as_deref()
        .map(crate::roster::custom_ai::class_of_file)
        .unwrap_or_default()
        .to_string()
}

/// Each team's picture in one championship's roster, keyed by team name — the same map
/// [`CarPerformance`] carries per class, for a caller that has a list of teams and wants to show
/// the cars.
///
/// Empty whenever anything is missing: no Custom AI folder, no roster on the championship, no
/// livery mod. A missing picture is never worth a message of its own.
fn champ_previews(config_path: &Path, career: &Path, champ: &Championship) -> HashMap<String, String> {
    let Some(dir) = cfg_custom_ai_dir(config_path) else {
        return Default::default();
    };
    let Some(file) = champ_roster_file(config_path, career, champ) else {
        return Default::default();
    };
    let installed = crate::roster::liveries::installed_liveries(&dir);
    team_previews(installed.as_ref(), &file)
}

/// The career ledger and the negotiating position it leaves the driver in.
///
/// Both operations that deal in offers need both — the money to say what a seat can be bought
/// with, the standing to say who is re-signing whom — and deriving them together keeps the two
/// from ever disagreeing about which season was the last one completed.
fn career_standing(
    cfg: &crate::config::Config,
    data: &CareerData,
) -> (crate::career::contracts::Finances, crate::career::contracts::Standing) {
    let ledger = crate::career::contracts::finances(
        &data.contracts,
        &data.championships,
        &data.sessions,
        None,
        // The save's own figure, not the config's — see `CareerData::starting_balance`.
        data.starting_balance.max(0),
        &cfg.prize_params(),
    );
    let standing = crate::career::contracts::standing(&ledger);
    (ledger, standing)
}

/// Distinguishes an absent key from an explicit `null`.
///
/// Plain `Option<Option<T>>` cannot: serde collapses a `null` into the *outer* `None`, which
/// reads as "leave unchanged" and makes an assignment impossible to clear.
fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(de).map(Some)
}

#[cfg(test)]
#[path = "tests/service.rs"]
mod tests;
