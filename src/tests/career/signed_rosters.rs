use super::*;
use crate::career::data_store::ChampionshipStatus;

fn tmp_dir(tag: &str) -> PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_rosters_test_{tag}_{ns}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn champ(id: &str, file: Option<&str>) -> Championship {
    Championship {
        id: id.into(),
        name: "Season".into(),
        status: ChampionshipStatus::Active,
        points_system: vec![10, 6, 4],
        manufacturer_scoring: false,
        rounds: vec![],
        session_ids: vec![],
        custom_ai_file: file.map(str::to_string),
        player_team: None,
        player_seat: None,
        planned_rounds: None,
    }
}

/// A folder save and a Custom AI folder holding one roster.
fn setup(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = tmp_dir(tag);
    let save = root.join("saves").join("career one");
    fs::create_dir_all(&save).unwrap();
    let career = save.join("career.json");
    fs::write(&career, "{}").unwrap();
    let ai = root.join("CustomAIDrivers");
    fs::create_dir_all(&ai).unwrap();
    fs::write(ai.join("F-Test.xml"), "<roster v=\"1\"/>").unwrap();
    (root, career, ai)
}

#[test]
fn test_without_a_copy_the_game_file_is_read() {
    let (root, career, ai) = setup("nocopy");
    let c = champ("17", Some("F-Test.xml"));
    assert_eq!(
        roster_file(&career, Some(&ai), &c),
        Some(ai.join("F-Test.xml"))
    );
    // Nothing stored, so nothing to compare — not a mismatch.
    assert_eq!(matches_game(&career, &ai, &c), None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_a_stored_copy_is_read_instead_of_the_game_file() {
    let (root, career, ai) = setup("copy");
    let c = champ("17", Some("F-Test.xml"));
    store(&career, "17", &ai.join("F-Test.xml")).unwrap();
    let copy = roster_file(&career, Some(&ai), &c).unwrap();
    assert!(copy.starts_with(career.parent().unwrap().join("rosters")));
    assert_eq!(matches_game(&career, &ai, &c), Some(true));

    // Editing the game's file leaves the season's roster where it was.
    fs::write(ai.join("F-Test.xml"), "<roster v=\"2\"/>").unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), "<roster v=\"1\"/>");
    assert_eq!(matches_game(&career, &ai, &c), Some(false));
    // A copy outlives the Custom AI folder setting, too.
    assert_eq!(roster_file(&career, None, &c), Some(copy));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_restore_puts_the_season_roster_back_and_keeps_the_baseline() {
    let (root, career, ai) = setup("restore");
    let c = champ("17", Some("F-Test.xml"));
    store(&career, "17", &ai.join("F-Test.xml")).unwrap();
    fs::write(ai.join("F-Test.xml"), "<roster v=\"2\"/>").unwrap();

    restore(&career, &ai, &c).unwrap();
    assert_eq!(
        fs::read_to_string(ai.join("F-Test.xml")).unwrap(),
        "<roster v=\"1\"/>"
    );
    assert_eq!(matches_game(&career, &ai, &c), Some(true));
    // The overwritten file had no baseline yet, so it got one before being replaced.
    assert!(crate::roster::custom_ai::has_baseline(&ai.join("F-Test.xml")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_a_deleted_game_file_is_a_mismatch_and_can_be_restored() {
    let (root, career, ai) = setup("deleted");
    let c = champ("17", Some("F-Test.xml"));
    store(&career, "17", &ai.join("F-Test.xml")).unwrap();
    fs::remove_file(ai.join("F-Test.xml")).unwrap();
    assert_eq!(matches_game(&career, &ai, &c), Some(false));
    restore(&career, &ai, &c).unwrap();
    assert_eq!(matches_game(&career, &ai, &c), Some(true));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_remove_forgets_the_copy() {
    let (root, career, ai) = setup("remove");
    let c = champ("17", Some("F-Test.xml"));
    store(&career, "17", &ai.join("F-Test.xml")).unwrap();
    remove(&career, "17");
    assert_eq!(
        roster_file(&career, Some(&ai), &c),
        Some(ai.join("F-Test.xml"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_a_flat_save_keeps_no_copy() {
    let (root, _, ai) = setup("flat");
    let flat = root.join("saves").join("old.json");
    fs::write(&flat, "{}").unwrap();
    assert!(store(&flat, "17", &ai.join("F-Test.xml")).is_err());
    let c = champ("17", Some("F-Test.xml"));
    assert_eq!(
        roster_file(&flat, Some(&ai), &c),
        Some(ai.join("F-Test.xml"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_an_id_that_is_not_a_filename_gets_no_copy() {
    let (root, career, _) = setup("badid");
    assert_eq!(copy_path(&career, "../escape"), None);
    let _ = fs::remove_dir_all(root);
}
