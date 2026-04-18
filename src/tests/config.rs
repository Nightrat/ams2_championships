use super::*;
use std::fs;

fn tmp_path() -> std::path::PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("ams2_cfg_test_{ns}.json"))
}

#[test]
fn test_load_or_create_missing_file_writes_defaults_and_creates_file() {
    let path = tmp_path();
    assert!(!path.exists());
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.poll_ms, 200);
    assert!(cfg.record_practice);
    assert!(cfg.record_qualify);
    assert!(cfg.record_race);
    assert!(cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 5000);
    assert!(cfg.data_file.is_none());
    assert!(path.exists(), "config file should be created");
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_valid_json_reads_values() {
    let path = tmp_path();
    fs::write(&path, r#"{"port":9090,"host":"0.0.0.0","poll_ms":500,"record_practice":false,"record_qualify":true,"record_race":true,"show_track_map":false,"track_map_max_points":1000}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 9090);
    assert_eq!(cfg.host, "0.0.0.0");
    assert_eq!(cfg.poll_ms, 500);
    assert!(!cfg.record_practice);
    assert!(cfg.record_qualify);
    assert!(!cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 1000);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_partial_json_fills_serde_defaults() {
    let path = tmp_path();
    // Only provide port — all other fields should get their serde defaults
    fs::write(&path, r#"{"port":7777}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 7777);
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.poll_ms, 200);
    assert!(cfg.record_race);
    assert!(cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 5000);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_invalid_json_returns_defaults() {
    let path = tmp_path();
    fs::write(&path, "not valid json {{{{").unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.host, "127.0.0.1");
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_rewrites_file_with_all_fields() {
    let path = tmp_path();
    // Write a minimal config — on load it should be rewritten with all fields present
    fs::write(&path, r#"{"port":9000}"#).unwrap();
    load_or_create(&path);
    let written = fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    for key in &["host", "poll_ms", "record_practice", "record_qualify", "record_race",
                 "show_track_map", "track_map_max_points"] {
        assert!(v.get(key).is_some(), "expected key '{key}' in rewritten config");
    }
    let _ = fs::remove_file(&path);
}

#[test]
fn test_config_default_values() {
    let cfg = Config::default();
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.poll_ms, 200);
    assert!(cfg.record_practice);
    assert!(cfg.record_qualify);
    assert!(cfg.record_race);
    assert!(cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 5000);
    assert!(cfg.data_file.is_none());
}

#[test]
fn test_load_or_create_data_file_some() {
    let path = tmp_path();
    fs::write(&path, r#"{"data_file":"/some/path/career.json"}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.data_file, Some("/some/path/career.json".into()));
    let _ = fs::remove_file(&path);
}
