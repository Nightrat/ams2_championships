use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// One driver's position on a specific completed lap.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LapChartEntry {
    pub lap: u32,
    pub driver: String,
    pub position: u32,
}

/// Final result for one participant in a recorded session.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionResult {
    pub name: String,
    #[serde(default)]
    pub car_name: String,
    #[serde(default)]
    pub car_class: String,
    pub race_position: u32,
    pub laps_completed: u32,
    pub fastest_lap: f32,
    pub last_lap: f32,
    pub dnf: bool,
    /// True when this result belongs to the human player (mViewedParticipantIndex at capture time).
    #[serde(default)]
    pub is_player: bool,
}

/// A race session captured from the AMS2 shared memory.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordedSession {
    /// Unique ID — Unix timestamp in seconds as a string.
    pub id: String,
    /// Unix timestamp (seconds since epoch) when the session was recorded.
    pub recorded_at: u64,
    pub track: String,
    #[serde(default)]
    pub track_variation: String,
    #[serde(default)]
    pub car_name: String,
    #[serde(default)]
    pub car_class: String,
    /// session_state from AMS2: 1=Practice, 3=Qualify, 5=Race.
    pub session_type: u32,
    pub results: Vec<SessionResult>,
    #[serde(default)]
    pub lap_chart: Vec<LapChartEntry>,
}

/// A championship round — groups one or more sessions (Practice / Qualify / Race).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Round {
    /// Session IDs that belong to this round, in any order.
    pub session_ids: Vec<String>,
}

/// Lifecycle state of a championship.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "PascalCase")]
pub enum ChampionshipStatus {
    /// The one being raced right now. **At most one championship is `Active`**: setting it via
    /// `PATCH /api/championships/{id}` demotes whichever other one held it to [`Self::Progress`].
    /// That makes it the answer to "which championship is this session part of", which is what
    /// the Manage tab opens on and what the live timing grid reads its team names from.
    #[default]
    Active,
    /// Started, but not the one currently being raced.
    Progress,
    /// All rounds completed — winner determined.
    Final,
}

/// User-created championship grouping a set of rounds.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Championship {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub status: ChampionshipStatus,
    /// Points awarded for positions 1, 2, 3, … (may be shorter than field size).
    pub points_system: Vec<i32>,
    /// Whether to compute and display constructor (manufacturer) standings.
    #[serde(default)]
    pub manufacturer_scoring: bool,
    /// Ordered list of rounds.  Each round groups a Practice / Qualify / Race set.
    #[serde(default)]
    pub rounds: Vec<Round>,
    /// Legacy flat session list — migrated to rounds on load, never written back.
    #[serde(default, skip_serializing)]
    pub session_ids: Vec<String>,
    /// Filename (inside the configured Custom AI Drivers folder) of an AMS2 Custom AI Driver
    /// XML file. When set, driver display names are looked up in this file to show the
    /// team/livery name instead of the generic AMS2 car name/class.
    #[serde(default)]
    pub custom_ai_file: Option<String>,
    /// Manual team/car name override for the human player's result rows. AMS2's shared memory
    /// doesn't expose a livery/team field, and the player's profile name usually won't match
    /// a Custom AI Driver file entry, so this fills the same role for the player as
    /// `custom_ai_file` does for AI drivers.
    #[serde(default)]
    pub player_team: Option<String>,
}

/// Which kind of career a save holds, and therefore which rules its seasons follow.
///
/// Chosen when the career is created and **never changed afterwards** — the two modes allow
/// different things, so switching would leave a career holding seasons it could not have made.
/// The one exception is [`Self::Unset`], which every save written before careers had a mode
/// deserializes to: it may be set once, and that is the only transition there is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CareerMode {
    /// A save from before this existed. Behaves exactly as the app did then: rosters and teams
    /// may be set by hand, seasons run in parallel, and contracts are dormant.
    #[default]
    Unset,
    /// Racing the AI. A season names a Custom AI roster, the seat is taken by signing a
    /// contract, and only one season runs at a time.
    Singleplayer,
    /// Racing people. No roster, no team, no contracts, and as many seasons at once as the
    /// user likes.
    Multiplayer,
}

impl CareerMode {
    /// Whether a seat is taken by signing a contract rather than picking a team.
    ///
    /// The single source of truth for whether the whole contracts feature is live. It replaced a
    /// `contracts_enabled` config switch: the mode already answers the question, and two ways to
    /// say it could disagree.
    pub fn uses_contracts(self) -> bool {
        self == CareerMode::Singleplayer
    }

    /// Whether a season may name a Custom AI roster and a player team at all.
    pub fn uses_roster(self) -> bool {
        self != CareerMode::Multiplayer
    }

    /// Whether a new season may only be created once every existing one is finished.
    pub fn one_season_at_a_time(self) -> bool {
        self == CareerMode::Singleplayer
    }

    /// Whether [`ChampionshipStatus::Final`] is the end of the road for a season.
    ///
    /// Only in singleplayer, where a finished season has paid out and the next one was created
    /// on the strength of it. Elsewhere a status is just a label.
    pub fn final_is_terminal(self) -> bool {
        self == CareerMode::Singleplayer
    }

    /// Human-readable, for refusal messages.
    pub fn label(self) -> &'static str {
        match self {
            CareerMode::Unset => "unset",
            CareerMode::Singleplayer => "singleplayer",
            CareerMode::Multiplayer => "multiplayer",
        }
    }
}

/// Root data structure persisted to ams2_career.json.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CareerData {
    pub sessions: Vec<RecordedSession>,
    pub championships: Vec<Championship>,
    /// Which kind of career this save holds. See [`CareerMode`].
    #[serde(default)]
    pub mode: CareerMode,
    /// Credits the career began with, before it had raced anything.
    ///
    /// Set from `config.starting_balance` when the career is created and then **kept**, rather
    /// than read from config on every request. A career that started with a million still
    /// started with a million after the setting is changed — the same reason a contract stores
    /// the terms that were agreed instead of re-deriving them.
    ///
    /// Zero for a save written before this existed, which is what it had.
    #[serde(default)]
    pub starting_balance: i64,
    /// Terms the player agreed for a championship, keyed to it by `champ_id`. Absent from every
    /// save written before contracts existed, and from every save where the player never signed
    /// anything, which is why it defaults rather than being required.
    ///
    /// This is the *only* persisted part of the contracts feature — see [`crate::contracts`].
    #[serde(default)]
    pub contracts: Vec<crate::contracts::Contract>,
}

pub type SharedStore = Arc<RwLock<CareerData>>;

/// Path of the *active* career save file. Shared and mutable so that switching saves at
/// runtime repoints both the HTTP handler and the recorder thread, which each hold a clone.
pub type SavePath = Arc<RwLock<PathBuf>>;

/// Read one career save file from disk. A missing or unparsable file yields an empty career.
/// Byte-order mark Windows editors put at the front of a UTF-8 file.
///
/// Notepad and PowerShell both write one by default, and a save file is plain JSON sitting in a
/// folder users are invited to open. `serde_json` rejects it outright, so it is stripped rather
/// than allowed to read as a corrupt career.
const BOM: &str = "\u{feff}";

/// Reads one career save, distinguishing "not there yet" from "there but unreadable".
///
/// A missing file is an empty career — that is how a new save begins. A file that exists but
/// cannot be read or parsed is an error and must stay one: defaulting there would present an
/// empty career as if it were real, and the next [`persist`] would write that emptiness over
/// whatever was actually in the file.
pub fn try_load_data(path: &Path) -> Result<CareerData, String> {
    if !path.exists() {
        return Ok(CareerData::default());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut data: CareerData = serde_json::from_str(content.strip_prefix(BOM).unwrap_or(&content))
        .map_err(|e| e.to_string())?;
    // Migrate legacy flat session_ids → one round per session.
    for champ in &mut data.championships {
        if champ.rounds.is_empty() && !champ.session_ids.is_empty() {
            champ.rounds = champ
                .session_ids
                .drain(..)
                .map(|sid| Round {
                    session_ids: vec![sid],
                })
                .collect();
        }
    }
    Ok(data)
}

/// [`try_load_data`], reporting a damaged save to the console and yielding an empty career.
///
/// Only for callers that have to produce *something* — the startup path, which must still serve
/// the UI so the problem can be seen and fixed. It is safe to return an empty career here only
/// because [`persist`] independently refuses to overwrite a file it cannot read.
pub fn load_data(path: &Path) -> CareerData {
    match try_load_data(path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!(
                "ERROR: career save {} could not be read: {e}\n\
                 It will NOT be written to, so nothing in it is lost. Fix or move the file, \
                 then restart. (A leading byte-order mark is handled automatically; this is \
                 something else.)",
                path.display()
            );
            CareerData::default()
        }
    }
}

pub fn load_store(path: &Path) -> SharedStore {
    Arc::new(RwLock::new(load_data(path)))
}

// ── Career computation ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StandingsEntry {
    pub name: String,
    pub points: i32,
    pub wins: u32,
}

/// Session result enriched with points earned (for display in career view).
#[derive(Serialize)]
pub struct SessionResultView {
    pub name: String,
    pub car_name: String,
    pub car_class: String,
    pub race_position: u32,
    pub laps_completed: u32,
    pub fastest_lap: f32,
    pub last_lap: f32,
    pub dnf: bool,
    pub points_earned: i32,
}

/// Recorded session with results enriched for the career view.
#[derive(Serialize)]
pub struct SessionView {
    pub id: String,
    pub recorded_at: u64,
    pub track: String,
    pub track_variation: String,
    pub session_type: u32,
    pub results: Vec<SessionResultView>,
    pub lap_chart: Vec<LapChartEntry>,
}

/// Round with sessions already resolved from IDs.
#[derive(Serialize)]
pub struct RoundView {
    pub sessions: Vec<SessionView>,
}

#[derive(Serialize)]
pub struct ChampionshipView {
    pub id: String,
    pub name: String,
    pub status: ChampionshipStatus,
    pub points_system: Vec<i32>,
    pub manufacturer_scoring: bool,
    pub driver_standings: Vec<StandingsEntry>,
    pub constructor_standings: Vec<StandingsEntry>,
    pub rounds: Vec<RoundView>,
}

#[derive(Serialize)]
pub struct DriverStat {
    pub name: String,
    pub races: u32,
    pub p1: u32,
    pub p2: u32,
    pub p3: u32,
    pub top10: u32,
    pub dnf: u32,
    pub quali_p1: u32,
    pub quali_p2: u32,
    pub quali_p3: u32,
    pub quali_top10: u32,
    pub champ_wins: u32,
    pub champ_p2: u32,
    pub champ_p3: u32,
    pub avg_pos: f32,
}

#[derive(Serialize)]
pub struct TrackStat {
    pub track: String,
    pub track_variation: String,
    /// Player's car for this group; empty string in the aggregated "all cars" view.
    pub car: String,
    pub races: u32,
    pub qualifyings: u32,
    pub best_lap: f32,
    pub best_lap_driver: String,
    pub best_lap_car: String,
    pub second_lap: f32,
    pub second_lap_driver: String,
    pub second_lap_car: String,
    pub third_lap: f32,
    pub third_lap_driver: String,
    pub third_lap_car: String,
    pub last_visited: u64,
}

#[derive(Serialize)]
pub struct CareerResponse {
    pub championships: Vec<ChampionshipView>,
    pub driver_stats: Vec<DriverStat>,
    pub track_stats: Vec<TrackStat>,
}

fn resolve_sessions<'a>(
    ids: &[String],
    sessions: &'a [RecordedSession],
) -> Vec<&'a RecordedSession> {
    ids.iter()
        .filter_map(|id| sessions.iter().find(|s| s.id == *id))
        .collect()
}

/// Championship standings, best first. Races only — points come from `champ.points_system`,
/// and a retirement scores nothing.
pub fn standings(champ: &Championship, sessions: &[RecordedSession]) -> Vec<StandingsEntry> {
    let mut pts: HashMap<String, i32> = HashMap::new();
    let mut wins: HashMap<String, u32> = HashMap::new();
    for round in &champ.rounds {
        for s in resolve_sessions(&round.session_ids, sessions) {
            if s.session_type != 5 {
                continue;
            }
            for r in &s.results {
                let p = pts.entry(r.name.clone()).or_insert(0);
                wins.entry(r.name.clone()).or_insert(0);
                if !r.dnf {
                    let pos = r.race_position as usize;
                    if pos > 0 && pos <= champ.points_system.len() {
                        *p += champ.points_system[pos - 1];
                    }
                    if r.race_position == 1 {
                        *wins.entry(r.name.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut out: Vec<StandingsEntry> = pts
        .into_iter()
        .map(|(name, points)| StandingsEntry {
            points,
            wins: wins.get(&name).copied().unwrap_or(0),
            name,
        })
        .collect();
    out.sort_by(|a, b| b.points.cmp(&a.points).then(b.wins.cmp(&a.wins)));
    out
}

/// The player's manual team override, if this result is the player's and one is set.
/// AMS2 shared memory has no livery/team field, so this is the only way to give the
/// player's row a team name when their profile name doesn't match a Custom AI Driver entry.
fn resolve_player_team<'a>(r: &SessionResult, champ: &'a Championship) -> Option<&'a str> {
    if !r.is_player {
        return None;
    }
    champ.player_team.as_deref().filter(|t| !t.is_empty())
}

fn constructors(
    champ: &Championship,
    sessions: &[RecordedSession],
    team_map: &HashMap<String, String>,
) -> Vec<StandingsEntry> {
    let mut pts: HashMap<String, i32> = HashMap::new();
    let mut wins: HashMap<String, u32> = HashMap::new();
    for round in &champ.rounds {
        for s in resolve_sessions(&round.session_ids, sessions) {
            if s.session_type != 5 {
                continue;
            }
            for r in &s.results {
                let key = if let Some(team) = team_map.get(&r.name) {
                    team.to_string()
                } else if let Some(team) = resolve_player_team(r, champ) {
                    team.to_string()
                } else if !r.car_name.is_empty() {
                    r.car_name.clone()
                } else if !r.car_class.is_empty() {
                    r.car_class.clone()
                } else {
                    continue;
                };
                let p = pts.entry(key.clone()).or_insert(0);
                wins.entry(key.clone()).or_insert(0);
                if !r.dnf {
                    let pos = r.race_position as usize;
                    if pos > 0 && pos <= champ.points_system.len() {
                        *p += champ.points_system[pos - 1];
                    }
                    if r.race_position == 1 {
                        *wins.entry(key.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut out: Vec<StandingsEntry> = pts
        .into_iter()
        .map(|(name, points)| StandingsEntry {
            points,
            wins: wins.get(&name).copied().unwrap_or(0),
            name,
        })
        .collect();
    out.sort_by(|a, b| b.points.cmp(&a.points).then(b.wins.cmp(&a.wins)));
    out
}

/// Resolve the driver-name -> team/livery-name map for a championship's assigned
/// Custom AI Driver file, if any. Returns an empty map when no file is assigned,
/// no directory is configured, or the file can't be parsed.
fn resolve_team_map(champ: &Championship, ai_dir: Option<&Path>) -> HashMap<String, String> {
    match (ai_dir, &champ.custom_ai_file) {
        (Some(dir), Some(file)) => crate::custom_ai::parse_driver_teams(&dir.join(file)),
        _ => HashMap::new(),
    }
}

/// Car classes the user is currently racing, used to pre-select the class filter on the
/// performance tabs so a 14-class list opens on the one season that matters.
///
/// `Progress` means rounds are already under way, so those win outright. When nothing is under
/// way the not-yet-started `Active` ones are the next best answer — they are what is being set
/// up. `Final` is never offered: that season is done. A championship with no Custom AI file has
/// no class to contribute.
///
/// Empty means "no preference", which callers should render as everything selected rather than
/// nothing.
pub fn active_classes(champs: &[Championship]) -> Vec<String> {
    fn classes_of(champs: &[Championship], want: ChampionshipStatus) -> Vec<String> {
        let mut out: Vec<String> = champs
            .iter()
            .filter(|c| c.status == want)
            .filter_map(|c| {
                let file = c.custom_ai_file.as_deref()?;
                Path::new(file).file_stem()?.to_str().map(str::to_string)
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }
    let in_progress = classes_of(champs, ChampionshipStatus::Progress);
    if in_progress.is_empty() {
        classes_of(champs, ChampionshipStatus::Active)
    } else {
        in_progress
    }
}

pub fn compute_career(champs: &[Championship], sessions: &[RecordedSession]) -> CareerResponse {
    compute_career_full(champs, sessions, None)
}

/// Like [`compute_career`], but resolves each championship's assigned Custom AI Driver file
/// (relative to `ai_dir`) to substitute team/livery names for driver car labels.
pub fn compute_career_full(
    champs: &[Championship],
    sessions: &[RecordedSession],
    ai_dir: Option<&Path>,
) -> CareerResponse {
    #[derive(Default)]
    struct Accum {
        races: u32,
        p1: u32,
        p2: u32,
        p3: u32,
        top10: u32,
        dnf: u32,
        quali_p1: u32,
        quali_p2: u32,
        quali_p3: u32,
        quali_top10: u32,
        champ_wins: u32,
        champ_p2: u32,
        champ_p3: u32,
        total_pos: u32,
    }
    let mut accum: HashMap<String, Accum> = HashMap::new();
    let mut championships: Vec<ChampionshipView> = Vec::new();

    for champ in champs {
        let team_map = resolve_team_map(champ, ai_dir);
        let driver_standings = standings(champ, sessions);
        let constructor_standings = constructors(champ, sessions, &team_map);

        if champ.status == ChampionshipStatus::Final {
            if let Some(w) = driver_standings.first() {
                accum.entry(w.name.clone()).or_default().champ_wins += 1;
            }
            if let Some(w) = driver_standings.get(1) {
                accum.entry(w.name.clone()).or_default().champ_p2 += 1;
            }
            if let Some(w) = driver_standings.get(2) {
                accum.entry(w.name.clone()).or_default().champ_p3 += 1;
            }
        }

        let mut rounds: Vec<RoundView> = Vec::new();
        for round in &champ.rounds {
            let mut rsessions: Vec<&RecordedSession> =
                resolve_sessions(&round.session_ids, sessions);
            rsessions.sort_by_key(|s| s.session_type);

            let mut session_views: Vec<SessionView> = Vec::new();
            for s in &rsessions {
                let is_race = s.session_type == 5;
                let mut result_views: Vec<SessionResultView> = Vec::new();
                for r in &s.results {
                    let a = accum.entry(r.name.clone()).or_default();
                    let points_earned = if is_race && !r.dnf {
                        let pos = r.race_position as usize;
                        if pos > 0 && pos <= champ.points_system.len() {
                            champ.points_system[pos - 1]
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    if is_race {
                        a.races += 1;
                        if r.dnf {
                            a.dnf += 1;
                        } else {
                            if r.race_position == 1 {
                                a.p1 += 1;
                            }
                            if r.race_position == 2 {
                                a.p2 += 1;
                            }
                            if r.race_position == 3 {
                                a.p3 += 1;
                            }
                            if r.race_position <= 10 {
                                a.top10 += 1;
                            }
                        }
                        a.total_pos += r.race_position;
                    } else if s.session_type == 3 {
                        if r.race_position == 1 {
                            a.quali_p1 += 1;
                        }
                        if r.race_position == 2 {
                            a.quali_p2 += 1;
                        }
                        if r.race_position == 3 {
                            a.quali_p3 += 1;
                        }
                        if r.race_position <= 10 {
                            a.quali_top10 += 1;
                        }
                    }
                    let car_name = team_map
                        .get(&r.name)
                        .cloned()
                        .or_else(|| resolve_player_team(r, champ).map(|s| s.to_string()))
                        .unwrap_or_else(|| r.car_name.clone());
                    result_views.push(SessionResultView {
                        name: r.name.clone(),
                        car_name,
                        car_class: r.car_class.clone(),
                        race_position: r.race_position,
                        laps_completed: r.laps_completed,
                        fastest_lap: r.fastest_lap,
                        last_lap: r.last_lap,
                        dnf: r.dnf,
                        points_earned,
                    });
                }
                session_views.push(SessionView {
                    id: s.id.clone(),
                    recorded_at: s.recorded_at,
                    track: s.track.clone(),
                    track_variation: s.track_variation.clone(),
                    session_type: s.session_type,
                    results: result_views,
                    lap_chart: s.lap_chart.clone(),
                });
            }
            rounds.push(RoundView {
                sessions: session_views,
            });
        }

        championships.push(ChampionshipView {
            id: champ.id.clone(),
            name: champ.name.clone(),
            status: champ.status.clone(),
            points_system: champ.points_system.clone(),
            manufacturer_scoring: champ.manufacturer_scoring,
            driver_standings,
            constructor_standings,
            rounds,
        });
    }

    let mut driver_stats: Vec<DriverStat> = accum
        .into_iter()
        .map(|(name, a)| DriverStat {
            avg_pos: if a.races > 0 {
                a.total_pos as f32 / a.races as f32
            } else {
                0.0
            },
            name,
            races: a.races,
            p1: a.p1,
            p2: a.p2,
            p3: a.p3,
            top10: a.top10,
            dnf: a.dnf,
            quali_p1: a.quali_p1,
            quali_p2: a.quali_p2,
            quali_p3: a.quali_p3,
            quali_top10: a.quali_top10,
            champ_wins: a.champ_wins,
            champ_p2: a.champ_p2,
            champ_p3: a.champ_p3,
        })
        .collect();
    driver_stats.sort_by(|a, b| {
        b.p1.cmp(&a.p1)
            .then(b.p2.cmp(&a.p2))
            .then(b.races.cmp(&a.races))
    });

    // ── Track stats — grouped by (track, variation, player car) ──────────────
    #[derive(Default)]
    struct TrackAccum {
        races: u32,
        qualifyings: u32,
        last_visited: u64,
        // driver_name -> (best_lap_time, result_car)
        driver_bests: HashMap<String, (f32, String)>,
    }
    let mut track_accum: HashMap<(String, String, String), TrackAccum> = HashMap::new();
    for s in sessions {
        let car = if !s.car_name.is_empty() {
            s.car_name.clone()
        } else {
            s.car_class.clone()
        };
        let key = (s.track.clone(), s.track_variation.clone(), car);
        let a = track_accum.entry(key).or_default();
        if s.session_type == 5 {
            a.races += 1;
        }
        if s.session_type == 3 {
            a.qualifyings += 1;
        }
        if s.recorded_at > a.last_visited {
            a.last_visited = s.recorded_at;
        }
        for r in &s.results {
            if r.fastest_lap <= 0.0 {
                continue;
            }
            let result_car = if !r.car_name.is_empty() {
                r.car_name.clone()
            } else {
                r.car_class.clone()
            };
            let entry = a
                .driver_bests
                .entry(r.name.clone())
                .or_insert((f32::MAX, String::new()));
            if r.fastest_lap < entry.0 {
                *entry = (r.fastest_lap, result_car);
            }
        }
    }
    fn lap_slot(
        driver_bests: &HashMap<String, (f32, String)>,
        rank: usize,
    ) -> (f32, String, String) {
        let mut sorted: Vec<(&String, &(f32, String))> = driver_bests.iter().collect();
        sorted.sort_by(|a, b| {
            a.1 .0
                .partial_cmp(&b.1 .0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
            .get(rank)
            .map(|(name, (t, c))| (*t, (*name).clone(), c.clone()))
            .unwrap_or((0.0, String::new(), String::new()))
    }
    let mut track_stats: Vec<TrackStat> = track_accum
        .into_iter()
        .map(|((track, track_variation, car), a)| {
            let (best_lap, best_lap_driver, best_lap_car) = lap_slot(&a.driver_bests, 0);
            let (second_lap, second_lap_driver, second_lap_car) = lap_slot(&a.driver_bests, 1);
            let (third_lap, third_lap_driver, third_lap_car) = lap_slot(&a.driver_bests, 2);
            TrackStat {
                track,
                track_variation,
                car,
                races: a.races,
                qualifyings: a.qualifyings,
                best_lap,
                best_lap_driver,
                best_lap_car,
                second_lap,
                second_lap_driver,
                second_lap_car,
                third_lap,
                third_lap_driver,
                third_lap_car,
                last_visited: a.last_visited,
            }
        })
        .collect();
    track_stats.sort_by(|a, b| b.last_visited.cmp(&a.last_visited));

    CareerResponse {
        championships,
        driver_stats,
        track_stats,
    }
}

/// Whether `path` may be written over.
///
/// A file that exists but will not parse was never loaded, so whatever is in memory did not come
/// from it — writing would destroy a career nobody has read. Checked here, at the point of
/// danger, rather than remembered from load time: it is stateless, it cannot desync from a flag
/// somebody forgot to set, and it also catches a file that was damaged *after* startup.
///
/// Only the shape is checked, not the content — `IgnoredAny` walks the JSON without building a
/// `CareerData`, so guarding every write costs a parse and no allocation.
fn safe_to_overwrite(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str::<serde::de::IgnoredAny>(content.strip_prefix(BOM).unwrap_or(&content))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Writes the career to `path`, unless that would overwrite a save that could not be read.
///
/// Returns the reason it declined, so a route can report it rather than appearing to succeed.
/// The console warning is unconditional: a background recorder has nowhere else to say it, and
/// silently not saving is exactly the failure this guard exists to make loud.
pub fn persist(store: &SharedStore, path: &PathBuf) -> Result<(), String> {
    if let Err(e) = safe_to_overwrite(path) {
        let msg = format!(
            "refusing to overwrite {}: it exists but could not be read ({e}). \
             Fix or move the file — nothing in it has been changed.",
            path.display()
        );
        eprintln!("ERROR: {msg}");
        return Err(msg);
    }
    let data = store.read().unwrap();
    let content = serde_json::to_string_pretty(&*data).unwrap_or_default();
    fs::write(path, content).map_err(|e| {
        let msg = format!("failed to save career data: {e}");
        eprintln!("ERROR: {msg}");
        msg
    })
}

#[cfg(test)]
#[path = "tests/data_store.rs"]
mod tests;
