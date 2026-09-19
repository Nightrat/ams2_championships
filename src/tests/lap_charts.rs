use super::*;
use crate::data_store::{RecordedSession, SessionResult};
use std::fs;

fn tmp_dir(tag: &str) -> PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_laps_test_{tag}_{ns}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn entry(lap: u32, driver: &str, position: u32) -> LapChartEntry {
    LapChartEntry {
        lap,
        driver: driver.into(),
        position,
    }
}

fn session(id: &str, chart: Vec<LapChartEntry>) -> RecordedSession {
    RecordedSession {
        id: id.into(),
        recorded_at: 0,
        track: "Interlagos".into(),
        track_variation: String::new(),
        car_name: String::new(),
        car_class: "F-Classic_Gen1".into(),
        session_type: 5,
        results: vec![SessionResult {
            name: "Nightrat".into(),
            car_name: String::new(),
            car_class: "F-Classic_Gen1".into(),
            race_position: 1,
            laps_completed: 10,
            fastest_lap: 90.0,
            last_lap: 90.0,
            dnf: false,
            is_player: true,
        }],
        lap_chart: chart,
    }
}

/// A folder save, which is the only kind that can keep charts beside itself.
fn folder_save(dir: &Path, name: &str) -> PathBuf {
    let career = crate::saves::save_path(dir, name);
    crate::saves::prepare_save_dir(&career).unwrap();
    fs::write(&career, r#"{"sessions":[],"championships":[]}"#).unwrap();
    career
}

#[test]
fn test_a_chart_round_trips_through_its_own_file() {
    let dir = tmp_dir("round_trip");
    let career = folder_save(&dir, "career");
    let chart = vec![
        entry(1, "Senna", 1),
        entry(1, "Prost", 2),
        entry(2, "Prost", 1),
    ];

    write(&career, "s1", &chart).unwrap();
    let back = read(&career, "s1");
    assert_eq!(back.len(), 3);
    assert_eq!(back[2].driver, "Prost");
    assert_eq!(back[2].position, 1);

    // Beside the career, in its own folder, named by the session.
    assert!(career
        .parent()
        .unwrap()
        .join("laps")
        .join("s1.json")
        .is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_externalizing_empties_the_session_and_leaves_the_career_file_smaller() {
    // The point of the whole module: the chart is the bulk of a session, and the career file is
    // rewritten on every single change, so the chart must not be in it.
    let dir = tmp_dir("externalize");
    let career = folder_save(&dir, "career");
    let mut sessions = vec![
        session("s1", vec![entry(1, "Senna", 1), entry(2, "Senna", 1)]),
        session("s2", vec![]),
        session("s3", vec![entry(1, "Prost", 1)]),
    ];

    assert_eq!(
        externalize(&career, &mut sessions),
        2,
        "only the two with charts"
    );
    assert!(sessions.iter().all(|s| s.lap_chart.is_empty()));
    assert_eq!(read(&career, "s1").len(), 2);
    assert_eq!(read(&career, "s3").len(), 1);
    // A session that never had one gets no file rather than an empty one.
    assert_eq!(read(&career, "s2").len(), 0);
    assert!(!career
        .parent()
        .unwrap()
        .join("laps")
        .join("s2.json")
        .exists());

    // Running it again has nothing left to do — it is not a repeated cost on every save.
    assert_eq!(externalize(&career, &mut sessions), 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_a_legacy_flat_save_keeps_its_charts_inline() {
    // A flat save is a bare file with no folder to put anything beside, and those saves are
    // never migrated. Moving the chart nowhere and leaving it where it is must not lose it.
    let dir = tmp_dir("flat");
    let career = crate::saves::legacy_save_path(&dir, "old");
    fs::write(&career, r#"{"sessions":[],"championships":[]}"#).unwrap();

    assert!(laps_dir(&career).is_none());
    assert!(chart_path(&career, "s1").is_none());
    assert!(write(&career, "s1", &[entry(1, "Senna", 1)]).is_err());

    let mut sessions = vec![session("s1", vec![entry(1, "Senna", 1)])];
    assert_eq!(externalize(&career, &mut sessions), 0);
    assert_eq!(sessions[0].lap_chart.len(), 1, "still there, not dropped");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_a_session_id_cannot_escape_the_laps_folder() {
    // Ids are written as unix timestamps, but a hand-edited career can hold anything and this
    // value is joined onto a path.
    let dir = tmp_dir("escape");
    let career = folder_save(&dir, "career");
    for bad in ["../../evil", "..", "a/b", "a\\b", ""] {
        assert!(chart_path(&career, bad).is_none(), "{bad} must be refused");
        assert!(write(&career, bad, &[entry(1, "Senna", 1)]).is_err());
        assert!(read(&career, bad).is_empty());
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_reading_a_chart_that_is_not_there_is_empty_not_an_error() {
    // Practice sessions never have a chart, so absence is ordinary and must not fail a request.
    let dir = tmp_dir("missing");
    let career = folder_save(&dir, "career");
    assert!(read(&career, "never_recorded").is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_remove_deletes_the_chart_and_tolerates_a_missing_one() {
    let dir = tmp_dir("remove");
    let career = folder_save(&dir, "career");
    write(&career, "s1", &[entry(1, "Senna", 1)]).unwrap();

    remove(&career, "s1");
    assert!(read(&career, "s1").is_empty());
    remove(&career, "s1"); // already gone is the wanted outcome
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_a_chart_written_with_a_byte_order_mark_still_reads() {
    // Same hazard the career loader guards: Notepad and PowerShell write one by default.
    let dir = tmp_dir("bom");
    let career = folder_save(&dir, "career");
    let path = chart_path(&career, "s1").unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "\u{feff}[{\"lap\":1,\"driver\":\"Senna\",\"position\":1}]",
    )
    .unwrap();

    assert_eq!(read(&career, "s1").len(), 1);
    let _ = fs::remove_dir_all(&dir);
}
