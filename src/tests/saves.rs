use super::*;
use std::fs;

fn tmp_dir(tag: &str) -> PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_saves_test_{tag}_{ns}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

const CAREER_JSON: &str = r#"{"sessions":[{"id":"1","recorded_at":1,"track":"Interlagos","car_class":"GT3","session_type":5,"results":[]}],"championships":[]}"#;

#[test]
fn test_list_saves_finds_json_files_and_skips_subdirs() {
    let dir = tmp_dir("list");
    fs::write(save_path(&dir, "ams2_career"), CAREER_JSON).unwrap();
    fs::write(
        save_path(&dir, "gt3"),
        r#"{"sessions":[],"championships":[]}"#,
    )
    .unwrap();
    fs::write(dir.join("notes.txt"), "ignore me").unwrap();
    fs::create_dir_all(dir.join("track_layouts")).unwrap();
    fs::write(dir.join("track_layouts").join("interlagos.json"), "[]").unwrap();

    let active = save_path(&dir, "gt3");
    let saves = list_saves(&dir, &active);

    let names: Vec<&str> = saves.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["ams2_career", "gt3"],
        "sorted, subdir + txt skipped"
    );
    assert_eq!(saves[0].sessions, 1);
    assert!(!saves[0].active);
    assert_eq!(saves[1].sessions, 0);
    assert!(saves[1].active);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_list_saves_appends_active_file_outside_dir() {
    let dir = tmp_dir("outside");
    let other = tmp_dir("outside_active");
    fs::write(save_path(&dir, "ams2_career"), CAREER_JSON).unwrap();
    let active = save_path(&other, "legacy");
    fs::write(&active, CAREER_JSON).unwrap();

    let saves = list_saves(&dir, &active);
    assert_eq!(saves.len(), 2);
    assert_eq!(saves[1].name, "legacy");
    assert!(
        saves[1].active,
        "legacy path outside the dir stays selectable"
    );
    assert!(!saves[0].active);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&other);
}

#[test]
fn test_list_saves_missing_dir_is_empty_except_active() {
    let dir = std::env::temp_dir().join("ams2_saves_test_does_not_exist_xyz");
    let saves = list_saves(&dir, &dir.join("nope.json"));
    // Active file doesn't exist either → load_data yields an empty career, still listed.
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].sessions, 0);
}

#[test]
fn test_sanitize_name_accepts_plain_names() {
    assert_eq!(sanitize_name("GT3 Career").as_deref(), Some("GT3 Career"));
    assert_eq!(sanitize_name("  spaced  ").as_deref(), Some("spaced"));
    assert_eq!(
        sanitize_name("formula_retro-2024").as_deref(),
        Some("formula_retro-2024")
    );
}

#[test]
fn test_sanitize_name_rejects_traversal_and_separators() {
    assert!(sanitize_name("").is_none());
    assert!(sanitize_name("   ").is_none());
    assert!(sanitize_name("..").is_none());
    assert!(sanitize_name("../x").is_none());
    assert!(sanitize_name("a/b").is_none());
    assert!(sanitize_name("a\\b").is_none());
    assert!(sanitize_name("C:file").is_none());
    assert!(
        sanitize_name("name.json").is_none(),
        "extension is added by save_path"
    );
    assert!(sanitize_name(&"x".repeat(65)).is_none());
}

#[test]
fn test_save_path_appends_json() {
    let dir = PathBuf::from("saves");
    assert_eq!(save_path(&dir, "GT3"), dir.join("GT3.json"));
}

#[test]
fn test_resolve_dir_defaults_to_championships_next_to_exe() {
    let exe = PathBuf::from("C:/apps");
    assert_eq!(resolve_dir(&exe, None), exe.join("championships"));
    assert_eq!(resolve_dir(&exe, Some("   ")), exe.join("championships"));
    assert_eq!(
        resolve_dir(&exe, Some("D:/careers")),
        PathBuf::from("D:/careers")
    );
    assert_eq!(
        resolve_dir(&exe, Some("  D:/careers  ")),
        PathBuf::from("D:/careers")
    );
}

#[test]
fn test_resolve_active_prefers_the_configured_file_when_it_exists() {
    let dir = tmp_dir("resolve_cfg");
    let gt3 = save_path(&dir, "gt3");
    fs::write(&gt3, CAREER_JSON).unwrap();
    fs::write(save_path(&dir, "ams2_career"), CAREER_JSON).unwrap();

    let picked = resolve_active(&dir, Some(gt3.to_str().unwrap()));
    assert_eq!(picked, gt3, "a remembered save wins over the default");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_falls_back_to_default_when_configured_file_is_gone() {
    // After the saves folder changes, the remembered path points into the old folder.
    let dir = tmp_dir("resolve_stale");
    let default = save_path(&dir, "ams2_career");
    fs::write(&default, CAREER_JSON).unwrap();

    let picked = resolve_active(&dir, Some("Z:/old_folder/gone.json"));
    assert_eq!(picked, default);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_picks_first_save_when_no_default_exists() {
    let dir = tmp_dir("resolve_first");
    fs::write(save_path(&dir, "zzz"), CAREER_JSON).unwrap();
    fs::write(save_path(&dir, "alpha"), CAREER_JSON).unwrap();
    fs::write(dir.join("notes.txt"), "ignore").unwrap();

    let picked = resolve_active(&dir, None);
    assert_eq!(
        picked,
        save_path(&dir, "alpha"),
        "alphabetically first save"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_empty_folder_yields_default_path() {
    let dir = tmp_dir("resolve_empty");
    let picked = resolve_active(&dir, None);
    assert_eq!(picked, save_path(&dir, "ams2_career"));
    assert!(!picked.exists(), "path is returned, not created");
    let _ = fs::remove_dir_all(&dir);
}
