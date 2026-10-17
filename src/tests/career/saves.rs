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
const EMPTY_JSON: &str = r#"{"sessions":[],"championships":[]}"#;

/// Write a save in the folder layout, the way every new career is created.
fn write_save(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = save_path(dir, name);
    prepare_save_dir(&path).unwrap();
    fs::write(&path, body).unwrap();
    path
}

/// Write a save in the pre-folders flat layout.
fn write_legacy(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = legacy_save_path(dir, name);
    fs::write(&path, body).unwrap();
    path
}

/// Put a file in a subfolder of a save, standing in for whatever a career comes to own beside
/// its sessions. The folder layout exists so that such things move with the career.
fn write_nested_file(career: &Path, sub: &str, file: &str) -> PathBuf {
    let dir = career_dir(career).unwrap().join(sub);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(file);
    fs::write(&path, "nested").unwrap();
    path
}

#[test]
fn test_list_saves_finds_folder_saves_and_skips_other_dirs() {
    let dir = tmp_dir("list");
    write_save(&dir, "ams2_career", CAREER_JSON);
    write_save(&dir, "gt3", EMPTY_JSON);
    fs::write(dir.join("notes.txt"), "ignore me").unwrap();
    // A directory with no career.json is not a save — which is what keeps the shared
    // track_layouts/ out of the list without naming it.
    fs::create_dir_all(dir.join("track_layouts")).unwrap();
    fs::write(dir.join("track_layouts").join("interlagos.json"), "[]").unwrap();

    let active = save_path(&dir, "gt3");
    let saves = list_saves(&dir, &active);

    let names: Vec<&str> = saves.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["ams2_career", "gt3"],
        "sorted; track_layouts and the txt are not saves"
    );
    assert_eq!(saves[0].sessions, 1);
    assert!(!saves[0].active);
    assert_eq!(saves[1].sessions, 0);
    assert!(saves[1].active);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_list_saves_still_lists_saves_written_before_folders() {
    // A career written by an older build is a bare <name>.json. It keeps working exactly as it
    // did, beside newer folder saves, and is never migrated behind the user's back.
    let dir = tmp_dir("legacy_list");
    write_legacy(&dir, "old_career", CAREER_JSON);
    write_save(&dir, "new_career", EMPTY_JSON);

    let saves = list_saves(&dir, &legacy_save_path(&dir, "old_career"));
    let names: Vec<&str> = saves.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["new_career", "old_career"]);

    let old = saves.iter().find(|s| s.name == "old_career").unwrap();
    assert_eq!(old.sessions, 1, "read from the flat file");
    assert!(old.active);
    assert!(old.error.is_none());
    assert!(
        old.file.ends_with("old_career.json"),
        "the flat path is what gets activated: {}",
        old.file
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_list_saves_folder_wins_over_a_legacy_file_of_the_same_name() {
    // Only reachable by hand — every route that creates a save refuses a name either layout
    // already holds. The folder wins because it is the layout a career grows into, and the
    // entry says so rather than letting the other file vanish silently.
    let dir = tmp_dir("shadow");
    write_save(&dir, "clash", EMPTY_JSON);
    write_legacy(&dir, "clash", CAREER_JSON);

    let saves = list_saves(&dir, &save_path(&dir, "clash"));
    assert_eq!(saves.len(), 1, "one entry per name");
    assert_eq!(saves[0].sessions, 0, "the folder save is the one read");
    let err = saves[0].error.as_deref().unwrap_or_default();
    assert!(
        err.contains("clash.json") && err.contains("ignored"),
        "the shadowed file is named: {err}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_list_saves_appends_active_file_outside_dir() {
    let dir = tmp_dir("outside");
    let other = tmp_dir("outside_active");
    write_save(&dir, "ams2_career", CAREER_JSON);
    let active = other.join("legacy.json");
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
fn test_list_saves_names_a_brand_new_career_after_its_folder() {
    // A career whose folder exists but whose file has not been written yet — the state between
    // creating one and the first save into it. The scan cannot find it, so it comes through the
    // active-save fallback. Naming that from the file stem called every folder save "career",
    // since that is what the file is always called.
    let dir = tmp_dir("fresh_name");
    let active = save_path(&dir, "ams2_career");
    prepare_save_dir(&active).unwrap();

    let saves = list_saves(&dir, &active);
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].name, "ams2_career", "named after its folder");
    assert!(saves[0].active);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_save_name_of_reads_the_folder_for_a_folder_save() {
    let dir = PathBuf::from("saves");
    assert_eq!(
        save_name_of(&save_path(&dir, "GT3 Career")).as_deref(),
        Some("GT3 Career")
    );
    assert_eq!(
        save_name_of(&legacy_save_path(&dir, "GT3 Career")).as_deref(),
        Some("GT3 Career")
    );
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
        "the layout adds the extension, not the user"
    );
    assert!(sanitize_name(&"x".repeat(65)).is_none());
}

#[test]
fn test_save_path_is_a_folder_holding_career_json() {
    let dir = PathBuf::from("saves");
    assert_eq!(save_path(&dir, "GT3"), dir.join("GT3").join("career.json"));
    assert_eq!(legacy_save_path(&dir, "GT3"), dir.join("GT3.json"));
}

#[test]
fn test_existing_save_path_finds_either_layout_and_prefers_the_folder() {
    let dir = tmp_dir("existing");
    assert!(existing_save_path(&dir, "nope").is_none());
    assert!(!name_taken(&dir, "nope"));

    write_legacy(&dir, "flat", CAREER_JSON);
    assert_eq!(
        existing_save_path(&dir, "flat"),
        Some(legacy_save_path(&dir, "flat"))
    );
    assert!(name_taken(&dir, "flat"));

    write_save(&dir, "flat", EMPTY_JSON);
    assert_eq!(
        existing_save_path(&dir, "flat"),
        Some(save_path(&dir, "flat")),
        "the folder layout wins, matching list_saves"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_only_a_folder_save_has_a_folder() {
    let dir = PathBuf::from("saves");
    assert_eq!(career_dir(&save_path(&dir, "GT3")), Some(dir.join("GT3")));
    assert_eq!(career_dir(&legacy_save_path(&dir, "GT3")), None);
}

#[test]
fn test_rename_moves_a_folder_save_with_everything_in_it() {
    let dir = tmp_dir("rename_folder");
    let career = write_save(&dir, "old", CAREER_JSON);
    write_nested_file(&career, "extra", "notes.txt");

    let to = rename_save(&dir, "old", "new").unwrap();
    assert_eq!(to, save_path(&dir, "new"));
    assert!(!dir.join("old").exists(), "the old folder is gone");
    assert!(
        career_dir(&to)
            .unwrap()
            .join("extra")
            .join("notes.txt")
            .is_file(),
        "whatever the career owned travelled with it"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_rename_keeps_a_legacy_save_flat() {
    // Renaming is not consent to change layout: a career the user did not ask to move stays put.
    let dir = tmp_dir("rename_flat");
    write_legacy(&dir, "old", CAREER_JSON);

    let to = rename_save(&dir, "old", "new").unwrap();
    assert_eq!(to, legacy_save_path(&dir, "new"));
    assert!(to.is_file());
    assert!(!legacy_save_path(&dir, "old").exists());
    assert!(!dir.join("new").is_dir());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_rename_refuses_a_missing_source_or_a_taken_name() {
    let dir = tmp_dir("rename_refuse");
    write_save(&dir, "a", CAREER_JSON);
    write_legacy(&dir, "b", CAREER_JSON);

    assert!(rename_save(&dir, "nope", "x").is_err());
    assert!(
        rename_save(&dir, "a", "b").is_err(),
        "a legacy file of that name is still a name in use"
    );
    assert!(dir.join("a").is_dir(), "the refused rename changed nothing");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_duplicate_copies_a_folder_save_including_its_contents() {
    let dir = tmp_dir("dup_folder");
    let career = write_save(&dir, "src", CAREER_JSON);
    write_nested_file(&career, "extra", "notes.txt");

    let to = duplicate_save(&dir, "src", "copy").unwrap();
    assert_eq!(to, save_path(&dir, "copy"));
    assert!(
        career_dir(&to)
            .unwrap()
            .join("extra")
            .join("notes.txt")
            .is_file(),
        "a duplicate of a career includes what it owned"
    );
    assert!(career.is_file(), "the original is untouched");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_duplicating_a_legacy_save_writes_the_copy_in_the_folder_layout() {
    // A duplicate is a new save, so it is written the way new saves are written. The original
    // stays flat — it was not the thing the user asked to change.
    let dir = tmp_dir("dup_flat");
    write_legacy(&dir, "old", CAREER_JSON);

    let to = duplicate_save(&dir, "old", "copy").unwrap();
    assert_eq!(to, save_path(&dir, "copy"));
    assert!(to.is_file());
    assert!(legacy_save_path(&dir, "old").is_file());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_delete_removes_a_folder_save_and_everything_in_it() {
    let dir = tmp_dir("del_folder");
    let career = write_save(&dir, "gone", CAREER_JSON);
    write_nested_file(&career, "extra", "notes.txt");

    delete_save(&dir, "gone").unwrap();
    assert!(!dir.join("gone").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_delete_removes_a_legacy_flat_save() {
    let dir = tmp_dir("del_flat");
    write_legacy(&dir, "gone", CAREER_JSON);
    write_save(&dir, "kept", CAREER_JSON);

    delete_save(&dir, "gone").unwrap();
    assert!(!legacy_save_path(&dir, "gone").exists());
    assert!(dir.join("kept").is_dir(), "only the named save went");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_delete_refuses_a_directory_that_is_not_a_save() {
    // Deleting a save became a recursive delete when it became a folder. A directory without a
    // career.json is not a save, and is not deleted just for sitting in the saves folder.
    let dir = tmp_dir("del_guard");
    fs::create_dir_all(dir.join("track_layouts")).unwrap();
    fs::write(dir.join("track_layouts").join("interlagos.json"), "[]").unwrap();

    assert!(delete_save(&dir, "track_layouts").is_err());
    assert!(dir.join("track_layouts").is_dir(), "left alone");
    assert!(delete_save(&dir, "never_existed").is_err());

    let _ = fs::remove_dir_all(&dir);
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
fn test_resolve_active_prefers_the_remembered_career_when_it_is_still_there() {
    let dir = tmp_dir("resolve_cfg");
    let gt3 = write_save(&dir, "gt3", CAREER_JSON);
    write_save(&dir, "ams2_career", CAREER_JSON);

    let picked = resolve_active(&dir, Some("gt3")).unwrap();
    assert_eq!(picked, gt3, "a remembered career wins over the default");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_remembers_a_career_in_either_layout() {
    let dir = tmp_dir("resolve_cfg_flat");
    let flat = write_legacy(&dir, "gt3", CAREER_JSON);
    write_save(&dir, "ams2_career", CAREER_JSON);

    assert_eq!(resolve_active(&dir, Some("gt3")), Some(flat));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_falls_back_to_default_when_the_remembered_career_is_gone() {
    // After the saves folder changes, the remembered name may name nothing in the new one.
    let dir = tmp_dir("resolve_stale");
    let default = write_save(&dir, "ams2_career", CAREER_JSON);

    assert_eq!(resolve_active(&dir, Some("no_such_career")), Some(default.clone()));
    assert_eq!(resolve_active(&dir, Some("  ")), Some(default));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_finds_a_legacy_default_save() {
    // Upgrading the app must not lose the career the user was on: ams2_career.json is still
    // the default save, flat layout and all.
    let dir = tmp_dir("resolve_legacy_default");
    write_legacy(&dir, "ams2_career", CAREER_JSON);
    write_save(&dir, "aaa_other", CAREER_JSON);

    assert_eq!(
        resolve_active(&dir, None),
        Some(legacy_save_path(&dir, "ams2_career")),
        "the default save wins over an alphabetically earlier one"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_picks_first_save_across_both_layouts() {
    let dir = tmp_dir("resolve_first");
    write_save(&dir, "zzz", CAREER_JSON);
    write_legacy(&dir, "alpha", CAREER_JSON);
    fs::write(dir.join("notes.txt"), "ignore").unwrap();

    assert_eq!(
        resolve_active(&dir, None),
        Some(legacy_save_path(&dir, "alpha")),
        "alphabetically first save, whichever layout it is in"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_active_invents_nothing_for_an_empty_folder() {
    // Naming a career is the user's decision, and so is its mode — which is permanent. An
    // invented save arrives `Unset`, so a first-time user was greeted by a prompt to settle a
    // question about a career they never asked for.
    let dir = tmp_dir("resolve_empty");
    assert_eq!(resolve_active(&dir, None), None);
    assert_eq!(resolve_active(&dir, Some("ams2_career")), None);
    assert!(
        !dir.join("ams2_career").exists(),
        "and nothing was created on disk"
    );

    // A folder holding only the shared track layouts still holds no careers.
    fs::create_dir_all(dir.join("track_layouts")).unwrap();
    fs::write(dir.join("track_layouts").join("interlagos.json"), "[]").unwrap();
    assert_eq!(resolve_active(&dir, None), None);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_list_saves_marks_a_save_it_cannot_read() {
    // A broken save stays listed: showing it as an empty career would be a lie, and hiding it
    // would leave the user wondering where their season went.
    let dir = tmp_dir("broken");
    write_save(&dir, "ams2_career", CAREER_JSON);
    write_save(&dir, "corrupt", "{ not json");

    let saves = list_saves(&dir, &save_path(&dir, "ams2_career"));
    let good = saves.iter().find(|s| s.name == "ams2_career").unwrap();
    let bad = saves.iter().find(|s| s.name == "corrupt").unwrap();
    assert!(good.error.is_none());
    assert!(bad.error.is_some(), "a damaged save must say so");
    assert_eq!((bad.sessions, bad.championships), (0, 0));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_list_saves_reads_a_save_with_a_byte_order_mark() {
    let dir = tmp_dir("bom");
    write_save(&dir, "ams2_career", &format!("\u{feff}{CAREER_JSON}"));
    let saves = list_saves(&dir, &save_path(&dir, "ams2_career"));
    assert!(saves[0].error.is_none(), "{:?}", saves[0].error);
    assert_eq!(saves[0].sessions, 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_delete_survives_something_holding_the_folder_for_a_moment() {
    // The bug this fixes, seen on a saves folder inside Google Drive: `remove_dir_all` empties
    // the career folder and is then denied the directory itself, because the sync client still
    // holds it. The career vanishes from the switcher and an empty folder stays behind.
    //
    // The holder is transient — a plain `rmdir` a second later succeeds — so retrying is the
    // fix. Here an open handle inside the folder stands in for the sync client, released while
    // the retries are still running.
    let dir = tmp_dir("del_held");
    let career = write_save(&dir, "held", CAREER_JSON);
    let pinned = write_nested_file(&career, "extra", "pinned.bin");

    // Opened *without* FILE_SHARE_DELETE, which is what makes the removal fail — a plain
    // `File::open` shares delete access and would not reproduce the bug at all.
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&pinned)
        .unwrap();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(80));
        drop(handle);
    });

    delete_save(&dir, "held").unwrap();
    assert!(
        !dir.join("held").exists(),
        "the folder goes too, not just what was in it"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_removing_a_folder_that_is_already_gone_is_not_a_failure() {
    // Something else may finish the job mid-retry — a sync client completing its own delete, or
    // the user clicking twice. The folder being absent is the outcome that was wanted, so it
    // must not come back as an error.
    let dir = tmp_dir("del_vanished");
    assert!(remove_dir_all_briefly(&dir.join("never_existed")).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_the_startup_sweep_clears_husks_and_leaves_everything_else() {
    // A delete that could not remove its own folder leaves an empty one behind, and only a
    // later process can clear it. Empty is the whole test for safety: a save always has its
    // career.json, and `track_layouts` has files, so neither is ever a candidate.
    let dir = tmp_dir("sweep_husks");
    fs::create_dir_all(dir.join("husk_one")).unwrap();
    fs::create_dir_all(dir.join("husk_two")).unwrap();
    write_save(&dir, "real_career", CAREER_JSON);
    fs::create_dir_all(dir.join("track_layouts")).unwrap();
    fs::write(dir.join("track_layouts").join("monza.json"), "[]").unwrap();
    write_legacy(&dir, "flat", CAREER_JSON);

    assert_eq!(sweep_empty_husks(&dir), 2);
    assert!(!dir.join("husk_one").exists());
    assert!(!dir.join("husk_two").exists());
    assert!(dir.join("real_career").is_dir(), "a save is never empty");
    assert!(dir.join("track_layouts").is_dir(), "shared layouts stay");
    assert!(
        legacy_save_path(&dir, "flat").is_file(),
        "files are not touched"
    );

    // Nothing left to do on the next start.
    assert_eq!(sweep_empty_husks(&dir), 0);
    let _ = fs::remove_dir_all(&dir);
}
