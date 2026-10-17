//! The application layer driven directly — no TCP, no JSON. The HTTP routes are covered end to
//! end in `src/tests/server.rs`; these pin that the rules hold for any front end calling `App`.

use super::*;
use crate::career::data_store::{CareerMode, ChampionshipStatus};
use std::sync::{Arc, Mutex, RwLock};

/// An `App` over a fresh folder-layout career in its own temp directory. Dropping the returned
/// path's parent cleans everything up.
fn test_app(tag: &str, mode: CareerMode) -> (App, PathBuf) {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ams2_service_{tag}_{ns}"));
    let career = root.join("career").join("career.json");
    std::fs::create_dir_all(career.parent().unwrap()).unwrap();
    let store = Arc::new(RwLock::new(CareerData {
        mode,
        ..CareerData::default()
    }));
    let app = App::new(
        store,
        Arc::new(RwLock::new(career)),
        root.clone(),
        root.join("track_layouts"),
        root.join("config.json"),
        Arc::new(Mutex::new(crate::game::spotter::SpotterConfig::default())),
    );
    (app, root)
}

fn season(name: &str) -> NewChampionship {
    NewChampionship {
        name: name.into(),
        ..Default::default()
    }
}

#[test]
fn test_mode_can_be_set_once_and_never_changed() {
    let (app, root) = test_app("mode", CareerMode::Unset);
    assert!(matches!(
        app.set_career_mode(CareerMode::Unset),
        Err(AppError::BadRequest(_))
    ));
    assert_eq!(
        app.set_career_mode(CareerMode::Multiplayer),
        Ok(CareerMode::Multiplayer)
    );
    assert!(matches!(
        app.set_career_mode(CareerMode::Singleplayer),
        Err(AppError::Conflict(_))
    ));
    // The answer reached the file, not only memory.
    let saved = crate::career::data_store::try_load_data(&app.active_path()).unwrap();
    assert_eq!(saved.mode, CareerMode::Multiplayer);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_singleplayer_season_needs_a_roster_and_a_calendar() {
    let (app, root) = test_app("sp_new", CareerMode::Singleplayer);
    assert!(matches!(
        app.create_championship(season("1978")),
        Err(AppError::BadRequest(_))
    ));
    let with_roster = NewChampionship {
        custom_ai_file: Some("F-Classic_Gen1.xml".into()),
        ..season("1978")
    };
    match app.create_championship(with_roster.clone()) {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("how many races"), "{msg}"),
        other => panic!("expected a refusal for the missing calendar, got {other:?}"),
    }
    let champ = app
        .create_championship(NewChampionship {
            planned_rounds: Some(10),
            ..with_roster
        })
        .unwrap();
    // A singleplayer season is the current one the moment it exists.
    assert_eq!(champ.status, ChampionshipStatus::Active);
    assert_eq!(app.data().championships.len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_singleplayer_runs_one_season_at_a_time() {
    let (app, root) = test_app("sp_one", CareerMode::Singleplayer);
    let new = |name: &str| NewChampionship {
        custom_ai_file: Some("F-Classic_Gen1.xml".into()),
        planned_rounds: Some(8),
        ..season(name)
    };
    let first = app.create_championship(new("1978")).unwrap();
    assert!(matches!(
        app.create_championship(new("1979")),
        Err(AppError::Conflict(_))
    ));
    app.patch_championship(
        &first.id,
        ChampionshipPatch {
            status: Some(ChampionshipStatus::Final),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(app.create_championship(new("1979")).is_ok());
    // And the finished one stays finished.
    assert!(matches!(
        app.patch_championship(
            &first.id,
            ChampionshipPatch {
                status: Some(ChampionshipStatus::Active),
                ..Default::default()
            },
        ),
        Err(AppError::Conflict(_))
    ));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_multiplayer_refuses_a_roster() {
    let (app, root) = test_app("mp_roster", CareerMode::Multiplayer);
    let refused = app.create_championship(NewChampionship {
        custom_ai_file: Some("F-Classic_Gen1.xml".into()),
        ..season("League")
    });
    assert!(matches!(refused, Err(AppError::Conflict(_))));
    assert!(app.create_championship(season("League")).is_ok());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_missing_things_are_not_found_rather_than_empty() {
    let (app, root) = test_app("missing", CareerMode::Singleplayer);
    assert!(matches!(app.offers("nope"), Err(AppError::NotFound(_))));
    assert!(matches!(app.grid_check("nope"), Err(AppError::NotFound(_))));
    assert!(matches!(app.add_round("nope"), Err(AppError::NotFound(_))));
    assert!(matches!(
        app.delete_championship("nope"),
        Err(AppError::NotFound(_))
    ));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_spotter_patch_changes_only_what_it_names_and_is_remembered() {
    let (app, root) = test_app("spotter", CareerMode::Unset);
    app.patch_spotter(SpotterPatch {
        enabled: Some(true),
        voice: Some(Some("Zira".into())),
        ..Default::default()
    });
    let status = app.patch_spotter(SpotterPatch {
        player: Some(Some("Senna".into())),
        ..Default::default()
    });
    assert_eq!(
        status,
        SpotterStatus {
            enabled: true,
            player: Some("Senna".into()),
            voice: Some("Zira".into()),
        }
    );
    // `Some(None)` clears, rather than leaving alone.
    let cleared = app.patch_spotter(SpotterPatch {
        voice: Some(None),
        ..Default::default()
    });
    assert_eq!(cleared.voice, None);
    let cfg = app.config();
    assert!(cfg.spotter_enabled);
    assert_eq!(cfg.spotter_name.as_deref(), Some("Senna"));
    assert_eq!(cfg.spotter_voice, None);
    std::fs::remove_dir_all(&root).ok();
}

// ── The live frame ────────────────────────────────────────────────────────────

fn driver(name: &str, player: bool, lap: f32, sectors: [f32; 3], in_pits: bool) -> crate::game::shared_memory::ParticipantData {
    crate::game::shared_memory::ParticipantData {
        name: name.into(),
        car_name: "F3".into(),
        car_class: String::new(),
        is_active: true,
        is_player: player,
        race_position: 1,
        laps_completed: 3,
        current_lap: 4,
        current_lap_distance: 0.0,
        cur_s1: -1.0,
        cur_s2: -1.0,
        cur_s3: -1.0,
        best_s1: sectors[0],
        best_s2: sectors[1],
        best_s3: sectors[2],
        fastest_lap_time: lap,
        last_lap_time: 0.0,
        world_pos_x: 0.0,
        world_pos_z: 0.0,
        interval_gap_secs: 0.0,
        interval_gap_laps: 0,
        in_pits,
    }
}

/// A connected practice session at Spa with the player on track at `ride` cm per corner.
fn live(ride: [f32; 4]) -> crate::game::shared_memory::LiveSessionData {
    let mut d = crate::game::shared_memory::disconnected();
    d.connected = true;
    d.game_state = 2;
    d.session_state = crate::career::data_store::SESSION_PRACTICE;
    d.track_location = "Spa".into();
    d.track_variation = "GP".into();
    d.car_name = "F3".into();
    d.participants = vec![driver("Me", true, 0.0, [0.0; 3], false)];
    d.num_participants = 1;
    d.player_telemetry.ride_height = ride;
    d
}

#[test]
fn test_session_bests_ignore_unset_times() {
    let mut d = live([0.0; 4]);
    d.participants = vec![
        driver("A", false, 101.0, [30.0, -1.0, 40.0], false),
        driver("B", false, 0.0, [31.0, 33.0, 0.0], false),
        driver("C", false, 99.5, [32.0, 34.0, 39.0], false),
    ];
    assert_eq!(
        session_bests(&d),
        SessionBests { lap: 99.5, s1: 30.0, s2: 33.0, s3: 39.0 }
    );
    d.participants.clear();
    assert_eq!(session_bests(&d), SessionBests::default());
}

#[test]
fn test_ride_record_keeps_each_corners_extremes_on_track_only() {
    let (app, root) = test_app("ride", CareerMode::Unset);
    assert_eq!(app.frame_of(live([0.0; 4])).ride_seen, None, "zero is unset, not a reading");
    app.frame_of(live([5.0, 5.5, 0.0, 7.0]));
    let f = app.frame_of(live([4.0, 6.0, 0.0, 7.5]));
    assert!(f.on_track);
    assert_eq!(
        f.ride_seen,
        Some(RideSeen { min: [4.0, 5.5, 0.0, 7.0], max: [5.0, 6.0, 0.0, 7.5] })
    );
    // In the pit lane: held, not folded in.
    let mut pitting = live([1.0; 4]);
    pitting.participants[0].in_pits = true;
    let f = app.frame_of(pitting);
    assert!(!f.on_track);
    assert_eq!(f.ride_seen.unwrap().min[0], 4.0);
    // Returned to the garage through the menu: pit_mode alone says so.
    let mut garage = live([1.0; 4]);
    garage.pit_mode = 4;
    assert_eq!(app.frame_of(garage).ride_seen.unwrap().min[0], 4.0);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_ride_record_starts_over_for_a_new_run_a_reset_or_a_lost_game() {
    let (app, root) = test_app("ride_run", CareerMode::Unset);
    app.frame_of(live([4.0; 4]));
    let mut other_car = live([6.0; 4]);
    other_car.car_name = "GT3".into();
    assert_eq!(app.frame_of(other_car).ride_seen.unwrap().min, [6.0; 4], "a new car is a new run");

    app.reset_ride_record();
    assert_eq!(app.frame_of(live([0.0; 4])).ride_seen, None);

    app.frame_of(live([4.0; 4]));
    assert_eq!(app.frame_of(crate::game::shared_memory::disconnected()).ride_seen, None);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_can_record_is_the_record_routes_own_test() {
    let (app, root) = test_app("can_record", CareerMode::Unset);
    assert!(app.frame_of(live([0.0; 4])).can_record);
    let mut menu = live([0.0; 4]);
    menu.session_state = 0;
    assert!(!app.frame_of(menu).can_record);
    let mut replay = live([0.0; 4]);
    replay.game_state = 6;
    assert!(!app.frame_of(replay).can_record, "a replay is not the session");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_championships_and_sessions_come_ready_to_list() {
    let (app, root) = test_app("listing", CareerMode::Multiplayer);
    for name in ["b league", "A league"] {
        app.create_championship(season(name)).unwrap();
    }
    let names: Vec<String> = app.championships().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["A league", "b league"]);
    std::fs::remove_dir_all(&root).ok();
}
