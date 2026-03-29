use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

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
}

/// A championship round — groups one or more sessions (Practice / Qualify / Race).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Round {
    /// Session IDs that belong to this round, in any order.
    pub session_ids: Vec<String>,
}

/// User-created championship grouping a set of rounds.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Championship {
    pub id: String,
    pub name: String,
    /// "Pending", "Active", or "Finished".
    pub status: String,
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
}

/// Root data structure persisted to ams2_career.json.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CareerData {
    pub sessions: Vec<RecordedSession>,
    pub championships: Vec<Championship>,
}

pub type SharedStore = Arc<RwLock<CareerData>>;

pub fn load_store(path: &PathBuf) -> SharedStore {
    let mut data: CareerData = if path.exists() {
        let content = fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        CareerData::default()
    };
    // Migrate legacy flat session_ids → one round per session.
    for champ in &mut data.championships {
        if champ.rounds.is_empty() && !champ.session_ids.is_empty() {
            champ.rounds = champ
                .session_ids
                .drain(..)
                .map(|sid| Round { session_ids: vec![sid] })
                .collect();
        }
    }
    Arc::new(RwLock::new(data))
}

// ── Career computation ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StandingsEntry {
    pub name: String,
    pub points: i32,
    pub wins: u32,
}

/// Round with sessions already resolved from IDs.
#[derive(Serialize)]
pub struct RoundView {
    pub sessions: Vec<RecordedSession>,
}

#[derive(Serialize)]
pub struct ChampionshipView {
    pub id: String,
    pub name: String,
    pub status: String,
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
    pub wins: u32,
    pub top3: u32,
    pub top10: u32,
    pub dnf: u32,
    pub champ_wins: u32,
    pub avg_pos: f32,
}

#[derive(Serialize)]
pub struct CareerResponse {
    pub championships: Vec<ChampionshipView>,
    pub driver_stats: Vec<DriverStat>,
}

fn resolve_sessions<'a>(ids: &[String], sessions: &'a [RecordedSession]) -> Vec<&'a RecordedSession> {
    ids.iter().filter_map(|id| sessions.iter().find(|s| s.id == *id)).collect()
}

fn standings(champ: &Championship, sessions: &[RecordedSession]) -> Vec<StandingsEntry> {
    let mut pts: HashMap<String, i32> = HashMap::new();
    let mut wins: HashMap<String, u32> = HashMap::new();
    for round in &champ.rounds {
        for s in resolve_sessions(&round.session_ids, sessions) {
            if s.session_type != 5 { continue; }
            for r in &s.results {
                pts.entry(r.name.clone()).or_insert(0);
                wins.entry(r.name.clone()).or_insert(0);
                if !r.dnf {
                    let pos = r.race_position as usize;
                    if pos > 0 && pos <= champ.points_system.len() {
                        *pts.get_mut(&r.name).unwrap() += champ.points_system[pos - 1];
                    }
                    if r.race_position == 1 {
                        *wins.get_mut(&r.name).unwrap() += 1;
                    }
                }
            }
        }
    }
    let mut out: Vec<StandingsEntry> = pts.into_iter().map(|(name, points)| StandingsEntry {
        points, wins: wins.get(&name).copied().unwrap_or(0), name,
    }).collect();
    out.sort_by(|a, b| b.points.cmp(&a.points).then(b.wins.cmp(&a.wins)));
    out
}

fn constructors(champ: &Championship, sessions: &[RecordedSession]) -> Vec<StandingsEntry> {
    let mut pts: HashMap<String, i32> = HashMap::new();
    let mut wins: HashMap<String, u32> = HashMap::new();
    for round in &champ.rounds {
        for s in resolve_sessions(&round.session_ids, sessions) {
            if s.session_type != 5 { continue; }
            for r in &s.results {
                let key = if !r.car_name.is_empty() { &r.car_name }
                          else if !r.car_class.is_empty() { &r.car_class }
                          else { continue };
                pts.entry(key.clone()).or_insert(0);
                wins.entry(key.clone()).or_insert(0);
                if !r.dnf {
                    let pos = r.race_position as usize;
                    if pos > 0 && pos <= champ.points_system.len() {
                        *pts.get_mut(key).unwrap() += champ.points_system[pos - 1];
                    }
                    if r.race_position == 1 {
                        *wins.get_mut(key).unwrap() += 1;
                    }
                }
            }
        }
    }
    let mut out: Vec<StandingsEntry> = pts.into_iter().map(|(name, points)| StandingsEntry {
        points, wins: wins.get(&name).copied().unwrap_or(0), name,
    }).collect();
    out.sort_by(|a, b| b.points.cmp(&a.points).then(b.wins.cmp(&a.wins)));
    out
}

pub fn compute_career(champs: &[Championship], sessions: &[RecordedSession]) -> CareerResponse {
    #[derive(Default)]
    struct Accum { races: u32, wins: u32, top3: u32, top10: u32, dnf: u32, champ_wins: u32, total_pos: u32 }
    let mut accum: HashMap<String, Accum> = HashMap::new();
    let mut championships: Vec<ChampionshipView> = Vec::new();

    for champ in champs {
        let driver_standings = standings(champ, sessions);
        let constructor_standings = constructors(champ, sessions);

        if champ.status == "Finished" {
            if let Some(w) = driver_standings.first() {
                accum.entry(w.name.clone()).or_default().champ_wins += 1;
            }
        }

        let mut rounds: Vec<RoundView> = Vec::new();
        for round in &champ.rounds {
            let mut rsessions: Vec<RecordedSession> = resolve_sessions(&round.session_ids, sessions)
                .into_iter().cloned().collect();
            rsessions.sort_by_key(|s| s.session_type);

            for s in &rsessions {
                if s.session_type != 5 { continue; }
                for r in &s.results {
                    let a = accum.entry(r.name.clone()).or_default();
                    a.races += 1;
                    if r.dnf { a.dnf += 1; }
                    else {
                        if r.race_position == 1 { a.wins += 1; }
                        if r.race_position <= 3 { a.top3 += 1; }
                        if r.race_position <= 10 { a.top10 += 1; }
                    }
                    a.total_pos += r.race_position;
                }
            }
            rounds.push(RoundView { sessions: rsessions });
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

    let mut driver_stats: Vec<DriverStat> = accum.into_iter().map(|(name, a)| DriverStat {
        avg_pos: if a.races > 0 { a.total_pos as f32 / a.races as f32 } else { 0.0 },
        name, races: a.races, wins: a.wins, top3: a.top3, top10: a.top10,
        dnf: a.dnf, champ_wins: a.champ_wins,
    }).collect();
    driver_stats.sort_by(|a, b| b.wins.cmp(&a.wins).then(b.races.cmp(&a.races)));

    CareerResponse { championships, driver_stats }
}

pub fn persist(store: &SharedStore, path: &PathBuf) {
    let data = store.read().unwrap();
    let content = serde_json::to_string_pretty(&*data).unwrap_or_default();
    if let Err(e) = fs::write(path, content) {
        eprintln!("Failed to save career data: {e}");
    }
}

#[cfg(test)]
#[path = "data_store_tests.rs"]
mod tests;
