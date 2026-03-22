use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Final result for one participant in a recorded session.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionResult {
    pub name: String,
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

pub fn persist(store: &SharedStore, path: &PathBuf) {
    let data = store.read().unwrap();
    let content = serde_json::to_string_pretty(&*data).unwrap_or_default();
    if let Err(e) = fs::write(path, content) {
        eprintln!("Failed to save career data: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Returns a unique temp path that does not yet exist.
    fn tmp() -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        std::env::temp_dir().join(format!("ams2_test_{ns}.json"))
    }

    fn sample_championship() -> Championship {
        Championship {
            id: "1".into(),
            name: "Formula Test".into(),
            status: "Active".into(),
            points_system: vec![25, 18, 15, 12, 10],
            manufacturer_scoring: false,
            rounds: vec![Round { session_ids: vec!["100".into()] }],
            session_ids: vec!["100".into()],
        }
    }

    fn sample_session() -> RecordedSession {
        RecordedSession {
            id: "100".into(),
            recorded_at: 1_700_000_000,
            track: "Silverstone".into(),
            session_type: 5,
            results: vec![
                SessionResult {
                    name: "Alice".into(),
                    race_position: 1,
                    laps_completed: 20,
                    fastest_lap: 89.5,
                    last_lap: 90.1,
                    dnf: false,
                },
                SessionResult {
                    name: "Bob".into(),
                    race_position: 2,
                    laps_completed: 20,
                    fastest_lap: 90.0,
                    last_lap: 91.0,
                    dnf: false,
                },
            ],
        }
    }

    // ── load_store ────────────────────────────────────────────────────────────

    #[test]
    fn test_load_store_nonexistent_file_returns_empty_default() {
        let path = tmp();
        let store = load_store(&path);
        let data = store.read().unwrap();
        assert!(data.sessions.is_empty());
        assert!(data.championships.is_empty());
    }

    #[test]
    fn test_load_store_invalid_json_returns_empty_default() {
        let path = tmp();
        fs::write(&path, "not { valid } json %%%").unwrap();
        let store = load_store(&path);
        let data = store.read().unwrap();
        assert!(data.sessions.is_empty());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_store_empty_object_returns_empty_default() {
        let path = tmp();
        fs::write(&path, "{}").unwrap();
        let store = load_store(&path);
        let data = store.read().unwrap();
        assert!(data.sessions.is_empty());
        assert!(data.championships.is_empty());
        fs::remove_file(&path).ok();
    }

    // ── persist ───────────────────────────────────────────────────────────────

    #[test]
    fn test_persist_and_reload_championship() {
        let path = tmp();
        let store = load_store(&path);
        store.write().unwrap().championships.push(sample_championship());
        persist(&store, &path);

        let store2 = load_store(&path);
        let data = store2.read().unwrap();
        assert_eq!(data.championships.len(), 1);
        assert_eq!(data.championships[0].name, "Formula Test");
        assert_eq!(data.championships[0].points_system, vec![25, 18, 15, 12, 10]);
        // session_ids is skip_serializing; rounds persist instead
        assert_eq!(data.championships[0].rounds.len(), 1);
        assert_eq!(data.championships[0].rounds[0].session_ids, vec!["100"]);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_persist_and_reload_session() {
        let path = tmp();
        let store = load_store(&path);
        store.write().unwrap().sessions.push(sample_session());
        persist(&store, &path);

        let store2 = load_store(&path);
        let data = store2.read().unwrap();
        assert_eq!(data.sessions.len(), 1);
        assert_eq!(data.sessions[0].track, "Silverstone");
        assert_eq!(data.sessions[0].results.len(), 2);
        assert_eq!(data.sessions[0].results[0].name, "Alice");
        assert_eq!(data.sessions[0].results[0].fastest_lap, 89.5);
        assert!(!data.sessions[0].results[0].dnf);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_persist_overwrites_previous_contents() {
        let path = tmp();
        let store = load_store(&path);
        store.write().unwrap().championships.push(sample_championship());
        persist(&store, &path);

        // Add a second championship and persist again.
        let mut c2 = sample_championship();
        c2.id = "2".into();
        c2.name = "Second Champ".into();
        store.write().unwrap().championships.push(c2);
        persist(&store, &path);

        let store3 = load_store(&path);
        assert_eq!(store3.read().unwrap().championships.len(), 2);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_persist_writes_valid_json() {
        let path = tmp();
        let store = load_store(&path);
        store.write().unwrap().sessions.push(sample_session());
        persist(&store, &path);

        let raw = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(parsed.get("sessions").is_some());
        assert!(parsed.get("championships").is_some());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_dnf_result_round_trips() {
        let path = tmp();
        let store = load_store(&path);
        let mut session = sample_session();
        session.results[1].dnf = true;
        store.write().unwrap().sessions.push(session);
        persist(&store, &path);

        let store2 = load_store(&path);
        let data = store2.read().unwrap();
        assert!(data.sessions[0].results[1].dnf);
        fs::remove_file(&path).ok();
    }
}
