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

/// A driver whose best sectors are 25 / 30 / 35, with `cur` as this lap's.
fn timed(cur: [f32; 3]) -> crate::game::shared_memory::ParticipantData {
    let mut p = driver("A", false, 90.0, [25.0, 30.0, 35.0], false);
    (p.cur_s1, p.cur_s2, p.cur_s3) = (cur[0], cur[1], cur[2]);
    p
}

fn cells_of(participants: Vec<crate::game::shared_memory::ParticipantData>) -> Vec<[SectorCell; 3]> {
    let mut d = live([0.0; 4]);
    d.participants = participants;
    sector_cells(&d, &session_bests(&d))
}

fn cell(time: f32, mark: SectorMark) -> SectorCell {
    SectorCell { time, mark }
}

#[test]
fn test_a_sector_faster_than_the_drivers_best_is_a_partial_and_not_shown() {
    // Monza 1984 as AMS2 reported it: S1 01.737 for a car that left the pits mid-sector. It
    // never entered the best, so it is a fragment — showing it purple called it the session's
    // fastest. The driver's best stands in for it.
    let cells = cells_of(vec![timed([1.737, -1.0, -1.0])]);
    assert_eq!(cells[0][0], cell(25.0, SectorMark::Best));
}

#[test]
fn test_a_sector_spent_partly_in_the_garage_is_not_shown() {
    // 9:09.152: the sector the car sat out in the garage.
    let cells = cells_of(vec![timed([-1.0, 549.152, -1.0])]);
    assert_eq!(cells[0][1].time, 30.0);
}

#[test]
fn test_a_counted_sector_is_shown_and_coloured() {
    let cells = cells_of(vec![
        timed([25.0, 31.5, -1.0]),
        // A second driver with faster bests, so the first one's are only personal.
        {
            let mut p = driver("B", false, 89.0, [24.0, 29.0, 34.0], false);
            p.cur_s1 = 24.5;
            p
        },
    ]);
    // This lap's S1 equals the driver's own best: green. S2 is slower: plain. No S3 yet this
    // lap: the best stands, plain because the session's is faster.
    assert_eq!(
        cells[0],
        [
            cell(25.0, SectorMark::Personal),
            cell(31.5, SectorMark::Plain),
            cell(35.0, SectorMark::Plain),
        ]
    );
    // B's 24.5 is slower than B's own 24.0, which is the session's best.
    assert_eq!(cells[1][0], cell(24.5, SectorMark::Plain));
    assert_eq!(cells[1][1], cell(29.0, SectorMark::Best));
}

#[test]
fn test_a_sector_with_no_best_behind_it_is_not_shown() {
    // A first out-lap: a current time, but nothing the game counted yet.
    let mut p = driver("A", false, 0.0, [-1.0, -1.0, -1.0], false);
    p.cur_s1 = 12.0;
    let cells = cells_of(vec![p]);
    assert_eq!(cells[0], [SectorCell::default(); 3]);
}

#[test]
fn test_the_frame_carries_one_row_of_sectors_per_participant() {
    let (app, root) = test_app("sectors", CareerMode::Unset);
    let mut d = live([0.0; 4]);
    d.participants.push(timed([25.0, -1.0, -1.0]));
    let frame = app.frame_of(d);
    assert_eq!(frame.sectors.len(), frame.data.participants.len());
    std::fs::remove_dir_all(&root).ok();
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

// ── The HTML export ───────────────────────────────────────────────────────────

#[test]
fn test_the_export_carries_standings_rounds_and_the_lap_chart() {
    use crate::career::data_store::{LapChartEntry, RecordedSession, SessionResult};
    let (app, root) = test_app("export", CareerMode::Multiplayer);
    let champ = app.create_championship(season("Club <Cup> & Co")).unwrap();
    app.add_round(&champ.id).unwrap();
    let result = |name: &str, pos: u32| SessionResult {
        name: name.into(),
        car_name: "F3".into(),
        car_class: String::new(),
        race_position: pos,
        laps_completed: 2,
        fastest_lap: 90.0 + pos as f32,
        last_lap: 0.0,
        dnf: false,
        is_player: false,
    };
    app.store.write().unwrap().sessions.push(RecordedSession {
        id: "s1".into(),
        recorded_at: 1_709_210_096,
        track: "Spa".into(),
        track_variation: "GP".into(),
        car_name: "F3".into(),
        car_class: String::new(),
        session_type: crate::career::data_store::SESSION_RACE,
        results: vec![result("Bob", 2), result("Alice", 1)],
        // Inline: this test career's chart file does not exist, so the reader falls back to it.
        lap_chart: vec![
            LapChartEntry { lap: 1, driver: "Bob".into(), position: 1 },
            LapChartEntry { lap: 1, driver: "Alice".into(), position: 2 },
            LapChartEntry { lap: 2, driver: "Alice".into(), position: 1 },
        ],
    });
    app.add_session_to_round(&champ.id, 0, "s1").unwrap();

    let html = app.career_export_html();
    assert!(html.starts_with("<!DOCTYPE html>"));
    // Names are escaped, never pasted in as markup.
    assert!(html.contains("Club &lt;Cup&gt; &amp; Co"), "{html}");
    assert!(!html.contains("<Cup>"));
    // The round names its track and winner, and the standings lead with her.
    assert!(html.contains("R1 \u{2014} Spa &nbsp;&#127942; Alice"));
    assert!(html.contains("<td class=\"pos\">1</td><td>Alice"));
    // The lap chart is filled in, not an empty box: Alice leads lap 2.
    assert!(html.contains("Lap Chart"));
    assert!(html.contains("<td class=\"lap-p1\">1</td></tr>"), "{html}");
    assert!(app.career_export_file_name().starts_with("ams2_career_"));
    std::fs::remove_dir_all(&root).ok();
}

// ── The session picker and the offer sentences ────────────────────────────────

#[test]
fn test_the_picker_offers_only_sessions_no_round_holds() {
    use crate::career::data_store::RecordedSession;
    let (app, root) = test_app("picker", CareerMode::Multiplayer);
    let champ = app.create_championship(season("League")).unwrap();
    app.add_round(&champ.id).unwrap();
    for id in ["a", "b", "c"] {
        app.store.write().unwrap().sessions.push(RecordedSession {
            id: id.into(),
            recorded_at: 1,
            track: "Spa".into(),
            track_variation: String::new(),
            car_name: String::new(),
            car_class: String::new(),
            session_type: crate::career::data_store::SESSION_RACE,
            results: vec![],
            lap_chart: vec![],
        });
    }
    app.add_session_to_round(&champ.id, 0, "b").unwrap();
    let picker = app.available_sessions(&champ.id).unwrap();
    let ids: Vec<&str> = picker.sessions.iter().map(|s| s.session.id.as_str()).collect();
    assert_eq!(ids, ["a", "c"]);
    // Nothing to enforce without a roster and a team, so nothing is hidden.
    assert_eq!(picker.hidden, 0);
    assert_eq!(app.unassigned_sessions().len(), 2);
    assert!(matches!(app.available_sessions("nope"), Err(AppError::NotFound(_))));
    std::fs::remove_dir_all(&root).ok();
}

fn offer(kind: crate::career::contracts::OfferKind, required: f32) -> crate::career::contracts::Offer {
    crate::career::contracts::Offer {
        team: "Osella".into(),
        kind,
        renewal: false,
        salary: 50_000,
        objective: Some(12),
        rank: 8,
        buy_in: 0,
        required,
        expected_position: 17.6,
    }
}

#[test]
fn test_the_offer_sentence_names_the_rule_that_applied() {
    use super::signing::offer_why;
    use crate::career::contracts::{OfferKind, Standing};
    let none = Standing::default();

    let clear = offer_why(&offer(OfferKind::Paid, 40.0), 52.0, 21, &none, 0);
    assert!(clear.why.starts_with("9th-fastest of 21 teams · its car should finish around P18"), "{}", clear.why);
    assert!(clear.why.contains("wants 40 and you have 52 — clear of their bar"));
    assert!(clear.affordable);

    // A team asking more than the driver has, still offering: the sentence must say why.
    let reach = offer_why(&offer(OfferKind::Paid, 57.0), 50.0, 21, &none, 0);
    assert!(reach.why.contains("short of their bar but within reach"), "{}", reach.why);

    // A back-marker charging a driver who clears its bar, and a price the career cannot pay.
    let mut bought = offer(OfferKind::Pay, 49.0);
    bought.buy_in = 80_000;
    bought.objective = None;
    let note = offer_why(&bought, 50.0, 21, &none, 10_000);
    assert!(note.why.contains("you clear their bar of 49, but not by enough"), "{}", note.why);
    assert!(note.why.contains("too far back for a finishing target"));
    assert!(note.why.ends_with("more than the career is worth"));
    assert!(!note.affordable);

    let mut renewal = offer(OfferKind::Paid, 90.0);
    renewal.renewal = true;
    let delivered = Standing { delivered: Some(true), ..Standing::default() };
    let note = offer_why(&renewal, 50.0, 21, &delivered, 0);
    assert!(note.why.contains("your own team: they re-sign you whatever the rating says, and pay more"));
}

#[test]
fn test_the_offers_summary_says_when_the_last_seat_was_another_series() {
    use super::signing::offers_summary;
    use crate::career::contracts::Standing;
    let held = Standing {
        incumbent: Some("Brabham".into()),
        class: "F-Classic_Gen1".into(),
        delivered: Some(true),
        tenure: 1,
        mood: None,
    };
    assert_eq!(
        offers_summary(51.6, 1_500_000, &held, "F-Classic_Gen1", None, None),
        "Rating 52/100 · Balance 1,500,000 · Last seat: Brabham (target met)"
    );
    assert_eq!(
        offers_summary(51.6, 0, &held, "F-Retro_Gen1", None, None),
        "Rating 52/100 · Balance 0 · Last seat: Brabham (F-Classic_Gen1 — different series, no renewal)"
    );
}

#[test]
fn test_a_config_saved_unchanged_through_the_gui_patch_comes_back_the_same() {
    let (app, root) = test_app("config_patch", CareerMode::Unset);
    let mut cfg = app.config_view().cfg;
    cfg.starting_rating = 42.0;
    cfg.custom_ai_dir = Some("   ".into()); // A blank box is "not set", not a folder named spaces.
    let saved = app
        .patch_config(ConfigPatch::from_config(&cfg, Default::default()))
        .unwrap()
        .config;
    assert_eq!(saved.starting_rating, 42.0);
    assert_eq!(saved.custom_ai_dir, None);
    // Saved again untouched, nothing moves — the patch says exactly what the config held.
    let again = app
        .patch_config(ConfigPatch::from_config(&saved, Default::default()))
        .unwrap()
        .config;
    assert_eq!(
        serde_json::to_value(&saved).unwrap(),
        serde_json::to_value(&again).unwrap()
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_live_frame_says_the_career_is_not_saved_until_a_save_succeeds() {
    // The recorder fails with nobody asking, usually just as AMS2 disconnects, so the frame
    // carries the reason whatever the game is doing — and saving by hand is what clears it.
    let (app, root) = test_app("unsaved", CareerMode::Unset);
    let career = app.active_path();
    std::fs::write(&career, "{ damaged").unwrap();
    app.adopt_rating().expect_err("the damaged file must not be written over");
    let frame = app.frame_of(crate::game::shared_memory::disconnected());
    assert!(
        frame.save_error.as_deref().is_some_and(|e| e.contains("refusing to overwrite")),
        "{:?}",
        frame.save_error
    );

    app.save_career().expect_err("still damaged, still refused");
    std::fs::remove_file(&career).unwrap();
    app.save_career().expect("nothing is in the way any more");
    assert_eq!(app.frame_of(crate::game::shared_memory::disconnected()).save_error, None);
    let saved = crate::career::data_store::try_load_data(&career).unwrap();
    assert!(saved.rating_params.is_some(), "the change made while unsaved reached the file");
    std::fs::remove_dir_all(&root).ok();
}

// ── A session recorded while the active career changes ────────────────────────

/// A folder save at `<root>/<name>/career.json` holding one championship called `season`.
fn career_on_disk(root: &Path, name: &str, season: &str) -> PathBuf {
    let file = root.join(name).join("career.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let json = format!(
        r#"{{"sessions":[],"championships":[{{"id":"{season}","name":"{season}","points_system":[25],"rounds":[]}}]}}"#
    );
    std::fs::write(&file, json).unwrap();
    file
}

fn season_names(data: &CareerData) -> Vec<String> {
    data.championships.iter().map(|c| c.name.clone()).collect()
}

/// The recorder filing a session with a lap chart, on a thread of its own.
fn capture_on_a_thread(app: &App) -> std::thread::JoinHandle<Result<(), String>> {
    let app = app.clone();
    std::thread::spawn(move || {
        let chart = vec![crate::career::data_store::LapChartEntry {
            lap: 1,
            driver: "Me".into(),
            position: 1,
        }];
        crate::game::recorder::capture(&app.store, &app.data_path, &live([0.0; 4]), chart, None)
    })
}

#[test]
fn test_a_session_filed_while_switching_careers_lands_whole_in_one_of_them() {
    // Both orders are legitimate: filed into A before the switch, or into B after it. What must
    // never happen is a mix — B written into A's file, or the session in one career with its
    // chart beside the other. The test holds the save lock while both start, so they are
    // waiting on it together when it is let go.
    let (app, root) = test_app("switch_race", CareerMode::Unset);
    let a = career_on_disk(&root, "career", "A");
    assert_eq!(a, app.active_path());
    *app.store.write().unwrap() = crate::career::data_store::try_load_data(&a).unwrap();
    let b = career_on_disk(&root, "b", "B");

    let held = crate::career::data_store::saving();
    let recorder = capture_on_a_thread(&app);
    let switch = {
        let app = app.clone();
        std::thread::spawn(move || app.activate_save("b").map(|_| ()))
    };
    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(held);
    recorder.join().unwrap().expect("the session is saved");
    switch.join().unwrap().expect("the switch goes through");

    let in_a = crate::career::data_store::try_load_data(&a).unwrap();
    let in_b = crate::career::data_store::try_load_data(&b).unwrap();
    assert_eq!(season_names(&in_a), ["A"], "A's file holds A");
    assert_eq!(season_names(&in_b), ["B"], "B's file holds B");
    let (holder, folder) = match (in_a.sessions.len(), in_b.sessions.len()) {
        (1, 0) => (&in_a, a.parent().unwrap()),
        (0, 1) => (&in_b, b.parent().unwrap()),
        counts => panic!("the session must be in exactly one career: {counts:?}"),
    };
    let chart = folder.join("laps").join(format!("{}.json", holder.sessions[0].id));
    assert!(chart.exists(), "the chart sits beside the career holding the session");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_a_session_filed_while_the_active_career_is_renamed_follows_it() {
    // A capture that read the old path and wrote after the move would recreate the old folder —
    // writing a lap chart creates its own — and save a second copy of the career there.
    let (app, root) = test_app("rename_race", CareerMode::Unset);
    let a = career_on_disk(&root, "career", "A");
    *app.store.write().unwrap() = crate::career::data_store::try_load_data(&a).unwrap();

    let held = crate::career::data_store::saving();
    let recorder = capture_on_a_thread(&app);
    let rename = {
        let app = app.clone();
        std::thread::spawn(move || app.rename_save("career", "renamed").map(|_| ()))
    };
    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(held);
    recorder.join().unwrap().expect("the session is saved");
    rename.join().unwrap().expect("the rename goes through");

    assert!(!root.join("career").exists(), "the old folder is not brought back");
    let renamed = root.join("renamed").join("career.json");
    assert_eq!(app.active_path(), renamed);
    let saved = crate::career::data_store::try_load_data(&renamed).unwrap();
    assert_eq!(season_names(&saved), ["A"]);
    assert_eq!(saved.sessions.len(), 1, "the session followed the career");
    let chart = root
        .join("renamed")
        .join("laps")
        .join(format!("{}.json", saved.sessions[0].id));
    assert!(chart.exists(), "and so did its chart");
    std::fs::remove_dir_all(&root).ok();
}

// ── The season's story ───────────────────────────────────────────────────────────────────────

/// Williams and Osella, two cars each. The file spells "Allan Berg"; telemetry says "Allen".
const STORY_ROSTER: &str = r##"<custom_ai_drivers>
    <driver livery_name="1986 Williams #5 - N. Mansell"><name>Nigel Mansell</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Williams #6 - N. Piquet"><name>Nelson Piquet</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Osella #21 - P. Ghinzani"><name>Piercarlo Ghinzani</name><race_skill>0.55</race_skill></driver>
    <driver livery_name="1986 Osella #22 - A. Berg"><name>Allan Berg</name><race_skill>0.54</race_skill></driver>
    <driver livery_name="1986 Zakspeed #14 - J. Palmer"><name>Jonathan Palmer</name><race_skill>0.60</race_skill></driver>
</custom_ai_drivers>"##;

/// An app whose config points at a Custom AI folder holding [`STORY_ROSTER`], with one season on
/// it driven for `team`: a qualifying and a race against the Osellas.
fn story_app(tag: &str, mode: CareerMode, team: Option<&str>) -> (App, PathBuf) {
    use crate::career::data_store::{RecordedSession, Round, SessionResult};
    let (app, root) = test_app(tag, mode);
    let ai_dir = root.join("CustomAIDrivers");
    std::fs::create_dir_all(&ai_dir).unwrap();
    std::fs::write(ai_dir.join("F-Classic_Gen1.xml"), STORY_ROSTER).unwrap();
    std::fs::write(
        &app.config_path,
        format!(
            "{{\"custom_ai_dir\":{}}}",
            serde_json::to_string(&ai_dir.display().to_string()).unwrap()
        ),
    )
    .unwrap();
    let result = |name: &str, pos: u32, is_player: bool| SessionResult {
        name: name.into(),
        car_name: String::new(),
        car_class: "F-Classic_Gen1".into(),
        race_position: pos,
        laps_completed: 10,
        fastest_lap: 90.0,
        last_lap: 90.0,
        dnf: false,
        is_player,
    };
    let session = |id: &str, session_type: u32, results: Vec<SessionResult>| RecordedSession {
        id: id.into(),
        recorded_at: 1,
        track: "Monza".into(),
        track_variation: String::new(),
        car_name: String::new(),
        car_class: "F-Classic_Gen1".into(),
        session_type,
        results,
        lap_chart: vec![],
    };
    {
        let mut data = app.store.write().unwrap();
        data.sessions = vec![
            session("q1", 3, vec![result("Allen Berg", 1, false), result("Nightrat", 2, true)]),
            session(
                "r1",
                5,
                vec![
                    result("Nightrat", 1, true),
                    result("Allen Berg", 2, false),
                    result("Piercarlo Ghinzani", 3, false),
                ],
            ),
        ];
        data.championships.push(Championship {
            id: "c1".into(),
            name: "1986".into(),
            status: ChampionshipStatus::Active,
            points_system: vec![9, 6, 4],
            manufacturer_scoring: false,
            rounds: vec![Round {
                session_ids: vec!["q1".into(), "r1".into()],
            }],
            session_ids: vec![],
            custom_ai_file: (mode != CareerMode::Multiplayer).then(|| "F-Classic_Gen1.xml".into()),
            player_team: team.map(String::from),
            player_seat: Some(crate::career::data_store::SeatLock {
                seat: format!("{} car", team.unwrap_or_default()),
                replaces: if team == Some("Zakspeed") { "Jonathan Palmer" } else { "Piercarlo Ghinzani" }.into(),
            }),
            planned_rounds: Some(10),
            team_lineage: Vec::new(),
        });
    }
    (app, root)
}

#[test]
fn test_the_story_measures_the_player_against_the_teams_other_car() {
    let (app, root) = story_app("story_h2h", CareerMode::Singleplayer, Some("Osella"));
    let t = app.season_story("c1").unwrap().teammates;

    assert_eq!(t.reason, None);
    assert_eq!(t.team.as_deref(), Some("Osella"));
    // Ghinzani's car is the player's, so Berg is the only teammate — found despite the spelling.
    assert_eq!(t.head_to_head.len(), 1, "{t:?}");
    let h = &t.head_to_head[0];
    assert_eq!(h.teammate, "Allan Berg");
    assert_eq!((h.qualifying.player, h.qualifying.teammate), (0, 1));
    assert_eq!((h.race.player, h.race.teammate), (1, 0));
    assert_eq!((h.points.player, h.points.teammate), (9, 6));
    assert_eq!(
        t.summary.as_deref(),
        Some("Against Allan Berg: races 1–0, qualifying 0–1")
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_story_says_why_there_is_no_teammate() {
    let cases = [
        ("story_mp", CareerMode::Multiplayer, None, "multiplayer"),
        ("story_noteam", CareerMode::Singleplayer, None, "No team yet"),
        ("story_onecar", CareerMode::Singleplayer, Some("Zakspeed"), "Zakspeed runs no other car"),
    ];
    for (tag, mode, team, expected) in cases {
        let (app, root) = story_app(tag, mode, team);
        let t = app.season_story("c1").unwrap().teammates;
        assert!(t.head_to_head.is_empty(), "{tag}: {t:?}");
        assert!(t.summary.is_none(), "{tag}");
        let reason = t.reason.unwrap_or_default();
        assert!(reason.contains(expected), "{tag}: {reason}");
        std::fs::remove_dir_all(&root).ok();
    }
}

#[test]
fn test_the_story_of_an_unknown_season_is_not_found() {
    let (app, root) = test_app("story_404", CareerMode::Singleplayer);
    assert!(matches!(app.season_story("nope"), Err(AppError::NotFound(_))));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_story_carries_the_news_newest_round_first() {
    let (app, root) = story_app("story_news", CareerMode::Singleplayer, Some("Osella"));
    let news = app.season_story("c1").unwrap().news;
    assert_eq!(news.reason, None);
    assert_eq!(news.rounds.len(), 1);
    assert_eq!(news.rounds[0].track, "Monza");
    let keys: Vec<&str> = news.rounds[0].headlines.iter().map(|h| h.key).collect();
    assert!(keys.contains(&"win.maiden"), "{keys:?}");
    assert!(news.rounds[0].headlines.iter().all(|h| !h.text.is_empty()));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_news_works_in_multiplayer_where_there_is_no_teammate() {
    let (app, root) = story_app("story_news_mp", CareerMode::Multiplayer, None);
    let story = app.season_story("c1").unwrap();
    assert!(story.teammates.reason.is_some());
    assert!(!story.news.rounds.is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_an_earlier_season_makes_a_win_no_longer_a_maiden() {
    use crate::career::data_store::{RecordedSession, Round, SessionResult};
    let (app, root) = story_app("story_history", CareerMode::Singleplayer, Some("Osella"));
    {
        let mut data = app.store.write().unwrap();
        // A season won before this one was recorded.
        data.sessions.push(RecordedSession {
            id: "old".into(),
            recorded_at: 0,
            track: "Imola".into(),
            track_variation: String::new(),
            car_name: String::new(),
            car_class: String::new(),
            session_type: 5,
            results: vec![SessionResult {
                name: "Nightrat".into(),
                car_name: String::new(),
                car_class: String::new(),
                race_position: 1,
                laps_completed: 10,
                fastest_lap: 90.0,
                last_lap: 90.0,
                dnf: false,
                is_player: true,
            }],
            lap_chart: vec![],
        });
        let mut old = data.championships[0].clone();
        old.id = "c0".into();
        old.rounds = vec![Round { session_ids: vec!["old".into()] }];
        data.championships.push(old);
    }
    let news = app.season_story("c1").unwrap().news;
    let keys: Vec<&str> = news.rounds[0].headlines.iter().map(|h| h.key).collect();
    assert!(keys.contains(&"win") && !keys.contains(&"win.maiden"), "{keys:?}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_taking_a_session_out_of_a_season_takes_its_news_too() {
    let (app, root) = story_app("story_news_removed", CareerMode::Singleplayer, Some("Osella"));
    app.store.write().unwrap().championships[0].rounds[0].session_ids.clear();
    let news = app.season_story("c1").unwrap().news;
    assert!(news.rounds.is_empty());
    assert!(news.reason.is_some());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_title_line_says_where_the_player_stands_on_the_calendar() {
    let (app, root) = story_app("story_title", CareerMode::Singleplayer, Some("Osella"));
    let title = app.season_story("c1").unwrap().title;
    assert_eq!(title.reason, None);
    // The player won the only race: 9 against Berg's 6.
    assert_eq!(title.summary.as_deref(), Some("1 of 10 races run — leading by 3 points"));
    assert!(title.clinch.is_none(), "nine races left");
    assert!(title.objective.is_none(), "no contract");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_title_reads_the_objective_off_the_contract() {
    let (app, root) = story_app("story_objective", CareerMode::Singleplayer, Some("Osella"));
    app.store.write().unwrap().contracts.push(
        serde_json::from_value(serde_json::json!({
            "champ_id": "c1", "team": "Osella", "salary": 50000, "objective": 1
        }))
        .unwrap(),
    );
    let title = app.season_story("c1").unwrap().title;
    assert_eq!(
        title.objective.as_deref(),
        Some("The team wants 1st or better. You are 1st: on target.")
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_no_calendar_no_title_fight_and_it_says_why() {
    let (app, root) = story_app("story_nocal", CareerMode::Multiplayer, None);
    app.store.write().unwrap().championships[0].planned_rounds = None;
    let title = app.season_story("c1").unwrap().title;
    assert!(title.fight.is_none() && title.summary.is_none());
    assert!(title.reason.unwrap().contains("Set Races"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_a_season_past_its_calendar_is_told_to_finish_unless_it_is_final() {
    let (app, root) = story_app("story_past_plan", CareerMode::Singleplayer, Some("Osella"));
    app.store.write().unwrap().championships[0].planned_rounds = Some(1);
    assert!(app.season_story("c1").unwrap().title.past_plan.is_some());
    app.store.write().unwrap().championships[0].status = ChampionshipStatus::Final;
    assert!(app.season_story("c1").unwrap().title.past_plan.is_none());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_rival_needs_two_races_and_then_comes_with_a_sentence() {
    let (app, root) = story_app("story_rival", CareerMode::Singleplayer, Some("Osella"));
    let rival = app.season_story("c1").unwrap().rival;
    assert!(rival.rival.is_none());
    assert!(rival.reason.is_some());

    {
        let mut data = app.store.write().unwrap();
        let mut again = data.sessions[1].clone();
        again.id = "r2".into();
        again.recorded_at = 2;
        data.sessions.push(again);
        data.championships[0]
            .rounds
            .push(crate::career::data_store::Round { session_ids: vec!["r2".into()] });
    }
    let rival = app.season_story("c1").unwrap().rival;
    // Berg is the player's teammate and never the rival, so it is Ghinzani's car — the
    // seat the player took — whose driver still raced in this fixture.
    let r = rival.rival.expect("two races in");
    assert_eq!(r.name, "Piercarlo Ghinzani");
    assert_eq!(r.since_round, 2);
    assert_eq!(rival.points_ahead, 18 - 8);
    assert_eq!(
        rival.summary.as_deref(),
        Some("Your rival since round 2. This season you have finished ahead of them twice, they ahead of you 0 times; you are 10 points ahead in the championship.")
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_targets_judge_each_raced_round_and_set_the_next() {
    let (app, root) = story_app("story_targets", CareerMode::Singleplayer, Some("Osella"));
    let t = app.season_story("c1").unwrap().targets;
    assert_eq!(t.reason, None);
    assert_eq!(t.judged.len(), 1);
    assert_eq!(t.judged[0].round, 1);
    assert_eq!(t.judged[0].track, "Monza");
    assert!(t.record.as_deref().unwrap().starts_with("Targets met: "), "{t:?}");
    assert!(t.next.as_deref().unwrap().starts_with("The team's target for round 2: "), "{t:?}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_targets_say_why_there_are_none() {
    let (app, root) = story_app("story_targets_mp", CareerMode::Multiplayer, None);
    assert!(app.season_story("c1").unwrap().targets.reason.unwrap().contains("multiplayer"));
    std::fs::remove_dir_all(&root).ok();
    let (app, root) = story_app("story_targets_noteam", CareerMode::Singleplayer, None);
    assert!(app.season_story("c1").unwrap().targets.reason.unwrap().contains("No team yet"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_mood_says_where_the_team_stands_and_why_it_moved() {
    let (app, root) = story_app("story_mood", CareerMode::Singleplayer, Some("Osella"));
    let m = app.season_story("c1").unwrap().mood;
    assert_eq!(m.reason, None);
    // The player won, ahead of the teammate Berg: the mood can only have gone up.
    assert!(m.mood.unwrap() > 60, "{m:?}");
    assert_eq!(m.trend.len(), 1);
    let why = m.why.unwrap();
    assert!(why.starts_with("Up ") && why.contains("finished ahead of your teammate"), "{why}");
    assert!(m.says.unwrap().contains("Osella"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_mood_says_why_there_is_none() {
    let (app, root) = story_app("story_mood_mp", CareerMode::Multiplayer, None);
    assert!(app.season_story("c1").unwrap().mood.reason.unwrap().contains("multiplayer"));
    std::fs::remove_dir_all(&root).ok();
}

// ── Team lineage ─────────────────────────────────────────────────────────────────────────────

/// A singleplayer career that finished 1983 for Spirit-Honda (the season keeps its own copy of the
/// shipped 1983 roster) and has just created 1984 on the shipped 1984 roster, now in the game's
/// folder — the real hand-over the lineage exists for.
fn lineage_app(tag: &str) -> (App, PathBuf) {
    let (app, root) = test_app(tag, CareerMode::Singleplayer);
    let shipped = |season: &str| {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("custom_ai_files_with_perf_scalars")
                .join(season)
                .join("F-Retro_Gen3.xml"),
        )
        .unwrap()
    };
    let ai_dir = root.join("CustomAIDrivers");
    std::fs::create_dir_all(&ai_dir).unwrap();
    std::fs::write(ai_dir.join("F-Retro_Gen3.xml"), shipped("F1_1984")).unwrap();
    let rosters = app.active_path().parent().unwrap().join("rosters");
    std::fs::create_dir_all(&rosters).unwrap();
    std::fs::write(rosters.join("c83.xml"), shipped("F1_1983")).unwrap();
    std::fs::write(
        &app.config_path,
        format!("{{\"custom_ai_dir\":{}}}", serde_json::to_string(&ai_dir.display().to_string()).unwrap()),
    )
    .unwrap();
    let season = |id: &str, name: &str, status: ChampionshipStatus, team: Option<&str>| Championship {
        id: id.into(),
        name: name.into(),
        status,
        points_system: vec![9, 6, 4, 3, 2, 1],
        manufacturer_scoring: false,
        rounds: vec![],
        session_ids: vec![],
        custom_ai_file: Some("F-Retro_Gen3.xml".into()),
        player_team: team.map(String::from),
        player_seat: None,
        planned_rounds: Some(15),
        team_lineage: Vec::new(),
    };
    {
        let mut data = app.store.write().unwrap();
        data.championships.push(season("c83", "1983", ChampionshipStatus::Final, Some("Spirit-Honda")));
        data.championships.push(season("c84", "1984", ChampionshipStatus::Active, None));
        data.contracts.push(
            serde_json::from_value(serde_json::json!({
                "champ_id": "c83", "team": "Spirit-Honda", "salary": 50000
            }))
            .unwrap(),
        );
    }
    (app, root)
}

#[test]
fn test_lineage_proposes_spirit_hart_for_spirit_honda_and_leaves_lotus_open() {
    let (app, root) = lineage_app("lineage_view");
    let v = app.lineage("c84").unwrap();
    assert_eq!(v.reason, None);
    assert_eq!(v.previous.as_ref().map(|p| p.name.as_str()), Some("1983"));
    assert_eq!(v.your_team.as_deref(), Some("Spirit-Honda"));
    assert!(!v.locked);
    let link = |from: &str| v.links.iter().find(|l| l.from == from).unwrap();
    assert_eq!(link("Spirit-Honda").to.as_deref(), Some("Spirit-Hart"));
    assert_eq!(link("Spirit-Honda").source, crate::career::lineage::Source::Proposed);
    assert_eq!(link("Lotus-Ford").source, crate::career::lineage::Source::Unmatched);
    assert!(v.teams.contains(&"Spirit-Hart".to_string()));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_a_linked_team_renews_and_an_unlinked_one_does_not() {
    let (app, root) = lineage_app("lineage_renew");
    let offers = app.offers("c84").unwrap();
    let spirit = offers.offers.iter().find(|o| o.team == "Spirit-Hart").expect("Spirit-Hart makes an offer");
    assert!(spirit.renewal, "the team that continues Spirit-Honda renews the seat");
    assert!(offers.summary.contains("Last seat: Spirit-Honda, now Spirit-Hart"), "{}", offers.summary);

    app.set_lineage(
        "c84",
        LineagePatch { links: vec![crate::career::lineage::Link { from: "Spirit-Honda".into(), to: None }] },
    )
    .unwrap();
    let offers = app.offers("c84").unwrap();
    assert!(offers.offers.iter().all(|o| !o.renewal), "nobody continues the seat now");
    assert!(offers.summary.contains("no team on this grid continues"), "{}", offers.summary);
    // The choice reached the file.
    let saved = crate::career::data_store::try_load_data(&app.active_path()).unwrap();
    assert_eq!(saved.championships[1].team_lineage.len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_lineage_refuses_teams_that_are_not_there_and_changes_after_signing() {
    let (app, root) = lineage_app("lineage_refuse");
    let link = |from: &str, to: Option<&str>| crate::career::lineage::Link { from: from.into(), to: to.map(String::from) };
    assert!(matches!(
        app.set_lineage("c84", LineagePatch { links: vec![link("Spirit-Honda", Some("Minardi"))] }),
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        app.set_lineage("c84", LineagePatch { links: vec![link("Brawn", Some("Spirit-Hart"))] }),
        Err(AppError::BadRequest(_))
    ));
    app.store.write().unwrap().contracts.push(
        serde_json::from_value(serde_json::json!({ "champ_id": "c84", "team": "Spirit-Hart", "salary": 1 })).unwrap(),
    );
    assert!(app.lineage("c84").unwrap().locked);
    assert!(matches!(
        app.set_lineage("c84", LineagePatch { links: vec![link("Spirit-Honda", None)] }),
        Err(AppError::Conflict(_))
    ));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_lineage_needs_a_finished_season_in_the_same_series() {
    let (app, root) = lineage_app("lineage_none");
    app.store.write().unwrap().championships[0].status = ChampionshipStatus::Active;
    assert!(app.lineage("c84").unwrap().reason.is_some());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_how_the_team_ended_last_season_reaches_the_renewal() {
    use crate::career::data_store::{RecordedSession, Round, SessionResult};
    let (app, root) = lineage_app("lineage_mood");
    {
        // Four races in 1983, the player winning each in the slowest car: the team ends delighted.
        let mut data = app.store.write().unwrap();
        let mut rounds = Vec::new();
        for i in 0..4u64 {
            let mut results = vec![SessionResult {
                name: "Nightrat".into(),
                car_name: String::new(),
                car_class: String::new(),
                race_position: 1,
                laps_completed: 10,
                fastest_lap: 90.0,
                last_lap: 90.0,
                dnf: false,
                is_player: true,
            }];
            let me = results[0].clone();
            results.extend((2..=24).map(|p| SessionResult {
                name: format!("Driver {p}"),
                race_position: p,
                is_player: false,
                ..me.clone()
            }));
            let id = format!("s{i}");
            data.sessions.push(RecordedSession {
                id: id.clone(),
                recorded_at: i + 1,
                track: "Monza".into(),
                track_variation: String::new(),
                car_name: String::new(),
                car_class: String::new(),
                session_type: 5,
                results,
                lap_chart: vec![],
            });
            rounds.push(Round { session_ids: vec![id] });
        }
        data.championships[0].rounds = rounds;
    }
    let offers = app.offers("c84").unwrap();
    assert!(offers.summary.contains("the team ended it delighted with you"), "{}", offers.summary);
    let why = &offers.notes["Spirit-Hart"].why;
    assert!(why.contains("delighted"), "{why}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_interest_starts_two_thirds_of_the_way_through_and_leaves_out_your_team() {
    let (app, root) = story_app("story_interest", CareerMode::Singleplayer, Some("Osella"));
    // One race of ten: two thirds is seven races.
    let i = app.season_story("c1").unwrap().interest;
    assert!(i.teams.is_empty());
    assert!(i.reason.unwrap().contains("6 more races"));

    app.store.write().unwrap().championships[0].planned_rounds = Some(2);
    assert!(app.season_story("c1").unwrap().interest.reason.unwrap().contains("1 more race."));

    app.store.write().unwrap().championships[0].planned_rounds = Some(1);
    let i = app.season_story("c1").unwrap().interest;
    assert!(i.note.unwrap().contains("this season's grid"));
    assert!(i.teams.iter().all(|t| t.team != "Osella"), "{:?}", i.teams);
    assert!(!i.teams.is_empty() || i.reason.is_some());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_interest_is_a_singleplayer_market() {
    let (app, root) = story_app("story_interest_mp", CareerMode::Multiplayer, None);
    assert!(app.season_story("c1").unwrap().interest.reason.unwrap().contains("singleplayer"));
    std::fs::remove_dir_all(&root).ok();
}

// ── Season review and career biography ───────────────────────────────────────────────────────

#[test]
fn test_a_finished_season_gets_a_review_and_an_open_one_says_when() {
    let (app, root) = story_app("story_review", CareerMode::Singleplayer, Some("Osella"));
    let open = app.season_story("c1").unwrap().review;
    assert!(open.lines.is_empty() && open.reason.is_some());

    app.store.write().unwrap().championships[0].status = ChampionshipStatus::Final;
    let r = app.season_story("c1").unwrap().review;
    assert_eq!(r.reason, None);
    // The player won the only race, so they are the champion.
    assert_eq!(r.lines[0], "You won the championship with 9 points!");
    assert!(r.lines.iter().any(|l| l.starts_with("Best weekend: 1st at Monza")), "{:?}", r.lines);
    assert!(!r.headlines.is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_career_is_told_season_by_season_with_its_firsts() {
    let (app, root) = story_app("story_career", CareerMode::Singleplayer, Some("Osella"));
    let c = app.career_story();
    assert_eq!(c.seasons.len(), 1);
    assert_eq!(c.seasons[0].line, "Osella · 1st of 3 · 9 points · 1 win");
    assert!(c.milestones.iter().any(|m| m.starts_with("First win: Monza, round 1 of 1986")), "{:?}", c.milestones);
    assert_eq!(c.latest_finished, None);

    app.store.write().unwrap().championships[0].status = ChampionshipStatus::Final;
    let c = app.career_story();
    assert_eq!(c.seasons[0].line, "Osella · Champion · 9 points · 1 win");
    assert_eq!(c.latest_finished.as_deref(), Some("c1"));
    assert!(c.milestones.iter().any(|m| m == "First title: 1986."));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_the_export_carries_the_career_and_the_review_of_a_finished_season() {
    let (app, root) = story_app("story_export", CareerMode::Singleplayer, Some("Osella"));
    app.store.write().unwrap().championships[0].status = ChampionshipStatus::Final;
    let html = app.career_export_html();
    assert!(html.contains("<h2>Career</h2>"));
    assert!(html.contains("Osella · Champion"));
    assert!(html.contains("<h3>Season review</h3>"));
    assert!(html.contains("You won the championship with 9 points!"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_live_player_is_the_careers_driver_not_the_car_the_camera_is_on() {
    use crate::career::data_store::{RecordedSession, SessionResult};
    use crate::service::live::live_player_index;
    let result = |name: &str, is_player: bool| SessionResult {
        name: name.into(),
        car_name: String::new(),
        car_class: "F-Retro_Gen3".into(),
        race_position: 1,
        laps_completed: 10,
        fastest_lap: 90.0,
        last_lap: 90.0,
        dnf: false,
        is_player,
    };
    let session = |results: Vec<SessionResult>| RecordedSession {
        id: "s".into(),
        recorded_at: 1,
        track: "Jacarepagua".into(),
        track_variation: String::new(),
        car_name: String::new(),
        car_class: "F-Retro_Gen3".into(),
        session_type: 5,
        results,
        lap_chart: vec![],
    };
    // One earlier session misflagged by the camera; the career's driver is still Nightrat.
    let sessions = vec![
        session(vec![result("Nightrat", true), result("Niki Lauda", false)]),
        session(vec![result("Nightrat", true), result("Niki Lauda", false)]),
        session(vec![result("Nightrat", false), result("Niki Lauda", true)]),
    ];
    let grid = ["Alain Prost", "Niki Lauda", "Nightrat"];

    // The 1984 bug: AMS2's camera sits on Lauda (index 1) while the session loads.
    assert_eq!(live_player_index(&sessions, &grid, Some(1)), Some(2));
    // Nothing recorded yet, or the name not on this grid: the viewed car is all there is.
    assert_eq!(live_player_index(&[], &grid, Some(1)), Some(1));
    assert_eq!(live_player_index(&sessions, &["Alain Prost", "Niki Lauda"], Some(0)), Some(0));
    assert_eq!(live_player_index(&[], &grid, None), None);
}
#[test]
fn test_a_finished_season_is_read_only() {
    let (app, root) = test_app("final_ro", CareerMode::Singleplayer);
    let champ = app
        .create_championship(NewChampionship {
            custom_ai_file: Some("F-Classic_Gen1.xml".into()),
            planned_rounds: Some(8),
            ..season("1983")
        })
        .unwrap();
    app.add_round(&champ.id).unwrap();
    let finish = ChampionshipPatch {
        status: Some(ChampionshipStatus::Final),
        ..Default::default()
    };
    app.patch_championship(&champ.id, finish.clone()).unwrap();

    let refused = |r: AppResult<Championship>| match r {
        Err(AppError::Conflict(msg)) => assert!(msg.contains("is finished"), "{msg}"),
        other => panic!("expected a finished-season refusal, got {other:?}"),
    };
    refused(app.patch_championship(
        &champ.id,
        ChampionshipPatch { name: Some("1983 again".into()), ..Default::default() },
    ));
    refused(app.patch_championship(
        &champ.id,
        ChampionshipPatch { planned_rounds: Some(16), ..Default::default() },
    ));
    refused(app.add_round(&champ.id));
    refused(app.remove_round(&champ.id, 0));
    refused(app.add_session_to_round(&champ.id, 0, "s1"));
    refused(app.remove_session_from_round(&champ.id, 0, "s1"));
    assert!(matches!(app.delete_championship(&champ.id), Err(AppError::Conflict(_))));
    assert!(matches!(
        app.sign(&champ.id, &SignRequest { team: "Lotus".into(), seat: None }),
        Err(AppError::Conflict(_))
    ));
    // Restating its status is not a change.
    assert!(app.patch_championship(&champ.id, finish).is_ok());
    // Nothing reached the season.
    let kept = app.data().championships[0].clone();
    assert_eq!((kept.name.as_str(), kept.rounds.len(), kept.planned_rounds), ("1983", 1, Some(8)));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_a_finished_multiplayer_season_reopens_to_be_edited() {
    let (app, root) = test_app("final_mp", CareerMode::Multiplayer);
    let champ = app.create_championship(season("League")).unwrap();
    let status = |s| ChampionshipPatch { status: Some(s), ..Default::default() };
    app.patch_championship(&champ.id, status(ChampionshipStatus::Final)).unwrap();
    match app.add_round(&champ.id) {
        Err(AppError::Conflict(msg)) => assert!(msg.contains("back to Progress"), "{msg}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    app.patch_championship(&champ.id, status(ChampionshipStatus::Progress)).unwrap();
    assert!(app.add_round(&champ.id).is_ok());
    std::fs::remove_dir_all(&root).ok();
}
#[test]
fn test_deleting_an_unraced_season_tears_up_its_contract() {
    // The bug a real save hit: a signed, unraced season deleted, and its contract left behind as
    // a season still "Racing" — the Finances tab took it for the current one.
    let (app, root) = lineage_app("delete_unraced");
    app.store.write().unwrap().contracts.push(
        serde_json::from_value(serde_json::json!({
            "champ_id": "c84", "team": "ATS-BMW", "salary": 34696, "bought_for": 30135
        }))
        .unwrap(),
    );
    let before = app.finances().finances.balance;
    app.delete_championship("c84").unwrap();
    let data = app.data();
    assert!(data.contracts.iter().all(|c| c.champ_id != "c84"), "the deal went with the season");
    assert_eq!(app.finances().finances.balance, before + 30135, "sponsorship paid back");
    let saved = crate::career::data_store::try_load_data(&app.active_path()).unwrap();
    assert_eq!(saved.contracts.len(), 1, "only 1983's contract reached the file");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_deleting_a_started_season_keeps_its_contract_as_a_deleted_row() {
    use crate::career::data_store::Round;
    let (app, root) = lineage_app("delete_started");
    {
        let mut data = app.store.write().unwrap();
        data.championships[1].rounds.push(Round { session_ids: vec!["s1".into()] });
        data.contracts.push(
            serde_json::from_value(serde_json::json!({
                "champ_id": "c84", "team": "ATS-BMW", "salary": 34696, "bought_for": 30135
            }))
            .unwrap(),
        );
    }
    app.delete_championship("c84").unwrap();
    let f = app.finances().finances;
    let row = f.seasons.iter().find(|s| s.champ_id == "c84").expect("the money spent keeps a row");
    assert!(row.deleted && !row.complete);
    assert_eq!(row.bought_for, 30135);
    std::fs::remove_dir_all(&root).ok();
}
