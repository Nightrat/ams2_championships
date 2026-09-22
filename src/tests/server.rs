use super::*;
use ams2_championship::data_store::{
    CareerData, Championship, ChampionshipStatus, RecordedSession, Round, SessionResult,
};
use ams2_championship::http::{parse_request, track_slug, Request};
use ams2_championship::websocket::{base64_encode, sha1, ws_accept_key, ws_send_text};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, RwLock};

fn req(raw: &[u8]) -> Request {
    parse_request(raw)
}

// ── HTTP route integration helpers ────────────────────────────────────────────

fn make_test_store() -> (
    ams2_championship::data_store::SharedStore,
    std::path::PathBuf,
) {
    let store = Arc::new(RwLock::new(CareerData::default()));
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ams2_srv_test_{ns}.json"));
    (store, path)
}

/// Send `request_bytes` to a temporary `handle()` invocation and return the full response.
fn call(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    request_bytes: Vec<u8>,
) -> String {
    call_with_config(store, data_path, request_bytes, None)
}

/// [`call`] with a caller-supplied `config.json`, for routes that read configured folders.
/// `None` gives each call a fresh path, so the defaults apply.
fn call_with_config(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    request_bytes: Vec<u8>,
    config: Option<std::path::PathBuf>,
) -> String {
    // Saves live alongside the career file, as they do under championships/ in production.
    let saves_dir = data_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    call_full(store, data_path, saves_dir, request_bytes, config)
}

/// [`call_with_config`] with the saves folder given explicitly — needed when there is no active
/// career, since then there is no career path to derive it from.
fn call_full(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    saves_dir: std::path::PathBuf,
    request_bytes: Vec<u8>,
    config: Option<std::path::PathBuf>,
) -> String {
    String::from_utf8_lossy(&call_full_raw(
        store,
        data_path,
        saves_dir,
        request_bytes,
        config,
    ))
    .into_owned()
}

/// [`call_full`] without the lossy conversion — for the one route that answers with an image,
/// whose bytes would not survive being read as text.
fn call_full_raw(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    saves_dir: std::path::PathBuf,
    request_bytes: Vec<u8>,
    config: Option<std::path::PathBuf>,
) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let html = Arc::new(b"<html/>".to_vec());
    let saves_dir = Arc::new(saves_dir);
    let dp: ams2_championship::data_store::SavePath = Arc::new(RwLock::new(data_path));
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let layouts_dir = Arc::new(std::env::temp_dir().join(format!("ams2_layouts_{ns}")));
    std::fs::create_dir_all(layouts_dir.as_ref()).unwrap();
    let config_path = Arc::new(
        config.unwrap_or_else(|| std::env::temp_dir().join(format!("ams2_cfg_{ns}.json"))),
    );
    let s = store;
    std::thread::spawn(move || {
        let (conn, _) = listener.accept().unwrap();
        handle(
            conn,
            html,
            s,
            dp,
            saves_dir,
            layouts_dir,
            config_path,
            200,
            Arc::new(Mutex::new(
                ams2_championship::spotter::SpotterConfig::default(),
            )),
        );
    });
    let mut client = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    client.write_all(&request_bytes).unwrap();
    let mut resp = Vec::new();
    client.read_to_end(&mut resp).unwrap();
    resp
}

fn get(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    path: &str,
) -> String {
    call(
        store,
        data_path,
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes(),
    )
}

fn post(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    path: &str,
    body: &[u8],
) -> String {
    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    call(store, data_path, req)
}

fn patch(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    path: &str,
    body: &[u8],
) -> String {
    let mut req = format!(
        "PATCH {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    call(store, data_path, req)
}

fn delete(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    path: &str,
) -> String {
    call(
        store,
        data_path,
        format!("DELETE {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes(),
    )
}

fn status_line(resp: &str) -> &str {
    resp.lines().next().unwrap_or("")
}

fn body(resp: &str) -> &str {
    resp.find("\r\n\r\n").map(|i| &resp[i + 4..]).unwrap_or("")
}

fn body_json(resp: &str) -> serde_json::Value {
    serde_json::from_str(body(resp)).expect("response body should be valid JSON")
}

/// A store for a singleplayer career — the only kind that signs contracts.
fn make_sp_store() -> (
    ams2_championship::data_store::SharedStore,
    std::path::PathBuf,
) {
    let (store, path) = make_test_store();
    store.write().unwrap().mode = ams2_championship::data_store::CareerMode::Singleplayer;
    (store, path)
}

fn make_champ(id: &str) -> Championship {
    Championship {
        id: id.into(),
        name: "Test Champ".into(),
        status: ChampionshipStatus::Active,
        points_system: vec![25, 18, 15],
        manufacturer_scoring: false,
        rounds: vec![],
        session_ids: vec![],
        custom_ai_file: None,
        player_team: None,
        planned_rounds: None,
    }
}

// ── GET routes ────────────────────────────────────────────────────────────────

#[test]
fn test_route_get_sessions_empty() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/sessions");
    assert!(status_line(&resp).contains("200"));
    assert_eq!(body_json(&resp).as_array().unwrap().len(), 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_get_championships_empty() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/championships");
    assert!(status_line(&resp).contains("200"));
    assert_eq!(body_json(&resp).as_array().unwrap().len(), 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_get_career_empty() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/career");
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert!(v.get("championships").is_some());
    assert!(v.get("driver_stats").is_some());
    assert!(v.get("track_stats").is_some());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_get_live_teams_without_custom_ai() {
    // No Custom AI folder configured (and no live session in the test environment),
    // so the live grid falls back to whatever car name AMS2 reports.
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/live-teams");
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["teams"].as_object().unwrap().len(), 0);
    assert!(v["player_team"].is_null());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_default_returns_html() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/");
    assert!(status_line(&resp).contains("200"));
    assert!(
        body(&resp).contains("<html"),
        "default route should return HTML"
    );
    let _ = std::fs::remove_file(&path);
}

// ── POST /api/championships ────────────────────────────────────────────────────

#[test]
fn test_route_post_championships_creates_championship() {
    let (store, path) = make_test_store();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        b"{\"name\":\"My Champ\"}",
    );
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["name"], "My Champ");
    assert!(v["id"].as_str().is_some());
    assert_eq!(store.read().unwrap().championships.len(), 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_post_championships_uses_default_points_when_absent() {
    let (store, path) = make_test_store();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        b"{\"name\":\"X\"}",
    );
    let v = body_json(&resp);
    let pts: Vec<i64> = v["points_system"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect();
    assert_eq!(pts, vec![25, 18, 15, 12, 10, 8, 6, 4, 2, 1]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_post_championships_invalid_body_returns_400() {
    let (store, path) = make_test_store();
    let resp = post(store, path.clone(), "/api/championships", b"not json");
    assert!(status_line(&resp).contains("400"));
    let _ = std::fs::remove_file(&path);
}

// ── PATCH /api/championships/:id ─────────────────────────────────────────────

#[test]
fn test_route_patch_championship_updates_name() {
    let (store, path) = make_test_store();
    store.write().unwrap().championships.push(make_champ("42"));
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/42",
        b"{\"name\":\"Renamed\"}",
    );
    assert!(status_line(&resp).contains("200"));
    assert_eq!(store.read().unwrap().championships[0].name, "Renamed");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_championship_not_found_returns_404() {
    let (store, path) = make_test_store();
    let resp = patch(
        store,
        path.clone(),
        "/api/championships/999",
        b"{\"name\":\"X\"}",
    );
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_championship_only_one_active_at_a_time() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.championships.push(make_champ("1")); // Active
        let mut c2 = make_champ("2");
        c2.status = ChampionshipStatus::Progress;
        data.championships.push(c2);
    }
    // Set c2 to Active — c1 should become Progress
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/2",
        b"{\"status\":\"Active\"}",
    );
    assert!(status_line(&resp).contains("200"));
    let data = store.read().unwrap();
    assert_eq!(
        data.championships
            .iter()
            .find(|c| c.id == "1")
            .unwrap()
            .status,
        ChampionshipStatus::Progress
    );
    assert_eq!(
        data.championships
            .iter()
            .find(|c| c.id == "2")
            .unwrap()
            .status,
        ChampionshipStatus::Active
    );
    let _ = std::fs::remove_file(&path);
}

// ── DELETE /api/championships/:id ────────────────────────────────────────────

#[test]
fn test_route_delete_championship_removes_it() {
    let (store, path) = make_test_store();
    store.write().unwrap().championships.push(make_champ("99"));
    let resp = delete(store.clone(), path.clone(), "/api/championships/99");
    assert!(status_line(&resp).contains("200"));
    assert_eq!(store.read().unwrap().championships.len(), 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_delete_championship_not_found_returns_404() {
    let (store, path) = make_test_store();
    let resp = delete(store, path.clone(), "/api/championships/404");
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

// ── POST /api/championships/:id/rounds ───────────────────────────────────────

#[test]
fn test_route_post_round_adds_empty_round() {
    let (store, path) = make_test_store();
    store.write().unwrap().championships.push(make_champ("c1"));
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships/c1/rounds",
        b"",
    );
    assert!(status_line(&resp).contains("200"));
    assert_eq!(store.read().unwrap().championships[0].rounds.len(), 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_post_round_unknown_champ_returns_404() {
    let (store, path) = make_test_store();
    let resp = post(store, path.clone(), "/api/championships/nope/rounds", b"");
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

// ── DELETE /api/championships/:id/rounds/:ridx ───────────────────────────────

#[test]
fn test_route_delete_round_removes_it() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds = vec![Round::default(), Round::default()];
        data.championships.push(champ);
    }
    let resp = delete(
        store.clone(),
        path.clone(),
        "/api/championships/c1/rounds/0",
    );
    assert!(status_line(&resp).contains("200"));
    assert_eq!(store.read().unwrap().championships[0].rounds.len(), 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_delete_round_out_of_bounds_returns_404() {
    let (store, path) = make_test_store();
    store.write().unwrap().championships.push(make_champ("c1")); // 0 rounds
    let resp = delete(store, path.clone(), "/api/championships/c1/rounds/0");
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

// ── POST /api/championships/:id/rounds/:r/sessions/:sid ──────────────────────

// ── GET /api/championships/:id/team-eligibility ──────────────────────────────

#[test]
fn test_route_team_eligibility_unrated_without_custom_ai_file() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.championships.push(make_champ("c1"));
    }
    let resp = get(
        store,
        path.clone(),
        "/api/championships/c1/team-eligibility",
    );
    assert!(status_line(&resp).contains("200"));
    // No roster to rate against, so nothing is gated — but enforcement state is still reported.
    assert!(resp.contains("\"rated\":false"), "got {resp}");
    assert!(
        resp.contains("\"enforced\":true"),
        "enforcement defaults on: {resp}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_team_eligibility_unknown_championship_is_404() {
    let (store, path) = make_test_store();
    let resp = get(
        store,
        path.clone(),
        "/api/championships/nope/team-eligibility",
    );
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_explicit_null_clears_assignments() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        champ.player_team = Some("Brabham".into());
        data.championships.push(champ);
    }
    // A plain Option<Option<_>> collapses an explicit null into "key absent", which silently
    // made these impossible to clear — the UI's "(none)" choice did nothing.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"player_team":null}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(store.read().unwrap().championships[0].player_team, None);

    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"custom_ai_file":null}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(store.read().unwrap().championships[0].custom_ai_file, None);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_omitted_key_leaves_assignment_untouched() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        champ.player_team = Some("Brabham".into());
        data.championships.push(champ);
    }
    // The other half of the contract: absent means "leave alone", not "clear".
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"name":"Renamed"}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    let data = store.read().unwrap();
    assert_eq!(
        data.championships[0].player_team.as_deref(),
        Some("Brabham")
    );
    assert_eq!(
        data.championships[0].custom_ai_file.as_deref(),
        Some("F-Classic_Gen1.xml")
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_player_team_locked_once_a_session_is_assigned() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        champ.player_team = Some("Brabham".into());
        champ.rounds.push(Round {
            session_ids: vec!["sess1".into()],
        });
        data.championships.push(champ);
    }
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"player_team":"Williams"}"#,
    );
    assert!(status_line(&resp).contains("409"), "got {resp}");
    assert_eq!(
        store.read().unwrap().championships[0]
            .player_team
            .as_deref(),
        Some("Brabham"),
        "a rejected change must leave the team untouched"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_locked_team_still_allows_other_edits() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        champ.player_team = Some("Brabham".into());
        champ.rounds.push(Round {
            session_ids: vec!["sess1".into()],
        });
        data.championships.push(champ);
    }
    // Renaming touches no team, so the lock must not stand in the way.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"name":"1986 Season"}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(store.read().unwrap().championships[0].name, "1986 Season");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_team_changeable_before_any_session() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        // An empty round is not a started championship.
        champ.rounds.push(Round::default());
        data.championships.push(champ);
    }
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"player_team":"Williams"}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(
        store.read().unwrap().championships[0]
            .player_team
            .as_deref(),
        Some("Williams")
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_started_champ_blocks_clearing_the_custom_ai_file() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        champ.player_team = Some("Brabham".into());
        champ.rounds.push(Round {
            session_ids: vec!["sess1".into()],
        });
        data.championships.push(champ);
    }
    // Unassigning the roster would leave the rounds already scored with nothing to have been
    // measured against, and clears the team as a side effect too.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"custom_ai_file":null}"#,
    );
    assert!(status_line(&resp).contains("409"), "got {resp}");
    let data = store.read().unwrap();
    assert_eq!(
        data.championships[0].custom_ai_file.as_deref(),
        Some("F-Classic_Gen1.xml")
    );
    assert_eq!(
        data.championships[0].player_team.as_deref(),
        Some("Brabham")
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_custom_ai_file_locked_once_a_session_is_assigned() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        champ.rounds.push(Round {
            session_ids: vec!["sess1".into()],
        });
        data.championships.push(champ);
    }
    // No team is claimed here, so only the roster lock can catch this: swapping the file would
    // re-score the assigned round against a different grid.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"custom_ai_file":"F-Retro_Gen2.xml"}"#,
    );
    assert!(status_line(&resp).contains("409"), "got {resp}");
    assert_eq!(
        store.read().unwrap().championships[0]
            .custom_ai_file
            .as_deref(),
        Some("F-Classic_Gen1.xml"),
        "a rejected change must leave the roster untouched"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_custom_ai_file_changeable_before_any_session() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        // An empty round is not a started championship.
        champ.rounds.push(Round::default());
        data.championships.push(champ);
    }
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"custom_ai_file":"F-Retro_Gen2.xml"}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(
        store.read().unwrap().championships[0]
            .custom_ai_file
            .as_deref(),
        Some("F-Retro_Gen2.xml")
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_started_champ_accepts_an_unchanged_custom_ai_file() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        champ.rounds.push(Round {
            session_ids: vec!["sess1".into()],
        });
        data.championships.push(champ);
    }
    // Only a *change* is gated — re-sending the value it already has must not 409.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"custom_ai_file":"F-Classic_Gen1.xml","name":"1986 Season"}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(store.read().unwrap().championships[0].name, "1986 Season");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_player_team_is_not_gated_without_a_roster() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        data.championships.push(champ);
    }
    // With no custom_ai_dir configured the rating cannot be computed, and an unrateable team
    // must never be blocked.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"player_team":"Williams"}"#,
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(
        store.read().unwrap().championships[0]
            .player_team
            .as_deref(),
        Some("Williams")
    );
    let _ = std::fs::remove_file(&path);
}

// ── GET /api/championships/:id/session-eligibility ───────────────────────────

#[test]
fn test_route_session_eligibility_not_enforced_without_player_team() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        // A Custom AI file alone is not enough — a selected team is what turns enforcement on.
        let mut champ = make_champ("c1");
        champ.custom_ai_file = Some("F-Classic_Gen1.xml".into());
        data.championships.push(champ);
    }
    let resp = get(
        store,
        path.clone(),
        "/api/championships/c1/session-eligibility",
    );
    assert!(status_line(&resp).contains("200"));
    assert!(resp.contains("\"enforced\":false"), "got {resp}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_session_eligibility_unknown_championship_is_404() {
    let (store, path) = make_test_store();
    let resp = get(
        store,
        path.clone(),
        "/api/championships/nope/session-eligibility",
    );
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_post_session_to_round_adds_it() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds.push(Round::default());
        data.championships.push(champ);
    }
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships/c1/rounds/0/sessions/sess1",
        b"",
    );
    assert!(status_line(&resp).contains("200"));
    let data = store.read().unwrap();
    assert!(data.championships[0].rounds[0]
        .session_ids
        .contains(&"sess1".to_string()));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_post_session_to_round_deduplicates() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds.push(Round {
            session_ids: vec!["sess1".into()],
        });
        data.championships.push(champ);
    }
    post(
        store.clone(),
        path.clone(),
        "/api/championships/c1/rounds/0/sessions/sess1",
        b"",
    );
    assert_eq!(
        store.read().unwrap().championships[0].rounds[0]
            .session_ids
            .len(),
        1
    );
    let _ = std::fs::remove_file(&path);
}

// ── DELETE /api/championships/:id/rounds/:r/sessions/:sid ────────────────────

#[test]
fn test_route_delete_session_from_round_removes_it() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds.push(Round {
            session_ids: vec!["s1".into(), "s2".into()],
        });
        data.championships.push(champ);
    }
    let resp = delete(
        store.clone(),
        path.clone(),
        "/api/championships/c1/rounds/0/sessions/s1",
    );
    assert!(status_line(&resp).contains("200"));
    let data = store.read().unwrap();
    assert!(!data.championships[0].rounds[0]
        .session_ids
        .contains(&"s1".to_string()));
    assert!(data.championships[0].rounds[0]
        .session_ids
        .contains(&"s2".to_string()));
    let _ = std::fs::remove_file(&path);
}

// ── DELETE /api/sessions/unassigned ──────────────────────────────────────────

#[test]
fn test_route_delete_unassigned_sessions_removes_orphans() {
    use ams2_championship::data_store::RecordedSession;
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        // Session "s1" is assigned; "s2" is not
        let mut champ = make_champ("c1");
        champ.rounds.push(Round {
            session_ids: vec!["s1".into()],
        });
        data.championships.push(champ);
        for id in &["s1", "s2"] {
            data.sessions.push(RecordedSession {
                id: (*id).into(),
                recorded_at: 1000,
                track: "Spa".into(),
                track_variation: "GP".into(),
                car_name: String::new(),
                car_class: String::new(),
                session_type: 5,
                results: vec![],
                lap_chart: vec![],
            });
        }
    }
    let resp = delete(store.clone(), path.clone(), "/api/sessions/unassigned");
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["removed"], 1);
    assert_eq!(store.read().unwrap().sessions.len(), 1);
    assert_eq!(store.read().unwrap().sessions[0].id, "s1");
    let _ = std::fs::remove_file(&path);
}

// ── POST /api/record-session ──────────────────────────────────────────────────

#[test]
fn test_route_record_session_returns_409_when_not_connected() {
    // In the test environment AMS2 shared memory is not available → disconnected
    let (store, path) = make_test_store();
    let resp = post(store, path.clone(), "/api/record-session", b"");
    assert!(status_line(&resp).contains("409"));
    let _ = std::fs::remove_file(&path);
}

// ── GET /api/track-layout ─────────────────────────────────────────────────────

#[test]
fn test_route_get_track_layout_returns_null_when_missing() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/track-layout/spa");
    assert!(status_line(&resp).contains("200"));
    assert_eq!(body(&resp), "null");
    let _ = std::fs::remove_file(&path);
}

// ── POST /api/track-layout ────────────────────────────────────────────────────

#[test]
fn test_route_post_track_layout_rejects_too_few_points() {
    let (store, path) = make_test_store();
    // Array with < 300 entries
    let few: serde_json::Value = serde_json::Value::Array(vec![serde_json::json!([0, 0]); 10]);
    let body = serde_json::to_vec(&few).unwrap();
    let resp = post(store, path.clone(), "/api/track-layout/spa", &body);
    assert!(status_line(&resp).contains("400"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_parse_get_root() {
    let r = req(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, "/");
    assert!(r.body.is_empty());
}

#[test]
fn test_parse_get_api_path() {
    let r = req(b"GET /api/championships HTTP/1.1\r\n\r\n");
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, "/api/championships");
}

#[test]
fn test_parse_get_live() {
    let r = req(b"GET /live HTTP/1.1\r\n\r\n");
    assert_eq!(r.path, "/live");
}

#[test]
fn test_parse_delete_with_id() {
    let r = req(b"DELETE /api/championships/12345 HTTP/1.1\r\n\r\n");
    assert_eq!(r.method, "DELETE");
    assert_eq!(r.path, "/api/championships/12345");
}

#[test]
fn test_parse_delete_session_assignment() {
    let r = req(b"DELETE /api/championships/abc/sessions/xyz HTTP/1.1\r\n\r\n");
    assert_eq!(r.method, "DELETE");
    assert_eq!(r.path, "/api/championships/abc/sessions/xyz");
}

#[test]
fn test_parse_post_with_json_body() {
    let body = b"{\"name\":\"Test Champ\"}";
    let header = format!(
        "POST /api/championships HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let mut raw = header.into_bytes();
    raw.extend_from_slice(body);

    let r = req(&raw);
    assert_eq!(r.method, "POST");
    assert_eq!(r.path, "/api/championships");
    assert_eq!(r.body, body);
}

#[test]
fn test_parse_patch_with_body() {
    let body = b"{\"status\":\"Final\"}";
    let header = format!(
        "PATCH /api/championships/99 HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let mut raw = header.into_bytes();
    raw.extend_from_slice(body);

    let r = req(&raw);
    assert_eq!(r.method, "PATCH");
    assert_eq!(r.path, "/api/championships/99");
    assert_eq!(r.body, body);
}

#[test]
fn test_parse_empty_buffer_defaults() {
    let r = req(b"");
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, "/");
    assert!(r.body.is_empty());
}

#[test]
fn test_parse_no_body_after_headers() {
    let r = req(b"GET /api/sessions HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(r.body.is_empty());
}

#[test]
fn test_parse_path_segments_round_session_route() {
    let r = req(b"POST /api/championships/42/rounds/0/sessions/7 HTTP/1.1\r\n\r\n");
    let segs: Vec<&str> = r.path.trim_start_matches('/').split('/').collect();
    assert_eq!(
        segs,
        ["api", "championships", "42", "rounds", "0", "sessions", "7"]
    );
    assert_eq!(segs.len(), 7);
}

#[test]
fn test_parse_path_segments_add_round_route() {
    let r = req(b"POST /api/championships/42/rounds HTTP/1.1\r\n\r\n");
    let segs: Vec<&str> = r.path.trim_start_matches('/').split('/').collect();
    assert_eq!(segs, ["api", "championships", "42", "rounds"]);
    assert_eq!(segs.len(), 4);
}

#[test]
fn test_parse_headers_field_captured() {
    let r = req(b"GET /ws HTTP/1.1\r\nUpgrade: websocket\r\nSec-WebSocket-Key: abc123\r\n\r\n");
    assert!(r.headers.contains("Upgrade: websocket"));
    assert!(r.headers.contains("Sec-WebSocket-Key: abc123"));
}

// ── sha1 ──────────────────────────────────────────────────────────────────────

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn test_sha1_empty() {
    // Well-known SHA-1 of empty string
    assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn test_sha1_abc() {
    assert_eq!(
        hex(&sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn test_sha1_longer_message() {
    // "The quick brown fox jumps over the lazy dog"
    assert_eq!(
        hex(&sha1(b"The quick brown fox jumps over the lazy dog")),
        "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
    );
}

#[test]
fn test_sha1_multichunk() {
    // Input longer than 64 bytes (two SHA-1 blocks)
    let input = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(
        hex(&sha1(input)),
        "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
    );
}

// ── base64_encode ─────────────────────────────────────────────────────────────

#[test]
fn test_base64_empty() {
    assert_eq!(base64_encode(b""), "");
}

#[test]
fn test_base64_one_byte() {
    assert_eq!(base64_encode(b"f"), "Zg==");
}

#[test]
fn test_base64_two_bytes() {
    assert_eq!(base64_encode(b"fo"), "Zm8=");
}

#[test]
fn test_base64_three_bytes() {
    assert_eq!(base64_encode(b"foo"), "Zm9v");
}

#[test]
fn test_base64_foobar() {
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn test_base64_man() {
    assert_eq!(base64_encode(b"Man"), "TWFu");
}

// ── ws_accept_key ─────────────────────────────────────────────────────────────

#[test]
fn test_ws_accept_key_rfc6455_example() {
    // Example from RFC 6455 Section 1.3
    assert_eq!(
        ws_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

// ── track_slug ────────────────────────────────────────────────────────────────

#[test]
fn test_track_slug_simple() {
    assert_eq!(track_slug("Silverstone"), "silverstone");
}

#[test]
fn test_track_slug_spaces_collapsed() {
    assert_eq!(track_slug("Le Mans"), "le_mans");
}

#[test]
fn test_track_slug_special_chars_collapsed() {
    assert_eq!(track_slug("Spa \u{2013} GP"), "spa_gp");
}

#[test]
fn test_track_slug_multiple_separators() {
    assert_eq!(track_slug("Jerez de la Frontera"), "jerez_de_la_frontera");
}

#[test]
fn test_track_slug_empty() {
    assert_eq!(track_slug(""), "");
}

#[test]
fn test_track_slug_numbers_preserved() {
    assert_eq!(track_slug("Circuit 1"), "circuit_1");
}

// ── ws_send_text ──────────────────────────────────────────────────────────────

fn ws_capture(payload: &[u8]) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let payload = payload.to_vec();
    std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        ws_send_text(&mut conn, &payload).unwrap();
    });
    let mut client = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    buf
}

#[test]
fn test_ws_send_text_small_payload() {
    // < 126 bytes: [0x81, len, payload...]
    let frame = ws_capture(b"hello");
    assert_eq!(frame[0], 0x81);
    assert_eq!(frame[1], 5);
    assert_eq!(&frame[2..], b"hello");
}

#[test]
fn test_ws_send_text_medium_payload() {
    // 126..65536 bytes: [0x81, 126, len_hi, len_lo, payload...]
    let payload = vec![b'x'; 200];
    let frame = ws_capture(&payload);
    assert_eq!(frame[0], 0x81);
    assert_eq!(frame[1], 126);
    assert_eq!(u16::from_be_bytes([frame[2], frame[3]]) as usize, 200);
    assert_eq!(&frame[4..], payload.as_slice());
}

#[test]
fn test_ws_send_text_large_payload() {
    // >= 65536 bytes: [0x81, 127, len as u64 BE, payload...]
    let payload = vec![b'y'; 70_000];
    let frame = ws_capture(&payload);
    assert_eq!(frame[0], 0x81);
    assert_eq!(frame[1], 127);
    assert_eq!(
        u64::from_be_bytes(frame[2..10].try_into().unwrap()) as usize,
        70_000
    );
    assert_eq!(&frame[10..], payload.as_slice());
}

// ── /api/saves routes ─────────────────────────────────────────────────────────

/// A store plus an isolated saves directory holding `ams2_career.json` as the active save.
fn make_saves_dir(
    tag: &str,
) -> (
    ams2_championship::data_store::SharedStore,
    std::path::PathBuf,
) {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_saves_route_{tag}_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ams2_career.json");
    let store = Arc::new(RwLock::new(CareerData::default()));
    store.write().unwrap().championships.push(make_champ("c1"));
    ams2_championship::data_store::persist(&store, &path).expect("fixture save must be written");
    (store, path)
}

#[test]
fn test_route_get_saves_lists_active() {
    let (store, path) = make_saves_dir("list");
    let resp = get(store, path.clone(), "/api/saves");
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["active"], "ams2_career");
    let saves = v["saves"].as_array().unwrap();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0]["championships"], 1);
    assert_eq!(saves[0]["active"], true);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_post_saves_creates_empty_and_activates() {
    let (store, path) = make_saves_dir("create");
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/saves",
        br#"{"name":"GT3 Career","mode":"multiplayer"}"#,
    );
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["active"], "GT3 Career");
    assert_eq!(v["saves"].as_array().unwrap().len(), 2);
    // The in-memory store was swapped to the new, empty career.
    assert!(store.read().unwrap().championships.is_empty());
    // A new career is created in the folder layout: <saves>/GT3 Career/career.json
    assert!(ams2_championship::saves::save_path(path.parent().unwrap(), "GT3 Career").exists());
    // The kind is recorded at creation and is what every rule below turns on.
    assert_eq!(store.read().unwrap().mode, ams2_championship::data_store::CareerMode::Multiplayer);
    // The previous career is untouched on disk.
    let old = ams2_championship::data_store::load_data(&path);
    assert_eq!(old.championships.len(), 1);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_post_saves_rejects_duplicate_name() {
    let (store, path) = make_saves_dir("dup_name");
    let resp = post(
        store,
        path.clone(),
        "/api/saves",
        br#"{"name":"ams2_career"}"#,
    );
    assert!(status_line(&resp).contains("409"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_post_saves_rejects_traversal_name() {
    let (store, path) = make_saves_dir("traversal");
    let resp = post(store, path.clone(), "/api/saves", br#"{"name":"../evil"}"#);
    assert!(status_line(&resp).contains("400"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_activate_swaps_store_contents() {
    let (store, path) = make_saves_dir("activate");
    let dir = path.parent().unwrap().to_path_buf();
    // A second career on disk with two championships.
    let other = dir.join("other.json");
    let other_store = Arc::new(RwLock::new(CareerData::default()));
    other_store
        .write()
        .unwrap()
        .championships
        .push(make_champ("x1"));
    other_store
        .write()
        .unwrap()
        .championships
        .push(make_champ("x2"));
    ams2_championship::data_store::persist(&other_store, &other)
        .expect("fixture save must be written");

    let resp = post(
        store.clone(),
        path.clone(),
        "/api/saves/activate",
        br#"{"name":"other"}"#,
    );
    assert!(status_line(&resp).contains("200"));
    assert_eq!(body_json(&resp)["active"], "other");
    assert_eq!(
        store.read().unwrap().championships.len(),
        2,
        "store now holds the other career"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_activate_unknown_save_404s() {
    let (store, path) = make_saves_dir("activate_404");
    let resp = post(
        store,
        path.clone(),
        "/api/saves/activate",
        br#"{"name":"nope"}"#,
    );
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_duplicate_copies_without_switching() {
    let (store, path) = make_saves_dir("duplicate");
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/saves/duplicate",
        br#"{"name":"ams2_career","new_name":"backup"}"#,
    );
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["active"], "ams2_career", "duplicating does not switch");
    // The source here is a legacy flat save; the copy is a new save, so it is written the way
    // new saves are written.
    let copy = ams2_championship::saves::save_path(path.parent().unwrap(), "backup");
    assert!(copy.exists());
    assert!(path.exists(), "the original stays where it was");
    assert_eq!(
        ams2_championship::data_store::load_data(&copy)
            .championships
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_rename_active_save_follows_the_path() {
    let (store, path) = make_saves_dir("rename");
    let dir = path.parent().unwrap().to_path_buf();
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/saves/ams2_career",
        br#"{"new_name":"Renamed"}"#,
    );
    assert!(status_line(&resp).contains("200"));
    assert_eq!(
        body_json(&resp)["active"],
        "Renamed",
        "active save follows the rename"
    );
    assert!(!path.exists());
    assert!(dir.join("Renamed.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_rename_percent_encoded_name() {
    let (store, path) = make_saves_dir("rename_enc");
    let dir = path.parent().unwrap().to_path_buf();
    std::fs::copy(&path, dir.join("GT3 Career.json")).unwrap();
    let resp = patch(
        store,
        path.clone(),
        "/api/saves/GT3%20Career",
        br#"{"new_name":"GT4 Career"}"#,
    );
    assert!(status_line(&resp).contains("200"));
    assert!(dir.join("GT4 Career.json").exists());
    assert!(!dir.join("GT3 Career.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_delete_save() {
    let (store, path) = make_saves_dir("delete");
    let dir = path.parent().unwrap().to_path_buf();
    std::fs::copy(&path, dir.join("scratch.json")).unwrap();
    let resp = delete(store, path.clone(), "/api/saves/scratch");
    assert!(status_line(&resp).contains("200"));
    assert!(!dir.join("scratch.json").exists());
    assert!(path.exists(), "active save untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_delete_active_save_rejected() {
    let (store, path) = make_saves_dir("delete_active");
    let resp = delete(store, path.clone(), "/api/saves/ams2_career");
    assert!(status_line(&resp).contains("400"));
    assert!(path.exists(), "active save must survive");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Put a file in a subfolder of a save, standing in for whatever a career comes to own beside
/// its sessions — the reason the folder layout exists.
fn seed_nested(career: &std::path::Path) -> std::path::PathBuf {
    let dir = ams2_championship::saves::career_dir(career).unwrap().join("extra");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("notes.txt");
    std::fs::write(&file, "nested").unwrap();
    file
}

#[test]
fn test_route_rename_folder_save_carries_everything_in_it() {
    let (store, path) = make_saves_dir("rename_folder");
    let dir = path.parent().unwrap().to_path_buf();
    // A folder save beside the legacy active one.
    let career = ams2_championship::saves::save_path(&dir, "Project");
    ams2_championship::saves::prepare_save_dir(&career).unwrap();
    std::fs::copy(&path, &career).unwrap();
    seed_nested(&career);

    let resp = patch(
        store,
        path.clone(),
        "/api/saves/Project",
        br#"{"new_name":"Renamed"}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert!(!dir.join("Project").exists(), "the old folder is gone");
    let moved = ams2_championship::saves::save_path(&dir, "Renamed");
    assert!(moved.exists());
    assert!(
        ams2_championship::saves::career_dir(&moved)
            .unwrap()
            .join("extra")
            .join("notes.txt")
            .is_file(),
        "what the career owned is part of it, so it moves with it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_delete_folder_save_removes_everything_in_it() {
    let (store, path) = make_saves_dir("delete_folder");
    let dir = path.parent().unwrap().to_path_buf();
    let career = ams2_championship::saves::save_path(&dir, "Scratch");
    ams2_championship::saves::prepare_save_dir(&career).unwrap();
    std::fs::copy(&path, &career).unwrap();
    seed_nested(&career);

    let resp = delete(store, path.clone(), "/api/saves/Scratch");
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert!(!dir.join("Scratch").exists());
    assert!(path.exists(), "active save untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_post_saves_creates_the_first_career_when_none_is_active() {
    // An empty saves folder leaves the app with no active career, carried as an empty path.
    // Creating one is the route it must still serve — and it used to fail, because the switch
    // flushes the outgoing career first and flushing "" is refused.
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_saves_route_none_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(RwLock::new(CareerData::default()));

    let body = br#"{"name":"First","mode":"singleplayer"}"#;
    let mut req = format!(
        "POST /api/saves HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    // No active career: an empty path, exactly as startup leaves it for an empty saves folder.
    let resp = call_full(store.clone(), std::path::PathBuf::new(), dir.clone(), req, None);
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(body_json(&resp)["active"], "First");
    assert!(ams2_championship::saves::save_path(&dir, "First").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_post_saves_refuses_a_name_a_legacy_file_already_holds() {
    // The fixture's own save is a flat ams2_career.json. Creating a folder save of that name
    // would shadow it, so the name counts as taken in either layout.
    let (store, path) = make_saves_dir("clash");
    let resp = post(
        store,
        path.clone(),
        "/api/saves",
        br#"{"name":"ams2_career","mode":"singleplayer"}"#,
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(
        !path.parent().unwrap().join("ams2_career").exists(),
        "no folder was created beside the legacy file"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

// ── PATCH /api/config — saves folder ──────────────────────────────────────────

fn config_body(saves_dir: &str) -> Vec<u8> {
    format!(
        r#"{{"port":8080,"host":"127.0.0.1","poll_ms":200,"record_practice":true,
             "record_qualify":true,"record_race":true,"show_track_map":true,
             "track_map_max_points":5000,"saves_dir":{saves_dir}}}"#
    )
    .into_bytes()
}

/// A config file that already remembers an active career, so clearing it can be told apart from
/// it never having been set.
fn config_remembering(tag: &str, career: &str) -> std::path::PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file = std::env::temp_dir().join(format!("ams2_cfg_{tag}_{ns}.json"));
    std::fs::write(
        &file,
        format!(r#"{{"port":8080,"active_career":"{career}"}}"#),
    )
    .unwrap();
    file
}

fn patch_config_with(
    store: ams2_championship::data_store::SharedStore,
    data_path: std::path::PathBuf,
    body: &[u8],
    config: std::path::PathBuf,
) -> String {
    let mut req = format!(
        "PATCH /api/config HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    call_with_config(store, data_path, req, Some(config))
}

#[test]
fn test_route_patch_config_saves_dir_requires_restart_and_clears_the_active_career() {
    let (store, path) = make_saves_dir("cfg_saves_dir");
    let new_dir = path.parent().unwrap().join("elsewhere");
    let config = config_remembering("saves_dir", "ams2_career");

    let body = config_body(&serde_json::Value::String(new_dir.display().to_string()).to_string());
    let resp = patch_config_with(store, path.clone(), &body, config.clone());
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    assert_eq!(v["config"]["saves_dir"], new_dir.display().to_string());
    assert!(
        v["restart_required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("saves_dir")),
        "moving the saves folder only takes effect on restart"
    );
    assert!(
        v["config"]["active_career"].is_null(),
        "the remembered career lived in the old folder, so it is dropped"
    );
    assert!(new_dir.is_dir(), "the folder is created eagerly");

    let _ = std::fs::remove_file(&config);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_patch_config_unchanged_saves_dir_keeps_the_active_career() {
    let (store, path) = make_saves_dir("cfg_saves_same");
    let config = config_remembering("saves_same", "ams2_career");

    let resp = patch_config_with(store, path.clone(), &config_body("null"), config.clone());
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    assert!(
        !v["restart_required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("saves_dir")),
        "null == unset, so nothing changed"
    );
    assert_eq!(
        v["config"]["active_career"], "ams2_career",
        "the form does not carry the active career, so it must be carried through"
    );

    let _ = std::fs::remove_file(&config);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

// ── PATCH /api/car-performance ────────────────────────────────────────────────

const PERF_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<custom_ai_drivers>
    <driver livery_name="Williams #5 N. Mansell">
        <name>Nigel Mansell</name>
        <power_scalar>1.10</power_scalar>
        <weight_scalar>0.97</weight_scalar>
        <drag_scalar>0.95</drag_scalar>
    </driver>
    <driver livery_name="AGS #31 I. Capelli">
        <name>Ivan Capelli</name>
        <power_scalar>0.90</power_scalar>
        <weight_scalar>1.05</weight_scalar>
        <drag_scalar>1.10</drag_scalar>
    </driver>
</custom_ai_drivers>
"#;

/// A temp Custom AI Drivers folder holding `F-Test.xml`, plus a config.json pointing at it.
fn make_perf_fixture() -> (std::path::PathBuf, std::path::PathBuf) {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_perf_route_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("F-Test.xml"), PERF_XML).unwrap();
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            "{{\"custom_ai_dir\":{}}}",
            serde_json::to_string(&dir.display().to_string()).unwrap()
        ),
    )
    .unwrap();
    (dir, config)
}

fn patch_perf(body: &str, config: &std::path::Path) -> String {
    let (store, data_path) = make_test_store();
    let mut req = format!(
        "PATCH /api/car-performance HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    call_with_config(store, data_path, req, Some(config.to_path_buf()))
}

fn post_perf(path: &str, body: &str, config: &std::path::Path) -> String {
    let (store, data_path) = make_test_store();
    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    call_with_config(store, data_path, req, Some(config.to_path_buf()))
}

#[test]
fn test_route_patch_car_performance_writes_the_xml_and_returns_the_new_table() {
    let (dir, config) = make_perf_fixture();
    let resp = patch_perf(
        r#"{"class":"F-Test","team":"AGS","power_scalar":1.1,"weight_scalar":0.97,"drag_scalar":0.95}"#,
        &config,
    );
    assert!(resp.starts_with("HTTP/1.1 200 OK"), "{resp}");

    let cars = ams2_championship::custom_ai::parse_car_performance(&dir.join("F-Test.xml"));
    let ags = cars.iter().find(|c| c.team == "AGS").unwrap();
    assert_eq!(ags.power_scalar, 1.10);
    assert_eq!(ags.weight_scalar, 0.97);
    assert_eq!(ags.drag_scalar, 0.95);
    // The original is kept next to it.
    assert_eq!(
        std::fs::read_to_string(dir.join("F-Test.xml.bak")).unwrap(),
        PERF_XML
    );

    // AGS now matches Williams exactly, so the response shows both as class-fastest.
    let body = resp.split("\r\n\r\n").nth(1).unwrap();
    assert!(body.contains("\"pace_delta_pct\":0.0"), "{body}");
    assert_eq!(body.matches("\"pace_delta_pct\":0.0").count(), 2, "{body}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_car_performance_says_whether_a_class_has_a_baseline() {
    // A baseline is taken on the first edit, so a class nobody has touched has nothing to reset
    // to and the table needs to know before offering the button.
    let (dir, config) = make_perf_fixture();
    let (store, data_path) = make_test_store();
    let before = body_json(&call_with_config(
        store,
        data_path,
        b"GET /api/car-performance HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config.clone()),
    ));
    assert_eq!(before["classes"][0]["has_baseline"], false);

    patch_perf(
        r#"{"class":"F-Test","team":"AGS","power_scalar":1.10,"weight_scalar":0.97,"drag_scalar":0.95}"#,
        &config,
    );
    let after = body_json(&patch_perf(
        r#"{"class":"F-Test","team":"AGS","power_scalar":1.09,"weight_scalar":0.97,"drag_scalar":0.95}"#,
        &config,
    ));
    assert_eq!(after["classes"][0]["has_baseline"], true);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_reset_restores_the_class_from_its_baseline() {
    let (dir, config) = make_perf_fixture();
    patch_perf(
        r#"{"class":"F-Test","team":"AGS","power_scalar":1.10,"weight_scalar":0.97,"drag_scalar":0.95}"#,
        &config,
    );
    assert_ne!(
        std::fs::read_to_string(dir.join("F-Test.xml")).unwrap(),
        PERF_XML,
        "the edit landed"
    );

    let resp = post_perf("/api/car-performance/reset", r#"{"class":"F-Test"}"#, &config);
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(
        std::fs::read_to_string(dir.join("F-Test.xml")).unwrap(),
        PERF_XML,
        "the file is back to its baseline"
    );
    // Answers with the whole table: a reset moves every scalar in the class.
    assert!(body_json(&resp)["classes"][0]["cars"].is_array());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_reset_without_a_baseline_is_refused() {
    let (dir, config) = make_perf_fixture();
    let resp = post_perf("/api/car-performance/reset", r#"{"class":"F-Test"}"#, &config);
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert_eq!(
        std::fs::read_to_string(dir.join("F-Test.xml")).unwrap(),
        PERF_XML,
        "nothing was emptied"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_set_baseline_adopts_the_file_as_it_stands() {
    let (dir, config) = make_perf_fixture();
    patch_perf(
        r#"{"class":"F-Test","team":"AGS","power_scalar":1.10,"weight_scalar":0.97,"drag_scalar":0.95}"#,
        &config,
    );
    let tuned = std::fs::read_to_string(dir.join("F-Test.xml")).unwrap();

    let resp = post_perf(
        "/api/car-performance/baseline",
        r#"{"class":"F-Test"}"#,
        &config,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(
        std::fs::read_to_string(dir.join("F-Test.xml.bak")).unwrap(),
        tuned,
        "the baseline is now the tuned file, not the shipped one"
    );

    // And a later reset comes back here rather than to what shipped.
    patch_perf(
        r#"{"class":"F-Test","team":"Williams","power_scalar":0.95,"weight_scalar":1.0,"drag_scalar":1.0}"#,
        &config,
    );
    post_perf("/api/car-performance/reset", r#"{"class":"F-Test"}"#, &config);
    assert_eq!(std::fs::read_to_string(dir.join("F-Test.xml")).unwrap(), tuned);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_baseline_rejects_an_unknown_class() {
    let (dir, config) = make_perf_fixture();
    for route in [
        "/api/car-performance/baseline",
        "/api/car-performance/reset",
    ] {
        let resp = post_perf(route, r#"{"class":"F-Nope"}"#, &config);
        assert!(
            !status_line(&resp).contains("200"),
            "{route} accepted a class that does not exist: {resp}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_car_performance_rejects_out_of_range_scalar() {
    let (dir, config) = make_perf_fixture();
    let resp = patch_perf(
        r#"{"class":"F-Test","team":"AGS","power_scalar":10.8,"weight_scalar":1.0,"drag_scalar":1.0}"#,
        &config,
    );
    assert!(resp.starts_with("HTTP/1.1 400"), "{resp}");
    // The file is untouched — validation runs before it is opened.
    assert_eq!(
        std::fs::read_to_string(dir.join("F-Test.xml")).unwrap(),
        PERF_XML
    );
    assert!(!dir.join("F-Test.xml.bak").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_car_performance_rejects_class_name_escaping_the_folder() {
    let (dir, config) = make_perf_fixture();
    let resp = patch_perf(
        r#"{"class":"../F-Test","team":"AGS","power_scalar":1.0,"weight_scalar":1.0,"drag_scalar":1.0}"#,
        &config,
    );
    assert!(resp.starts_with("HTTP/1.1 400"), "{resp}");
    assert!(resp.contains("invalid class name"), "{resp}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_car_performance_unknown_team_and_class_are_errors() {
    let (dir, config) = make_perf_fixture();
    let team = patch_perf(
        r#"{"class":"F-Test","team":"Ferrari","power_scalar":1.0,"weight_scalar":1.0,"drag_scalar":1.0}"#,
        &config,
    );
    assert!(team.starts_with("HTTP/1.1 400"), "{team}");
    assert!(team.contains("Ferrari"), "{team}");

    let class = patch_perf(
        r#"{"class":"F-Missing","team":"AGS","power_scalar":1.0,"weight_scalar":1.0,"drag_scalar":1.0}"#,
        &config,
    );
    assert!(class.starts_with("HTTP/1.1 404"), "{class}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_car_performance_without_configured_folder() {
    let (store, data_path) = make_test_store();
    let body = r#"{"class":"F-Test","team":"AGS","power_scalar":1.0,"weight_scalar":1.0,"drag_scalar":1.0}"#;
    let resp = patch(store, data_path, "/api/car-performance", body.as_bytes());
    assert!(resp.starts_with("HTTP/1.1 400"), "{resp}");
    assert!(resp.contains("Custom AI Drivers folder"), "{resp}");
}

// ── /api/driver-performance ───────────────────────────────────────────────────

fn get_with_config(path: &str, config: &std::path::Path) -> String {
    let (store, data_path) = make_test_store();
    call_with_config(
        store,
        data_path,
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes(),
        Some(config.to_path_buf()),
    )
}

fn patch_driver_perf(body: &str, config: &std::path::Path) -> String {
    let (store, data_path) = make_test_store();
    let mut req = format!(
        "PATCH /api/driver-performance HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    call_with_config(store, data_path, req, Some(config.to_path_buf()))
}

#[test]
fn test_route_get_driver_performance_lists_entries_with_the_editable_attrs() {
    let (dir, config) = make_perf_fixture();
    let resp = get_with_config("/api/driver-performance", &config);
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    // The column list comes from the server so it cannot drift from the writer's allowlist.
    let attrs = v["attrs"].as_array().unwrap();
    assert_eq!(
        attrs.len(),
        ams2_championship::custom_ai::DRIVER_ATTRS.len()
    );
    assert_eq!(attrs[0], "race_skill");

    let drivers = v["classes"][0]["drivers"].as_array().unwrap();
    assert_eq!(drivers.len(), 2);
    assert_eq!(drivers[0]["driver"], "Nigel Mansell");
    assert_eq!(drivers[0]["team"], "Williams");
    assert_eq!(drivers[0]["index"], 0);
    assert!(drivers[0]["tracks"].is_null());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_get_driver_performance_without_configured_folder() {
    let (store, data_path) = make_test_store();
    let resp = get(store, data_path, "/api/driver-performance");
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    assert_eq!(v["classes"].as_array().unwrap().len(), 0);
    // The attribute list is still served, so the tab can explain itself with no files present.
    assert!(!v["attrs"].as_array().unwrap().is_empty());
}

#[test]
fn test_route_patch_driver_performance_writes_and_returns_the_row() {
    let (dir, config) = make_perf_fixture();
    let resp = patch_driver_perf(
        r#"{"class":"F-Test","index":1,"driver":"Ivan Capelli","field":"race_skill","value":0.72}"#,
        &config,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    assert_eq!(v["driver"], "Ivan Capelli");
    assert_eq!(v["attrs"]["race_skill"], 0.72);

    let drivers = ams2_championship::custom_ai::parse_driver_attributes(&dir.join("F-Test.xml"));
    assert_eq!(drivers[1].attrs.get("race_skill"), Some(&0.72));
    // The other entry is untouched, and the original file is preserved.
    assert_eq!(drivers[0].attrs.get("race_skill"), None);
    assert!(dir.join("F-Test.xml.bak").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_driver_performance_rejects_a_stale_index() {
    let (dir, config) = make_perf_fixture();
    let resp = patch_driver_perf(
        r#"{"class":"F-Test","index":0,"driver":"Ivan Capelli","field":"race_skill","value":0.72}"#,
        &config,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(resp.contains("Nigel Mansell"), "{resp}");
    assert!(!dir.join("F-Test.xml.bak").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_driver_performance_rejects_a_non_attribute_field() {
    let (dir, config) = make_perf_fixture();
    // The car scalars have their own route; they are not editable through this one.
    let resp = patch_driver_perf(
        r#"{"class":"F-Test","index":0,"driver":"Nigel Mansell","field":"power_scalar","value":1.0}"#,
        &config,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(resp.contains("power_scalar"), "{resp}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_driver_performance_rejects_class_name_escaping_the_folder() {
    let (dir, config) = make_perf_fixture();
    let resp = patch_driver_perf(
        r#"{"class":"../F-Test","index":0,"driver":"Nigel Mansell","field":"race_skill","value":0.5}"#,
        &config,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(resp.contains("invalid class name"), "{resp}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_driver_performance_flags_entries_with_no_installed_livery() {
    let (dir, config) = make_perf_fixture();
    // A livery manifest covering Mansell but not Capelli, in the layout the game uses.
    let overrides = dir
        .join("Vehicles")
        .join("Textures")
        .join("CustomLiveries")
        .join("Overrides")
        .join("some_model");
    std::fs::create_dir_all(&overrides).unwrap();
    std::fs::write(
        overrides.join("some_model.xml"),
        r#"<USER_OVERRIDES>
        <LIVERY_OVERRIDE LIVERY="1" NAME="Williams #5 N. Mansell" BASELIVERY="Default" />
        </USER_OVERRIDES>"#,
    )
    .unwrap();
    // The route derives the install root two levels up from the Custom AI folder, so point it
    // at a nested folder the way a real install nests UserData/CustomAIDrivers.
    let ai = dir.join("UserData").join("CustomAIDrivers");
    std::fs::create_dir_all(&ai).unwrap();
    std::fs::copy(dir.join("F-Test.xml"), ai.join("F-Test.xml")).unwrap();
    std::fs::write(
        &config,
        format!(
            "{{\"custom_ai_dir\":{}}}",
            serde_json::to_string(&ai.display().to_string()).unwrap()
        ),
    )
    .unwrap();

    let resp = get_with_config("/api/driver-performance", &config);
    assert!(status_line(&resp).contains("200"), "{resp}");
    let drivers = body_json(&resp)["classes"][0]["drivers"].clone();
    assert_eq!(drivers[0]["driver"], "Nigel Mansell");
    assert_eq!(drivers[0]["phantom"], false);
    assert_eq!(drivers[1]["driver"], "Ivan Capelli");
    assert_eq!(drivers[1]["phantom"], true);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_driver_performance_leaves_phantom_unknown_without_manifests() {
    let (dir, config) = make_perf_fixture();
    let resp = get_with_config("/api/driver-performance", &config);
    let drivers = body_json(&resp)["classes"][0]["drivers"].clone();
    // No Overrides folder at all: nothing is claimed either way.
    assert!(drivers[0]["phantom"].is_null(), "{drivers}");
    assert!(drivers[1]["phantom"].is_null(), "{drivers}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_patch_config_omitting_rating_tuning_leaves_it_alone() {
    // `config_body` above sends no rating fields at all, exactly like a stale form would. Each
    // must fall back to its default rather than to zero — a starting_rating of 0 would drop
    // every driver to the back of the grid without anyone asking for it.
    let (store, path) = make_saves_dir("cfg_rating_absent");
    let resp = patch(store, path.clone(), "/api/config", &config_body("null"));
    assert!(status_line(&resp).contains("200"), "got {resp}");
    let v = body_json(&resp);
    assert_eq!(v["config"]["starting_rating"], 50.0);
    assert_eq!(v["config"]["rating_strictness"], 0.0);
    assert_eq!(v["config"]["rating_half_life"], 10.0);
    assert_eq!(v["config"]["eligibility_gates"], "both");
    assert_eq!(v["config"]["count_retirements"], true);
    assert_eq!(v["config"]["retirement_min_laps_down"], 3);
    assert_eq!(v["config"]["retirement_distance_pct"], 90.0);
    assert_eq!(v["config"]["hide_locked_teams"], false);

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_patch_config_clamps_rating_tuning() {
    let (store, path) = make_saves_dir("cfg_rating_clamp");
    let body = br#"{"port":8080,"host":"127.0.0.1","poll_ms":200,"record_practice":true,
        "record_qualify":true,"record_race":true,"show_track_map":true,
        "track_map_max_points":5000,"saves_dir":null,
        "starting_rating":250.0,"rating_strictness":-400.0,"rating_half_life":-3.0,
        "eligibility_gates":"grid","count_retirements":false,"hide_locked_teams":true,
        "retirement_min_laps_down":900,"retirement_distance_pct":400.0}"#;
    let resp = patch(store, path.clone(), "/api/config", body);
    assert!(status_line(&resp).contains("200"), "got {resp}");
    let v = body_json(&resp);
    // Clamped on the way in, so what is persisted is already within range and the form that
    // reads it back cannot show a value the rating would refuse to use.
    assert_eq!(v["config"]["starting_rating"], 100.0);
    assert_eq!(v["config"]["rating_strictness"], -50.0);
    assert_eq!(v["config"]["rating_half_life"], 0.0);
    assert_eq!(v["config"]["eligibility_gates"], "grid");
    assert_eq!(v["config"]["count_retirements"], false);
    assert_eq!(v["config"]["retirement_min_laps_down"], 50, "capped at a whole race");
    assert_eq!(v["config"]["retirement_distance_pct"], 100.0, "a share cannot exceed the whole");
    assert_eq!(v["config"]["hide_locked_teams"], true);

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

// ── resolve_live_teams (the /api/live-teams lookup) ───────────────────────────
//
// Two rosters for the same fictional season, as the historic Custom AI packs ship them: a core
// field, and a "full" variant adding the optional entries some tracks used. Both name the same
// regulars, so both match any grid from that season — which is why the live lookup must not try
// to pick between them by counting drivers, and follows the active championship instead.

const CORE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<custom_ai_drivers>
    <driver livery_name="Lotus #1 A. Alpha"><name>Alan Alpha</name></driver>
    <driver livery_name="Lotus #2 B. Bravo"><name>Ben Bravo</name></driver>
    <driver livery_name="Brabham #7 C. Charlie"><name>Carl Charlie</name></driver>
</custom_ai_drivers>
"#;

const FULL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<custom_ai_drivers>
    <driver livery_name="Lotus #1 A. Alpha"><name>Alan Alpha</name></driver>
    <driver livery_name="Lotus #2 B. Bravo"><name>Ben Bravo</name></driver>
    <driver livery_name="Brabham #7 C. Charlie"><name>Carl Charlie</name></driver>
    <driver livery_name="March #16 D. Delta"><name>Dan Delta</name></driver>
</custom_ai_drivers>
"#;

/// A temp Custom AI Drivers folder holding both rosters.
fn make_live_teams_dir() -> std::path::PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_live_teams_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("core.xml"), CORE_XML).unwrap();
    std::fs::write(dir.join("full.xml"), FULL_XML).unwrap();
    dir
}

/// The season being raced (Active) plus a second, larger roster left in Progress.
fn two_seasons() -> Vec<Championship> {
    let mut racing = make_champ("c1");
    racing.custom_ai_file = Some("core.xml".into());
    racing.player_team = Some("Brabham".into());
    racing.status = ChampionshipStatus::Active;
    let mut other = make_champ("c2");
    other.custom_ai_file = Some("full.xml".into());
    other.player_team = None;
    other.status = ChampionshipStatus::Progress;
    vec![racing, other]
}

#[test]
fn test_live_teams_come_from_the_active_championship() {
    let dir = make_live_teams_dir();
    let out = resolve_live_teams(&dir, &two_seasons(), &[], false);

    assert_eq!(out.player_team.as_deref(), Some("Brabham"));
    assert_eq!(
        out.teams.get("Carl Charlie").map(String::as_str),
        Some("Brabham")
    );
    // The regression this lookup exists for: "full" names one more driver, so a roster picked by
    // name count would have won with it and carried its empty player team onto the player's row.
    assert!(
        !out.teams.contains_key("Dan Delta"),
        "the roster must be the active championship's, not the larger variant's"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_live_teams_follow_the_active_flag_when_it_moves() {
    // Switching which season is active in the Manage tab is the whole control surface here.
    let dir = make_live_teams_dir();
    let mut champs = two_seasons();
    champs[0].status = ChampionshipStatus::Progress;
    champs[1].status = ChampionshipStatus::Active;
    champs[1].player_team = Some("March".into());

    let out = resolve_live_teams(&dir, &champs, &[], false);
    assert_eq!(out.player_team.as_deref(), Some("March"));
    assert_eq!(
        out.teams.get("Dan Delta").map(String::as_str),
        Some("March")
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_live_teams_empty_when_no_championship_is_active() {
    // Nothing is guessed from the other championships — the grid shows AMS2's car names until
    // one is marked active.
    let dir = make_live_teams_dir();
    let mut champs = two_seasons();
    champs[0].status = ChampionshipStatus::Progress;

    let out = resolve_live_teams(&dir, &champs, &[], false);
    assert!(out.teams.is_empty());
    assert_eq!(out.player_team, None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_live_teams_empty_when_the_active_championship_has_no_roster() {
    let dir = make_live_teams_dir();
    let mut champs = two_seasons();
    champs[0].custom_ai_file = None;

    let out = resolve_live_teams(&dir, &champs, &[], false);
    assert!(out.teams.is_empty());
    assert_eq!(
        out.player_team, None,
        "a player team is only settable alongside a roster, so it cannot outlive one"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_live_teams_treats_a_blank_player_team_as_unset() {
    let dir = make_live_teams_dir();
    let mut champs = two_seasons();
    champs[0].player_team = Some("   ".into());

    let out = resolve_live_teams(&dir, &champs, &[], false);
    assert!(!out.teams.is_empty(), "the roster still resolves");
    assert_eq!(out.player_team, None);

    let _ = std::fs::remove_dir_all(&dir);
}

/// An install tree whose class registry names only `F-Vintage_Gen2`, with a per-track variant
/// beside it that AMS2 would ignore, plus a config.json pointing at the Custom AI folder.
fn make_class_route_fixture() -> (std::path::PathBuf, std::path::PathBuf) {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ams2_class_route_{ns}"));
    let ai_dir = root.join("UserData").join("CustomAIDrivers");
    let hud = root.join("GUI").join("HUD_1_6");
    std::fs::create_dir_all(&ai_dir).unwrap();
    std::fs::create_dir_all(&hud).unwrap();
    std::fs::write(
        hud.join("HUD_ColoursDefs.xml"),
        r##"<Colours><Colour name="F-Vintage_Gen2" value="#fff" /></Colours>"##,
    )
    .unwrap();
    std::fs::write(ai_dir.join("F-Vintage_Gen2.xml"), "<custom_ai_drivers/>").unwrap();
    std::fs::write(
        ai_dir.join("F-Vintage_Gen2_03Nordschleiffe.xml"),
        "<custom_ai_drivers/>",
    )
    .unwrap();

    let config = root.join("config.json");
    std::fs::write(
        &config,
        format!(
            "{{\"custom_ai_dir\":{}}}",
            serde_json::to_string(&ai_dir.display().to_string()).unwrap()
        ),
    )
    .unwrap();
    (root, config)
}

#[test]
fn test_route_custom_ai_files_lists_only_names_ams2_reads() {
    let (root, config) = make_class_route_fixture();
    let (store, data_path) = make_test_store();
    let resp = call_with_config(
        store,
        data_path,
        b"GET /api/custom-ai-files HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    assert!(resp.starts_with("HTTP/1.1 200 OK"), "{resp}");
    let body = resp.split("\r\n\r\n").nth(1).unwrap_or("");
    let files: Vec<String> = serde_json::from_str(body).unwrap();
    assert_eq!(files, vec!["F-Vintage_Gen2.xml"], "{body}");

    std::fs::remove_dir_all(&root).ok();
}

// ── GET /api/championships/:id/offers ─────────────────────────────────────────

/// A roster with two teams of real pace scalars, so the offers route has something to rate
/// against rather than falling through to `rated: false`.
const OFFER_ROSTER: &str = r##"<custom_ai_drivers>
    <driver livery_name="1986 Williams #5 - N. Mansell" power_scalar="1.00" weight_scalar="1.00" drag_scalar="1.00"><name>Nigel Mansell</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Williams #6 - N. Piquet" power_scalar="1.00" weight_scalar="1.00" drag_scalar="1.00"><name>Nelson Piquet</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Osella #21 - P. Ghinzani" power_scalar="0.90" weight_scalar="1.00" drag_scalar="1.00"><name>Piercarlo Ghinzani</name><race_skill>0.55</race_skill></driver>
    <driver livery_name="1986 Osella #22 - A. Berg" power_scalar="0.90" weight_scalar="1.00" drag_scalar="1.00"><name>Allan Berg</name><race_skill>0.54</race_skill></driver>
</custom_ai_drivers>"##;

/// A Custom AI folder holding [`OFFER_ROSTER`] as `F-Classic_Gen1.xml`, plus a config.json
/// pointing at it. `contracts_enabled` follows the flag.
fn make_offer_fixture(enabled: bool) -> (std::path::PathBuf, std::path::PathBuf) {
    offer_fixture(enabled, "")
}

/// [`make_offer_fixture`] with pay-driver seats switched off, so a team out of reach makes no
/// offer at all rather than naming a price.
fn make_offer_fixture_no_buy_in() -> (std::path::PathBuf, std::path::PathBuf) {
    offer_fixture(true, r#","contract_buy_in_per_point":0"#)
}

/// A five-team grid, which is the smallest that puts a *locked* team on the market: only the
/// back third sells, and the very slowest car is always opened by the rating's own floor rule.
/// Coloni sits one place off the back and just out of reach, so it is the pay-driver seat.
const PAY_DRIVER_ROSTER: &str = r##"<custom_ai_drivers>
    <driver livery_name="1986 Williams #5 - N. Mansell" power_scalar="1.00" weight_scalar="1.00" drag_scalar="1.00"><name>Nigel Mansell</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Williams #6 - N. Piquet" power_scalar="1.00" weight_scalar="1.00" drag_scalar="1.00"><name>Nelson Piquet</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Brabham #7 - R. Patrese" power_scalar="0.97" weight_scalar="1.00" drag_scalar="1.00"><name>Riccardo Patrese</name><race_skill>0.85</race_skill></driver>
    <driver livery_name="1986 Brabham #8 - D. Warwick" power_scalar="0.97" weight_scalar="1.00" drag_scalar="1.00"><name>Derek Warwick</name><race_skill>0.85</race_skill></driver>
    <driver livery_name="1986 Lotus #11 - J. Dumfries" power_scalar="0.94" weight_scalar="1.00" drag_scalar="1.00"><name>Johnny Dumfries</name><race_skill>0.70</race_skill></driver>
    <driver livery_name="1986 Lotus #12 - A. Senna" power_scalar="0.94" weight_scalar="1.00" drag_scalar="1.00"><name>Ayrton Senna</name><race_skill>0.70</race_skill></driver>
    <driver livery_name="1986 Coloni #31 - N. Larini" power_scalar="0.91" weight_scalar="1.00" drag_scalar="1.00"><name>Nicola Larini</name><race_skill>0.75</race_skill></driver>
    <driver livery_name="1986 Coloni #32 - G. Tarquini" power_scalar="0.91" weight_scalar="1.00" drag_scalar="1.00"><name>Gabriele Tarquini</name><race_skill>0.75</race_skill></driver>
    <driver livery_name="1986 Osella #21 - P. Ghinzani" power_scalar="0.88" weight_scalar="1.00" drag_scalar="1.00"><name>Piercarlo Ghinzani</name><race_skill>0.55</race_skill></driver>
    <driver livery_name="1986 Osella #22 - A. Berg" power_scalar="0.88" weight_scalar="1.00" drag_scalar="1.00"><name>Allan Berg</name><race_skill>0.54</race_skill></driver>
</custom_ai_drivers>"##;

/// The seat on [`PAY_DRIVER_ROSTER`] that money — and only money — opens.
const PAY_SEAT: &str = "Coloni";

fn pay_driver_fixture(extra: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let (root, config) = offer_fixture(true, extra);
    let ai_dir = root.join("CustomAIDrivers");
    std::fs::write(ai_dir.join("F-Classic_Gen1.xml"), PAY_DRIVER_ROSTER).unwrap();
    (root, config)
}

/// `extra` is appended to the config object verbatim, leading comma and all.
fn offer_fixture(enabled: bool, extra: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ams2_offers_route_{ns}"));
    let ai_dir = root.join("CustomAIDrivers");
    std::fs::create_dir_all(&ai_dir).unwrap();
    std::fs::write(ai_dir.join("F-Classic_Gen1.xml"), OFFER_ROSTER).unwrap();

    let config = root.join("config.json");
    std::fs::write(
        &config,
        format!(
            "{{\"custom_ai_dir\":{},\"contracts_enabled\":{enabled}{extra}}}",
            serde_json::to_string(&ai_dir.display().to_string()).unwrap()
        ),
    )
    .unwrap();
    (root, config)
}

fn rated_champ(id: &str) -> Championship {
    Championship {
        custom_ai_file: Some("F-Classic_Gen1.xml".into()),
        ..make_champ(id)
    }
}

fn offers_resp(champ: Championship, config: &std::path::Path) -> String {
    let (store, data_path) = make_sp_store();
    let id = champ.id.clone();
    store.write().unwrap().championships.push(champ);
    call_with_config(
        store,
        data_path,
        format!("GET /api/championships/{id}/offers HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .into_bytes(),
        Some(config.to_path_buf()),
    )
}

#[test]
fn test_route_offers_unknown_championship_is_404() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/championships/nope/offers");
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_offers_unrated_without_custom_ai_file() {
    let (store, path) = make_test_store();
    store.write().unwrap().championships.push(make_champ("c1"));
    let resp = get(store, path.clone(), "/api/championships/c1/offers");
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    // No roster means no teams and no car pace, so there is nothing to offer — but the season is
    // still open, and the config switch is still reported.
    assert_eq!(v["rated"], false);
    assert_eq!(v["open"], true);
    assert_eq!(v["enabled"], false, "contracts default off");
    assert_eq!(v["offers"].as_array().unwrap().len(), 0);
    assert!(v["signed"].is_null());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_offers_lists_terms_for_a_rated_season() {
    let (root, config) = make_offer_fixture(true);
    let resp = offers_resp(rated_champ("c1"), &config);
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);

    assert_eq!(v["enabled"], true);
    assert_eq!(v["rated"], true);
    assert_eq!(v["open"], true);
    let offers = v["offers"].as_array().unwrap();
    assert!(!offers.is_empty(), "{resp}");
    // Every offer carries the terms the UI needs without a second request.
    for o in offers {
        assert!(o["salary"].as_i64().unwrap() > 0);
        assert!(o["seasons"].is_null(), "deals are single-season: {o}");
        assert!(!o["team"].as_str().unwrap().is_empty());
        // Two kinds, and only two: either the team pays the driver or the driver pays the team.
        let kind = o["kind"].as_str().unwrap();
        assert!(["paid", "pay"].contains(&kind), "{o}");
        assert!(o["renewal"].is_boolean(), "{o}");
        // A seat the rating earned is never priced. A bought one is — unless the lockout
        // guarantee has discounted it to what the career holds, which on this fresh career is
        // nothing.
        if kind == "paid" {
            assert_eq!(o["buy_in"].as_i64().unwrap(), 0, "{o}");
        }
    }
    // A fresh career has no seat behind it and nothing banked.
    assert!(v["standing"]["incumbent"].is_null());
    assert_eq!(v["balance"], 0);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_offers_stay_open_for_a_team_picked_without_a_contract() {
    let (root, config) = make_offer_fixture(true);
    let champ = Championship {
        player_team: Some("Osella".into()),
        planned_rounds: None,
        ..rated_champ("c1")
    };
    let v = body_json(&offers_resp(champ, &config));
    // A team set through the picker is not a commitment — `PATCH` may still change it before
    // the first session, so signing may too. Only a contract or a raced session closes a season.
    assert_eq!(v["open"], true);
    assert!(v["signed"].is_null());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_offers_closed_once_the_season_has_started() {
    let (root, config) = make_offer_fixture(true);
    let champ = Championship {
        rounds: vec![Round {
            session_ids: vec!["s1".into()],
        }],
        ..rated_champ("c1")
    };
    assert_eq!(body_json(&offers_resp(champ, &config))["open"], false);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_offers_are_advisory_in_a_career_that_does_not_sign() {
    // Contracts follow the career kind, not a config switch. An unset career still sees what
    // the grid would offer, but `enabled` says it cannot act on it.
    let (root, config) = make_offer_fixture(true);
    let (store, data_path) = make_test_store();
    store.write().unwrap().championships.push(rated_champ("c1"));
    let resp = call_with_config(
        store,
        data_path,
        b"GET /api/championships/c1/offers HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    let v = body_json(&resp);
    assert_eq!(v["enabled"], false, "{resp}");
    assert!(!v["offers"].as_array().unwrap().is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_offers_shows_a_signed_deal() {
    let (root, config) = make_offer_fixture(true);
    let (store, data_path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.championships.push(rated_champ("c1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: "Osella".into(),
            signed_at: 42,
            salary: 500_000,
            objective: Some(4),
            bought_for: 0,
            settled: None,
        });
    }
    let resp = call_with_config(
        store,
        data_path,
        b"GET /api/championships/c1/offers HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    let v = body_json(&resp);
    assert_eq!(v["signed"]["team"], "Osella");
    assert_eq!(v["signed"]["salary"], 500_000);
    assert_eq!(v["open"], false, "a signed season takes no more offers");
    std::fs::remove_dir_all(&root).ok();
}

// ── GET /api/career/finances ──────────────────────────────────────────────────

/// A two-car race the player won, flagged as the recorder writes it.
fn win_session(id: &str) -> RecordedSession {
    let driver = |name: &str, pos: u32, is_player: bool| SessionResult {
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
    RecordedSession {
        id: id.into(),
        recorded_at: 1,
        track: "Monza".into(),
        track_variation: String::new(),
        car_name: String::new(),
        car_class: "F-Classic_Gen1".into(),
        session_type: 5,
        results: vec![driver("Nightrat", 1, true), driver("Berg", 2, false)],
        lap_chart: vec![],
    }
}

#[test]
fn test_route_finances_empty_career() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/career/finances");
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["balance"], 0);
    assert_eq!(v["earned"], 0);
    assert_eq!(v["spent"], 0);
    assert_eq!(v["enabled"], false);
    assert_eq!(v["seasons"].as_array().unwrap().len(), 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_finances_pays_a_completed_season() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.status = ChampionshipStatus::Final;
        champ.rounds = vec![Round {
            session_ids: vec!["s1".into()],
        }];
        data.championships.push(champ);
        data.sessions.push(win_session("s1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: "Osella".into(),
            signed_at: 1,
            salary: 500_000,
            objective: Some(3),
            bought_for: 0,
            settled: None,
        });
    }
    let resp = get(store, path.clone(), "/api/career/finances");
    let v = body_json(&resp);

    let season = &v["seasons"][0];
    assert_eq!(season["team"], "Osella");
    assert_eq!(season["complete"], true);
    assert_eq!(season["position"], 1);
    assert_eq!(season["salary"], 500_000);
    assert_eq!(season["objective_met"], true);
    assert!(season["prize"].as_i64().unwrap() > 0);
    assert_eq!(
        v["balance"].as_i64().unwrap(),
        season["salary"].as_i64().unwrap() + season["prize"].as_i64().unwrap()
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_finances_withholds_an_unfinished_season() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.championships.push(make_champ("c1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: "Osella".into(),
            signed_at: 1,
            salary: 500_000,
            objective: None,
            bought_for: 0,
            settled: None,
        });
    }
    let v = body_json(&get(store, path.clone(), "/api/career/finances"));
    assert_eq!(v["seasons"][0]["complete"], false);
    assert_eq!(v["seasons"][0]["salary"], 0);
    assert_eq!(v["balance"], 0);
    let _ = std::fs::remove_file(&path);
}


// ── POST / DELETE /api/championships/:id/sign ─────────────────────────────────

/// Sends `request` against a store holding `champ`, and hands back both the response and the
/// store so the caller can inspect what was actually written.
fn sign_call(
    champ: Championship,
    config: &std::path::Path,
    method: &str,
    body: &str,
) -> (String, ams2_championship::data_store::SharedStore) {
    let (store, data_path) = make_sp_store();
    let id = champ.id.clone();
    store.write().unwrap().championships.push(champ);
    let mut req = format!(
        "{method} /api/championships/{id}/sign HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    let resp = call_with_config(store.clone(), data_path, req, Some(config.to_path_buf()));
    (resp, store)
}

/// The team the offer fixture's rating can actually reach — the slowest car on its two-team grid.
const OPEN_SEAT: &str = "Osella";

#[test]
fn test_route_sign_records_the_deal_and_takes_the_seat() {
    let (root, config) = make_offer_fixture(true);
    let (resp, store) = sign_call(
        rated_champ("c1"),
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}"}}"#),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);

    // The response carries both halves: what was agreed, and the championship it applies to.
    assert_eq!(v["contract"]["team"], OPEN_SEAT);
    assert_eq!(v["contract"]["champ_id"], "c1");
    assert!(v["contract"]["salary"].as_i64().unwrap() > 0);
    assert!(v["contract"]["signed_at"].as_u64().unwrap() > 0);
    assert_eq!(v["championship"]["player_team"], OPEN_SEAT);

    let data = store.read().unwrap();
    assert_eq!(data.contracts.len(), 1);
    assert_eq!(
        data.championships[0].player_team.as_deref(),
        Some(OPEN_SEAT)
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_terms_are_the_servers_not_the_callers() {
    // A caller naming its own salary must not get it: terms are regenerated from the grid.
    let (root, config) = make_offer_fixture(true);
    let (resp, _) = sign_call(
        rated_champ("c1"),
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}","salary":999999999,"seasons":99}}"#),
    );
    let v = body_json(&resp);
    assert_ne!(v["contract"]["salary"], 999_999_999i64);
    // `seasons` is not a term any more; a caller sending one is simply ignored.
    assert!(v["contract"]["seasons"].is_null(), "{resp}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_matches_the_team_name_case_insensitively() {
    let (root, config) = make_offer_fixture(true);
    let (resp, _) = sign_call(
        rated_champ("c1"),
        &config,
        "POST",
        r#"{"team":"  osella  "}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    // The roster's own spelling is what gets recorded, not the caller's.
    assert_eq!(body_json(&resp)["contract"]["team"], OPEN_SEAT);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_refuses_a_team_that_is_not_offering() {
    // With pay-driver seats off, the quick car on this grid is simply not available, and the
    // refusal is the useful half of the answer: what it would take.
    let (root, config) = make_offer_fixture_no_buy_in();
    let (resp, store) = sign_call(rated_champ("c1"), &config, "POST", r#"{"team":"Williams"}"#);
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(
        resp.contains("driver rating"),
        "should quote the bar: {resp}"
    );
    assert!(
        store.read().unwrap().contracts.is_empty(),
        "nothing written"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_refuses_a_seat_the_career_cannot_afford() {
    // Coloni will take sponsorship. A career that has earned nothing has none to bring, and the
    // balance is derived from results — naming a price in the request changes nothing.
    let (root, config) = pay_driver_fixture("");
    let (resp, store) = sign_call(
        rated_champ("c1"),
        &config,
        "POST",
        &format!(r#"{{"team":"{PAY_SEAT}","buy_in":1}}"#),
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("in sponsorship"), "{resp}");
    assert!(
        store.read().unwrap().contracts.is_empty(),
        "nothing written"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_buys_a_seat_the_career_can_afford() {
    // A nominal sponsorship rate against a career that has banked a title.
    let (root, config) = pay_driver_fixture(r#","contract_buy_in_per_point":1"#);
    let (store, data_path) = make_sp_store();
    {
        let mut data = store.write().unwrap();
        data.championships.push(rated_champ("c1"));

        // A completed, won season under a contract, which is what puts money in the bank.
        let mut past = rated_champ("past");
        past.status = ChampionshipStatus::Final;
        past.rounds = vec![Round {
            session_ids: vec!["s1".into()],
        }];
        data.championships.push(past);
        data.sessions.push(win_session("s1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "past".into(),
            team: OPEN_SEAT.into(),
            signed_at: 1,
            salary: 500_000,
            objective: None,
            bought_for: 0,
            settled: None,
        });
    }
    let body = format!(r#"{{"team":"{PAY_SEAT}"}}"#);
    let mut req = format!(
        "POST /api/championships/c1/sign HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    let resp = call_with_config(store.clone(), data_path, req, Some(config));

    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    assert_eq!(v["contract"]["team"], PAY_SEAT);
    assert!(
        v["contract"]["bought_for"].as_i64().unwrap() > 0,
        "the sponsorship brought is on the record: {resp}"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_refuses_a_team_not_on_the_grid() {
    let (root, config) = make_offer_fixture(true);
    let (resp, _) = sign_call(rated_champ("c1"), &config, "POST", r#"{"team":"Ferrari"}"#);
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("not a team on this grid"), "{resp}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_refused_in_a_career_that_does_not_sign() {
    // Only a singleplayer career takes seats by contract. Recording a salary in a multiplayer or
    // unset one would put money in a ledger the user never opted into.
    let (root, config) = make_offer_fixture(true);
    for mode in [
        ams2_championship::data_store::CareerMode::Unset,
        ams2_championship::data_store::CareerMode::Multiplayer,
    ] {
        let (store, data_path) = make_test_store();
        {
            let mut data = store.write().unwrap();
            data.mode = mode;
            data.championships.push(rated_champ("c1"));
        }
        let body = format!(r#"{{"team":"{OPEN_SEAT}"}}"#);
        let mut req = format!(
            "POST /api/championships/c1/sign HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        req.extend_from_slice(body.as_bytes());
        let resp = call_with_config(store.clone(), data_path, req, Some(config.clone()));

        assert!(status_line(&resp).contains("409"), "{mode:?}: {resp}");
        assert!(resp.contains("singleplayer career"), "{resp}");
        assert!(store.read().unwrap().contracts.is_empty());
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_refused_once_the_season_has_started() {
    let (root, config) = make_offer_fixture(true);
    let champ = Championship {
        rounds: vec![Round {
            session_ids: vec!["s1".into()],
        }],
        ..rated_champ("c1")
    };
    let (resp, store) = sign_call(
        champ,
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}"}}"#),
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("already started"), "{resp}");
    assert!(store.read().unwrap().contracts.is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_refused_without_a_roster() {
    let (root, config) = make_offer_fixture(true);
    let (resp, _) = sign_call(
        make_champ("c1"),
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}"}}"#),
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("Custom AI Drivers file"), "{resp}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_refuses_a_second_deal_for_one_season() {
    let (root, config) = make_offer_fixture(true);
    let (store, data_path) = make_sp_store();
    {
        let mut data = store.write().unwrap();
        data.championships.push(rated_champ("c1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: OPEN_SEAT.into(),
            signed_at: 1,
            salary: 1,
            objective: None,
            bought_for: 0,
            settled: None,
        });
    }
    let body = format!(r#"{{"team":"{OPEN_SEAT}"}}"#);
    let mut req = format!(
        "POST /api/championships/c1/sign HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    let resp = call_with_config(store.clone(), data_path, req, Some(config));

    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("already signed"), "{resp}");
    assert_eq!(store.read().unwrap().contracts.len(), 1, "no duplicate row");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_unknown_championship_is_404() {
    let (root, config) = make_offer_fixture(true);
    let (store, data_path) = make_test_store();
    let body = r#"{"team":"Osella"}"#;
    let mut req = format!(
        "POST /api/championships/nope/sign HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    let resp = call_with_config(store, data_path, req, Some(config));
    assert!(status_line(&resp).contains("404"), "{resp}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_rejects_a_malformed_body() {
    let (root, config) = make_offer_fixture(true);
    let (resp, _) = sign_call(rated_champ("c1"), &config, "POST", "{}");
    assert!(status_line(&resp).contains("400"), "{resp}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_replaces_a_team_picked_directly() {
    // The old picker is still there; signing over the top of it is the same move `PATCH` allows
    // before the first session, so it must not be a dead end.
    let (root, config) = make_offer_fixture(true);
    let champ = Championship {
        player_team: Some("Williams".into()),
        planned_rounds: None,
        ..rated_champ("c1")
    };
    let (resp, store) = sign_call(
        champ,
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}"}}"#),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(
        store.read().unwrap().championships[0]
            .player_team
            .as_deref(),
        Some(OPEN_SEAT)
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_closes_the_offer_list() {
    let (root, config) = make_offer_fixture(true);
    let (resp, store) = sign_call(
        rated_champ("c1"),
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}"}}"#),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let (_, data_path) = make_test_store();
    let offers = call_with_config(
        store,
        data_path,
        b"GET /api/championships/c1/offers HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    let v = body_json(&offers);
    assert_eq!(v["open"], false);
    assert_eq!(v["signed"]["team"], OPEN_SEAT);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_release_tears_up_an_unraced_contract() {
    let (root, config) = make_offer_fixture(true);
    let (resp, store) = sign_call(
        rated_champ("c1"),
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}"}}"#),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let (_, data_path) = make_test_store();
    let gone = call_with_config(
        store.clone(),
        data_path,
        b"DELETE /api/championships/c1/sign HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    assert!(status_line(&gone).contains("200"), "{gone}");

    let data = store.read().unwrap();
    assert!(data.contracts.is_empty(), "the deal is gone");
    assert!(
        data.championships[0].player_team.is_none(),
        "and so is the seat it came with"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_release_refused_once_the_season_has_started() {
    let (root, config) = make_offer_fixture(true);
    let (store, data_path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let champ = Championship {
            rounds: vec![Round {
                session_ids: vec!["s1".into()],
            }],
            player_team: Some(OPEN_SEAT.into()),
            planned_rounds: None,
            ..rated_champ("c1")
        };
        data.championships.push(champ);
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: OPEN_SEAT.into(),
            signed_at: 1,
            salary: 500_000,
            objective: None,
            bought_for: 0,
            settled: None,
        });
    }
    let resp = call_with_config(
        store.clone(),
        data_path,
        b"DELETE /api/championships/c1/sign HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert_eq!(store.read().unwrap().contracts.len(), 1, "the deal stands");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_release_without_a_contract_is_404() {
    let (root, config) = make_offer_fixture(true);
    let (resp, _) = sign_call(rated_champ("c1"), &config, "DELETE", "");
    assert!(status_line(&resp).contains("404"), "{resp}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_sign_then_finances_reports_the_season() {
    // End to end: signing writes the row the ledger reads, with the terms the grid gave.
    let (root, config) = make_offer_fixture(true);
    let (resp, store) = sign_call(
        rated_champ("c1"),
        &config,
        "POST",
        &format!(r#"{{"team":"{OPEN_SEAT}"}}"#),
    );
    let salary = body_json(&resp)["contract"]["salary"].as_i64().unwrap();

    let (_, data_path) = make_test_store();
    let fin = call_with_config(
        store,
        data_path,
        b"GET /api/career/finances HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    let v = body_json(&fin);
    assert_eq!(v["seasons"][0]["team"], OPEN_SEAT);
    // Unfinished, so the wage is contracted but not yet credited.
    assert_eq!(v["seasons"][0]["complete"], false);
    assert_eq!(v["seasons"][0]["salary"], 0);
    assert!(salary > 0, "the deal itself is worth something");
    std::fs::remove_dir_all(&root).ok();
}

// ── A save that cannot be read ────────────────────────────────────────────────

#[test]
fn test_route_activate_refuses_a_save_it_cannot_read() {
    // Switching would put an empty career in front of the user under that save's name. The file
    // itself is safe either way — persist guards it — but the switch is still the wrong answer.
    let (store, path) = make_saves_dir("activate_broken");
    let broken = path.parent().unwrap().join("corrupt.json");
    std::fs::write(&broken, "{ not json").unwrap();

    let resp = post(
        store.clone(),
        path.clone(),
        "/api/saves/activate",
        br#"{"name":"corrupt"}"#,
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("could not be read"), "{resp}");
    // The career in memory is the one that was already active, untouched.
    assert_eq!(store.read().unwrap().championships.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&broken).unwrap(),
        "{ not json",
        "the damaged file must not be rewritten"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_activate_accepts_a_save_with_a_byte_order_mark() {
    let (store, path) = make_saves_dir("activate_bom");
    let other = path.parent().unwrap().join("withbom.json");
    std::fs::write(
        &other,
        "\u{feff}{\"sessions\":[],\"championships\":[]}",
    )
    .unwrap();

    let resp = post(
        store.clone(),
        path.clone(),
        "/api/saves/activate",
        br#"{"name":"withbom"}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert!(store.read().unwrap().championships.is_empty());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_saves_list_reports_an_unreadable_file() {
    let (store, path) = make_saves_dir("list_broken");
    std::fs::write(path.parent().unwrap().join("corrupt.json"), "nope").unwrap();

    let v = body_json(&get(store, path.clone(), "/api/saves"));
    let bad = v["saves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "corrupt")
        .expect("the broken save is still listed");
    assert!(bad["error"].is_string(), "{bad}");
    // A healthy save carries no error key at all, so the UI can test for its presence.
    let good = v["saves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "ams2_career")
        .unwrap();
    assert!(good.get("error").is_none(), "{good}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_mutation_reports_when_the_save_cannot_be_written() {
    // The change reached memory but not the file. Answering 200 would tell the user their
    // championship was saved when it was not — the exact silent failure the guard exists for.
    let (store, path) = make_saves_dir("write_refused");
    std::fs::write(&path, "{ corrupted while the server was running").unwrap();

    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"New","points_system":[],"manufacturer_scoring":false}"#,
    );
    assert!(status_line(&resp).contains("500"), "{resp}");
    assert!(resp.contains("refusing to overwrite"), "{resp}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{ corrupted while the server was running",
        "the file must be byte-for-byte untouched"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_patch_config_stores_the_economy_it_will_actually_use() {
    // The bug this pins: per-field clamping cannot enforce a relationship *between* two fields,
    // so PATCH stored a floor above its top. The Config tab then showed one economy while the
    // grid ran on another.
    let (store, path) = make_saves_dir("cfg_economy");
    let body = format!(
        r#"{{{},"contract_top_salary":100000,"contract_floor_salary":900000,
             "champion_prize":500,"last_place_prize":9000}}"#,
        r#""port":8080,"host":"127.0.0.1","poll_ms":200,"record_practice":true,
           "record_qualify":true,"record_race":true,"show_track_map":true,
           "track_map_max_points":5000"#
    );
    let resp = patch(store, path.clone(), "/api/config", body.as_bytes());
    assert!(status_line(&resp).contains("200"), "{resp}");

    let v = body_json(&resp);
    assert_eq!(v["config"]["contract_top_salary"], 100_000);
    assert_eq!(
        v["config"]["contract_floor_salary"], 100_000,
        "a floor above its top must not be stored: {resp}"
    );
    assert_eq!(v["config"]["last_place_prize"], 500);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_get_config_does_not_rewrite_the_file() {
    // Reading the config used to rewrite it, from several call sites per request and more than
    // one thread. That truncate-then-write window is what produced
    // "could not parse config file (EOF while parsing a value at line 1 column 0)".
    let (store, path) = make_saves_dir("cfg_readonly");
    let config = path.parent().unwrap().join("config.json");
    std::fs::write(&config, r#"{"port":9123}"#).unwrap();

    for _ in 0..5 {
        let resp = call_with_config(
            store.clone(),
            path.clone(),
            b"GET /api/config HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
            Some(config.clone()),
        );
        assert!(status_line(&resp).contains("200"));
    }
    assert_eq!(
        std::fs::read_to_string(&config).unwrap(),
        r#"{"port":9123}"#,
        "a read must leave the config byte-for-byte alone"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_offers_does_not_renew_a_seat_held_in_another_series() {
    // End to end on the rule that stops a 1967 Ferrari drive opening a 1990 Ferrari. The signed
    // season here ran against a different Custom AI file, so its team is not held in this one.
    let (root, config) = make_offer_fixture(true);
    let (store, data_path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.championships.push(rated_champ("c1"));

        // A completed season, won under contract, in a different class entirely.
        let mut past = make_champ("past");
        past.custom_ai_file = Some("F-Vintage_Gen1.xml".into());
        past.status = ChampionshipStatus::Final;
        past.rounds = vec![Round {
            session_ids: vec!["s1".into()],
        }];
        data.championships.push(past);
        data.sessions.push(win_session("s1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "past".into(),
            team: "Williams".into(),
            signed_at: 1,
            salary: 500_000,
            objective: Some(5),
            bought_for: 0,
            settled: None,
        });
    }
    let resp = call_with_config(
        store,
        data_path,
        b"GET /api/championships/c1/offers HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    let v = body_json(&resp);

    assert_eq!(v["class"], "F-Classic_Gen1", "{resp}");
    assert_eq!(v["standing"]["incumbent"], "Williams");
    assert_eq!(
        v["standing"]["class"], "F-Vintage_Gen1",
        "the payload says which series the seat was held in"
    );
    // Williams is locked on merit here and is not at the back, so with no renewal to carry it
    // there is no offer from them at all.
    // A renewal is a flag on an ordinary paid offer, not a kind of its own.
    let renewals: Vec<&serde_json::Value> = v["offers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["renewal"] == true)
        .collect();
    assert!(renewals.is_empty(), "{resp}");
    std::fs::remove_dir_all(&root).ok();
}

// ── Career mode ───────────────────────────────────────────────────────────────

use ams2_championship::data_store::CareerMode;

fn mode_store(mode: CareerMode) -> (ams2_championship::data_store::SharedStore, PathBuf) {
    let (store, path) = make_saves_dir("mode");
    store.write().unwrap().mode = mode;
    (store, path)
}

#[test]
fn test_route_new_season_needs_a_roster_in_singleplayer() {
    // A singleplayer season is defined by the grid it is raced on, so the roster is chosen when
    // the season is created rather than patched in afterwards.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"1986","points_system":[],"manufacturer_scoring":false}"#,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(resp.contains("Custom AI Drivers file"), "{resp}");
    assert!(store.read().unwrap().championships.is_empty());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_new_season_takes_the_roster_at_creation_in_singleplayer() {
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"1986","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml","planned_rounds":16}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    let data = store.read().unwrap();
    assert_eq!(
        data.championships[0].custom_ai_file.as_deref(),
        Some("F-Classic_Gen1.xml")
    );
    // The seat is never set by creating — it comes from signing.
    assert!(data.championships[0].player_team.is_none());
    drop(data);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_new_season_refuses_a_roster_in_multiplayer() {
    let (store, path) = mode_store(CareerMode::Multiplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"Friday night","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml","planned_rounds":16}"#,
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("races people"), "{resp}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_singleplayer_allows_only_one_season_at_a_time() {
    // The next drive is offered on the strength of the last one, so the last one has to be over
    // before there is anything to offer against.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    let body =
        br#"{"name":"1987","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml","planned_rounds":16}"#;

    let resp = post(store.clone(), path.clone(), "/api/championships", body);
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("finish the current season first"), "{resp}");

    // Finish it, and the next one may be created.
    store.write().unwrap().championships[0].status = ChampionshipStatus::Final;
    let resp = post(store.clone(), path.clone(), "/api/championships", body);
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(store.read().unwrap().championships.len(), 2);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_multiplayer_runs_as_many_seasons_as_it_likes() {
    let (store, path) = mode_store(CareerMode::Multiplayer);
    let body = br#"{"name":"Another","points_system":[],"manufacturer_scoring":false}"#;
    for _ in 0..3 {
        let resp = post(store.clone(), path.clone(), "/api/championships", body);
        assert!(status_line(&resp).contains("200"), "{resp}");
    }
    assert_eq!(store.read().unwrap().championships.len(), 4);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_singleplayer_team_comes_from_a_contract_not_the_picker() {
    // The gating that was deliberately left open when contracts were added: with the career
    // kind deciding, setting a team directly is no longer a way around signing for it.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    // A team is only meaningful against a roster, so the season needs one for the change to be
    // a change at all — without it the handler nulls the team and nothing has happened.
    store.write().unwrap().championships[0].custom_ai_file = Some("F-Classic_Gen1.xml".into());

    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"player_team":"Osella"}"#,
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("sign for a team"), "{resp}");
    assert!(store.read().unwrap().championships[0].player_team.is_none());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_multiplayer_has_no_roster_and_no_team() {
    let (store, path) = mode_store(CareerMode::Multiplayer);
    // Hand-edited into a shape multiplayer would never produce, so both edits are real changes.
    {
        let mut data = store.write().unwrap();
        data.championships[0].custom_ai_file = Some("F-Classic_Gen1.xml".into());
    }
    for body in [
        &br#"{"player_team":"Osella"}"#[..],
        &br#"{"custom_ai_file":null}"#[..],
    ] {
        let resp = patch(store.clone(), path.clone(), "/api/championships/c1", body);
        assert!(status_line(&resp).contains("409"), "{resp}");
        assert!(resp.contains("no roster and no team"), "{resp}");
    }
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_a_finished_singleplayer_season_stays_finished() {
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships[0].status = ChampionshipStatus::Final;

    // `Active` is the only other state singleplayer has, so that is what reopening would mean.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"status":"Active"}"#,
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("stays finished"), "{resp}");
    assert_eq!(
        store.read().unwrap().championships[0].status,
        ChampionshipStatus::Final
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_an_unset_career_behaves_as_it_always_did() {
    // Every rule above is off, so a save written before career modes keeps working untouched.
    let (store, path) = mode_store(CareerMode::Unset);
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"status":"Final"}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"status":"Progress"}"#,
    );
    assert!(
        status_line(&resp).contains("200"),
        "reopening is fine: {resp}"
    );
    // And a second season may be created while the first runs.
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"Second","points_system":[],"manufacturer_scoring":false}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_career_mode_may_be_set_once_and_never_changed() {
    let (store, path) = mode_store(CareerMode::Unset);
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/career/mode",
        br#"{"mode":"multiplayer"}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(store.read().unwrap().mode, CareerMode::Multiplayer);

    // Set once. There is no second time, in either direction.
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/career/mode",
        br#"{"mode":"singleplayer"}"#,
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("cannot be changed"), "{resp}");
    assert_eq!(store.read().unwrap().mode, CareerMode::Multiplayer);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_career_mode_refuses_unset_as_an_answer() {
    let (store, path) = mode_store(CareerMode::Unset);
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/career/mode",
        br#"{"mode":"unset"}"#,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert_eq!(store.read().unwrap().mode, CareerMode::Unset);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_new_career_must_name_its_kind() {
    let (store, path) = make_saves_dir("mode_required");
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/saves",
        br#"{"name":"Nameless"}"#,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(resp.contains("singleplayer or multiplayer"), "{resp}");
    assert!(!path.parent().unwrap().join("Nameless.json").exists());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_new_career_is_founded_with_the_configured_balance() {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_start_balance_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config.json");
    std::fs::write(&config, r#"{"starting_balance":750000}"#).unwrap();

    let (store, path) = make_saves_dir("start_balance");
    let body = br#"{"name":"Rookie","mode":"singleplayer"}"#;
    let mut req = format!(
        "POST /api/saves HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    let resp = call_with_config(store.clone(), path.clone(), req, Some(config));
    assert!(status_line(&resp).contains("200"), "{resp}");

    // Recorded on the save, not left to be read from config later.
    assert_eq!(store.read().unwrap().starting_balance, 750_000);
    let written = ams2_championship::data_store::load_data(
        &ams2_championship::saves::save_path(path.parent().unwrap(), "Rookie"),
    );
    assert_eq!(written.starting_balance, 750_000);

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_finances_reports_what_the_career_started_with() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.mode = CareerMode::Singleplayer;
        data.starting_balance = 1_000_000;
    }
    let v = body_json(&get(store, path.clone(), "/api/career/finances"));
    assert_eq!(v["starting"], 1_000_000);
    assert_eq!(v["balance"], 1_000_000);
    assert_eq!(v["earned"], 0, "capital is not income");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_a_founding_balance_can_buy_a_seat_before_anything_is_earned() {
    // End to end on why this exists: a rookie with no results can still take the one seat that
    // does not need a rating, because the career was founded with enough to bring sponsorship.
    let (root, config) = pay_driver_fixture(r#","contract_buy_in_per_point":1000"#);
    let (store, data_path) = make_sp_store();
    {
        let mut data = store.write().unwrap();
        data.starting_balance = 1_000_000;
        data.championships.push(rated_champ("c1"));
    }
    let body = format!(r#"{{"team":"{PAY_SEAT}"}}"#);
    let mut req = format!(
        "POST /api/championships/c1/sign HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    let resp = call_with_config(store.clone(), data_path, req, Some(config));

    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    assert_eq!(v["contract"]["team"], PAY_SEAT);
    assert!(
        v["contract"]["bought_for"].as_i64().unwrap() > 0,
        "the seat was bought with the founding money: {resp}"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_changing_the_config_does_not_move_an_existing_careers_balance() {
    // The reason the figure lives on the save: a career that was founded with a million still
    // started with a million after someone edits the setting.
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.mode = CareerMode::Singleplayer;
        data.starting_balance = 1_000_000;
    }
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_start_balance_cfg_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config.json");
    std::fs::write(&config, r#"{"starting_balance":5}"#).unwrap();

    let resp = call_with_config(
        store,
        path.clone(),
        b"GET /api/career/finances HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    assert_eq!(body_json(&resp)["starting"], 1_000_000, "{resp}");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_a_singleplayer_season_is_the_current_one_from_the_moment_it_exists() {
    // It is the only unfinished season a career may have, so there is nothing for it to be
    // "in progress but not current" relative to. Creating it as Progress also left a fresh
    // career with nothing marked Active — which is what /api/live-teams looks for, so the live
    // grid showed no team names until the user found the dropdown.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"1986","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml","planned_rounds":16}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(
        store.read().unwrap().championships[0].status,
        ChampionshipStatus::Active
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_a_multiplayer_season_still_starts_in_progress() {
    // Several can run at once there, so "started, but not the one I am racing tonight" is a
    // state that means something.
    let (store, path) = mode_store(CareerMode::Multiplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"Thursday","points_system":[],"manufacturer_scoring":false}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(
        store.read().unwrap().championships[0].status,
        ChampionshipStatus::Progress
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_singleplayer_has_no_progress_state() {
    let (store, path) = mode_store(CareerMode::Singleplayer);
    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"status":"Progress"}"#,
    );
    assert!(status_line(&resp).contains("409"), "{resp}");
    assert!(resp.contains("being raced or finished"), "{resp}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_the_only_singleplayer_transition_is_finishing() {
    // Active → Final, and that is the whole state machine.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships[0].status = ChampionshipStatus::Active;

    let resp = patch(
        store.clone(),
        path.clone(),
        "/api/championships/c1",
        br#"{"status":"Final"}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(
        store.read().unwrap().championships[0].status,
        ChampionshipStatus::Final
    );
    // And there is no way back, by either route.
    for body in [
        &br#"{"status":"Active"}"#[..],
        &br#"{"status":"Progress"}"#[..],
    ] {
        let resp = patch(store.clone(), path.clone(), "/api/championships/c1", body);
        assert!(status_line(&resp).contains("409"), "{resp}");
    }
    assert_eq!(
        store.read().unwrap().championships[0].status,
        ChampionshipStatus::Final
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_a_fresh_singleplayer_career_shows_team_names_live() {
    // The bug this fixes, end to end: /api/live-teams reads the Active championship, and a
    // newly created season used to be Progress — so a fresh career had no Active one at all.
    let (root, config) = make_offer_fixture(true);
    let (store, data_path) = make_sp_store();
    let body = br#"{"name":"1986","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml","planned_rounds":16}"#;
    let mut req = format!(
        "POST /api/championships HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    let resp = call_with_config(store.clone(), data_path.clone(), req, Some(config.clone()));
    assert!(status_line(&resp).contains("200"), "{resp}");

    let resp = call_with_config(
        store,
        data_path,
        b"GET /api/live-teams HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    let v = body_json(&resp);
    assert!(
        v["teams"].as_object().is_some_and(|t| !t.is_empty()),
        "the roster's team names should be live immediately: {resp}"
    );
    std::fs::remove_dir_all(&root).ok();
}

// ── Finishing a season closes its books ──────────────────────────────────────

/// A request with a body, as raw bytes — `call_with_config` and `call_full` take the whole
/// request, and the `post`/`patch` helpers above build their own connection without a config.
fn req_bytes(method: &str, path: &str, body: &str) -> Vec<u8> {
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    req
}

/// An offer fixture whose prize money is named explicitly, so a test can move it and see.
fn sealing_fixture(champion: i64, floor: i64) -> (std::path::PathBuf, std::path::PathBuf) {
    offer_fixture(
        true,
        &format!(r#","champion_prize":{champion},"last_place_prize":{floor}"#),
    )
}

/// A championship with one race already assigned — a season there is something to finish.
fn raced_champ(id: &str, status: ChampionshipStatus) -> Championship {
    Championship {
        status,
        rounds: vec![Round {
            session_ids: vec!["s1".into()],
        }],
        ..rated_champ(id)
    }
}

fn finances_of(
    store: &ams2_championship::data_store::SharedStore,
    data_path: &std::path::Path,
    config: &std::path::Path,
) -> serde_json::Value {
    body_json(&call_with_config(
        store.clone(),
        data_path.to_path_buf(),
        b"GET /api/career/finances HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config.to_path_buf()),
    ))
}

#[test]
fn test_route_finishing_a_season_seals_its_payout_against_the_config_tab() {
    // The whole point of sealing: once a season is over, retuning the economy must not reach
    // back into it. Before this, `champion_prize` re-paid every finished season in the career.
    let (root, config) = sealing_fixture(1_000_000, 100_000);
    let (store, data_path) = make_sp_store();
    {
        let mut data = store.write().unwrap();
        data.championships
            .push(raced_champ("c1", ChampionshipStatus::Active));
        data.sessions.push(win_session("s1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: OPEN_SEAT.into(),
            signed_at: 1,
            salary: 500_000,
            objective: Some(3),
            bought_for: 0,
            settled: None,
        });
    }

    let resp = call_with_config(
        store.clone(),
        data_path.clone(),
        req_bytes("PATCH", "/api/championships/c1", r#"{"status":"Final"}"#),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let at_close = finances_of(&store, &data_path, &config);
    let paid = at_close["seasons"][0]["prize"].as_i64().unwrap();
    assert_eq!(paid, 1_000_000, "a win pays the champion rate");
    assert_eq!(at_close["seasons"][0]["complete"], true);
    assert!(
        store.read().unwrap().contracts[0].settled.is_some(),
        "finishing stamped the payout onto the contract"
    );

    // Now move the economy, exactly as the Config tab does.
    std::fs::write(
        &config,
        std::fs::read_to_string(&config)
            .unwrap()
            .replace("1000000", "9000000"),
    )
    .unwrap();
    let after = finances_of(&store, &data_path, &config);
    assert_eq!(
        after["seasons"][0]["prize"].as_i64().unwrap(),
        paid,
        "a finished season is closed: {after}"
    );
    assert_eq!(after["balance"], at_close["balance"]);

    let _ = std::fs::remove_file(&data_path);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_reopening_a_season_takes_its_settlement_back() {
    // Reopening has always taken the payout back. Sealing must not quietly make it permanent,
    // so the transition out of `Final` tears the stamp up again.
    let (root, config) = sealing_fixture(1_000_000, 100_000);
    // Not a singleplayer career: there, a finished season stays finished by design.
    let (store, data_path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        data.championships
            .push(raced_champ("c1", ChampionshipStatus::Active));
        data.sessions.push(win_session("s1"));
        data.contracts.push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: OPEN_SEAT.into(),
            signed_at: 1,
            salary: 500_000,
            objective: Some(3),
            bought_for: 0,
            settled: None,
        });
    }
    for body in [r#"{"status":"Final"}"#, r#"{"status":"Active"}"#] {
        let resp = call_with_config(
            store.clone(),
            data_path.clone(),
            req_bytes("PATCH", "/api/championships/c1", body),
            Some(config.clone()),
        );
        assert!(status_line(&resp).contains("200"), "{resp}");
    }
    assert!(
        store.read().unwrap().contracts[0].settled.is_none(),
        "an open season carries no settlement"
    );

    // Finishing it again takes a fresh stamp, at whatever the economy is now.
    std::fs::write(
        &config,
        std::fs::read_to_string(&config)
            .unwrap()
            .replace("1000000", "9000000"),
    )
    .unwrap();
    let resp = call_with_config(
        store.clone(),
        data_path.clone(),
        req_bytes("PATCH", "/api/championships/c1", r#"{"status":"Final"}"#),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = finances_of(&store, &data_path, &config);
    assert_eq!(v["seasons"][0]["prize"].as_i64().unwrap(), 9_000_000, "{v}");

    let _ = std::fs::remove_file(&data_path);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_a_season_finished_before_sealing_is_stamped_when_its_career_is_activated() {
    // The one-time upgrade. A career whose seasons were finished before settlements existed is
    // sealed as it is loaded, at the payout it was already showing — so the numbers do not move
    // on the upgrade, and stop moving afterwards.
    let (root, config) = sealing_fixture(1_000_000, 100_000);
    let saves_dir = root.join("championships");
    let legacy_dir = saves_dir.join("legacy");
    std::fs::create_dir_all(&legacy_dir).unwrap();

    let mut legacy = CareerData {
        mode: ams2_championship::data_store::CareerMode::Singleplayer,
        ..CareerData::default()
    };
    legacy
        .championships
        .push(raced_champ("c1", ChampionshipStatus::Final));
    legacy.sessions.push(win_session("s1"));
    legacy
        .contracts
        .push(ams2_championship::contracts::Contract {
            champ_id: "c1".into(),
            team: OPEN_SEAT.into(),
            signed_at: 1,
            salary: 500_000,
            objective: Some(3),
            bought_for: 0,
            settled: None,
        });
    let career = legacy_dir.join("career.json");
    std::fs::write(&career, serde_json::to_string(&legacy).unwrap()).unwrap();

    // Activating loads it, which is where the upgrade runs.
    let (store, data_path) = make_sp_store();
    let resp = call_full(
        store.clone(),
        data_path.clone(),
        saves_dir.clone(),
        req_bytes("POST", "/api/saves/activate", r#"{"name":"legacy"}"#),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let sealed = store.read().unwrap().contracts[0].settled.clone();
    let sealed = sealed.expect("activation seals a career finished before settlements existed");
    assert_eq!(
        sealed.prize, 1_000_000,
        "stamped at what it was already paying"
    );
    assert_eq!(sealed.at, 0, "no record of when it was actually finished");

    // Written through, not just held in memory.
    let on_disk: CareerData =
        serde_json::from_str(&std::fs::read_to_string(&career).unwrap()).unwrap();
    assert_eq!(on_disk.contracts[0].settled, Some(sealed));

    let _ = std::fs::remove_file(&data_path);
    std::fs::remove_dir_all(&root).ok();
}

// ── The career's own rating tuning ───────────────────────────────────────────

/// An offer fixture whose starting rating is named explicitly, so a test can move it and see.
fn rating_fixture(starting: i32) -> (std::path::PathBuf, std::path::PathBuf) {
    offer_fixture(true, &format!(r#","starting_rating":{starting}"#))
}

/// Rewrites a fixture's starting rating, standing in for an edit in the Config tab.
///
/// Goes through the JSON rather than a string replacement: creating a career rewrites config
/// via `config::save`, which pretty-prints, so the spelling of the field differs between the
/// file the fixture wrote and the one the server left behind.
fn retune(config: &std::path::Path, to: f64) {
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config).unwrap()).unwrap();
    v["starting_rating"] = serde_json::json!(to);
    std::fs::write(config, serde_json::to_string(&v).unwrap()).unwrap();
}

fn reputation_of(
    store: &ams2_championship::data_store::SharedStore,
    data_path: &std::path::Path,
    config: &std::path::Path,
) -> f64 {
    let resp = call_with_config(
        store.clone(),
        data_path.to_path_buf(),
        b"GET /api/championships/c1/team-eligibility HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config.to_path_buf()),
    );
    let v = body_json(&resp);
    assert_eq!(v["rated"], true, "{resp}");
    v["reputation"]["value"].as_f64().unwrap()
}

#[test]
fn test_route_a_new_career_records_the_rating_tuning_it_was_created_with() {
    let (root, config) = rating_fixture(20);
    let (store, path) = make_saves_dir("rating_stamp");
    let resp = call_with_config(
        store.clone(),
        path.clone(),
        req_bytes(
            "POST",
            "/api/saves",
            r#"{"name":"Stamped","mode":"singleplayer"}"#,
        ),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let stamped = store
        .read()
        .unwrap()
        .rating_params
        .expect("stamped at creation");
    assert_eq!(stamped.starting_rating, 20.0);

    // Written into the save, not just held in memory.
    let on_disk = ams2_championship::data_store::load_data(&ams2_championship::saves::save_path(
        path.parent().unwrap(),
        "Stamped",
    ));
    assert_eq!(on_disk.rating_params, Some(stamped));

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_retuning_the_rating_does_not_reach_into_an_existing_career() {
    // The point of the stamp: which seats a career was ever allowed to take must not be
    // rewritten backwards by an edit in Config.
    let (root, config) = rating_fixture(20);
    let (store, path) = make_saves_dir("rating_keep");
    let resp = call_with_config(
        store.clone(),
        path.clone(),
        req_bytes(
            "POST",
            "/api/saves",
            r#"{"name":"Kept","mode":"singleplayer"}"#,
        ),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    store.write().unwrap().championships.push(rated_champ("c1"));

    let before = reputation_of(&store, &path, &config);
    assert!(
        (before - 20.0).abs() < 0.001,
        "an unraced driver starts at the starting rating: {before}"
    );

    retune(&config, 80.0);
    let after = reputation_of(&store, &path, &config);
    assert!(
        (after - before).abs() < 0.001,
        "the career kept its own tuning: {after}"
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_adopting_moves_the_career_onto_the_current_settings() {
    // The deliberate exception. Retuning difficulty mid-career has to be possible; it just must
    // not happen as a side effect of editing a form.
    let (root, config) = rating_fixture(20);
    let (store, path) = make_saves_dir("rating_adopt");
    let resp = call_with_config(
        store.clone(),
        path.clone(),
        req_bytes(
            "POST",
            "/api/saves",
            r#"{"name":"Adopt","mode":"singleplayer"}"#,
        ),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    store.write().unwrap().championships.push(rated_champ("c1"));
    retune(&config, 80.0);

    let save = ams2_championship::saves::save_path(path.parent().unwrap(), "Adopt");
    let resp = call_with_config(
        store.clone(),
        save.clone(),
        req_bytes("POST", "/api/career/rating/adopt", ""),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(body_json(&resp)["starting_rating"], 80.0);

    let after = reputation_of(&store, &save, &config);
    assert!((after - 80.0).abs() < 0.001, "{after}");
    // Persisted, so the change survives a restart rather than lasting until the next load.
    assert_eq!(
        ams2_championship::data_store::load_data(&save)
            .rating_params
            .unwrap()
            .starting_rating,
        80.0
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_config_says_when_the_career_is_not_on_the_settings_shown() {
    // The Config tab shows config.json, but the career runs on its own copy. Without this the
    // tab would quietly imply the numbers on screen are the ones in force.
    let (root, config) = rating_fixture(20);
    let (store, path) = make_saves_dir("rating_diverge");
    let resp = call_with_config(
        store.clone(),
        path.clone(),
        req_bytes(
            "POST",
            "/api/saves",
            r#"{"name":"Diverge","mode":"singleplayer"}"#,
        ),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let matched = call_with_config(
        store.clone(),
        path.clone(),
        b"GET /api/config HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config.clone()),
    );
    assert_eq!(body_json(&matched)["career_rating_matches"], true);

    retune(&config, 80.0);
    let diverged = call_with_config(
        store.clone(),
        path.clone(),
        b"GET /api/config HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config.clone()),
    );
    let v = body_json(&diverged);
    assert_eq!(v["career_rating_matches"], false, "{diverged}");
    // The config fields still come through: the flag is added alongside them, not instead.
    assert_eq!(v["starting_rating"], 80.0);
    assert_eq!(v["career_rating"]["starting_rating"], 20.0);

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_a_career_created_before_stamping_adopts_its_tuning_when_activated() {
    // The one-time upgrade: a career that has been judged on config all along keeps being judged
    // on exactly those numbers, and stops moving afterwards.
    let (root, config) = rating_fixture(20);
    let saves_dir = root.join("championships");
    let legacy_dir = saves_dir.join("legacy");
    std::fs::create_dir_all(&legacy_dir).unwrap();

    let mut legacy = CareerData {
        mode: ams2_championship::data_store::CareerMode::Singleplayer,
        ..CareerData::default()
    };
    legacy.championships.push(rated_champ("c1"));
    assert!(
        legacy.rating_params.is_none(),
        "the state this upgrade is for"
    );
    let career = legacy_dir.join("career.json");
    std::fs::write(&career, serde_json::to_string(&legacy).unwrap()).unwrap();

    let (store, data_path) = make_sp_store();
    let resp = call_full(
        store.clone(),
        data_path.clone(),
        saves_dir.clone(),
        req_bytes("POST", "/api/saves/activate", r#"{"name":"legacy"}"#),
        Some(config.clone()),
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let stamped = store
        .read()
        .unwrap()
        .rating_params
        .expect("activation stamps it");
    assert_eq!(
        stamped.starting_rating, 20.0,
        "stamped at what it was already judged on"
    );
    assert_eq!(
        ams2_championship::data_store::load_data(&career).rating_params,
        Some(stamped),
        "written through, not just held in memory"
    );

    // And from here it is the career's own, not config's.
    retune(&config, 80.0);
    let after = reputation_of(&store, &career, &config);
    assert!((after - 20.0).abs() < 0.001, "{after}");

    let _ = std::fs::remove_file(&data_path);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_a_stamp_missing_a_field_falls_back_to_the_shipped_value_not_zero() {
    // `#[serde(default)]` on the container is what makes storing this safe: a hand-edited stamp
    // that drops `starting_rating` must not put every driver on the floor.
    let p: ams2_championship::driver_rating::RatingParams =
        serde_json::from_str(r#"{"strictness":5.0}"#).unwrap();
    assert_eq!(p.strictness, 5.0);
    assert_eq!(
        p.starting_rating,
        ams2_championship::driver_rating::RatingParams::default().starting_rating
    );
}

// ── Lap charts are fetched, not shipped ──────────────────────────────────────

/// A folder save with a store pointed at it — the only layout that keeps charts beside itself.
fn make_folder_store(
    tag: &str,
) -> (
    ams2_championship::data_store::SharedStore,
    std::path::PathBuf,
) {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_laps_route_{tag}_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    let career = ams2_championship::saves::save_path(&dir, "career");
    ams2_championship::saves::prepare_save_dir(&career).unwrap();
    let store = Arc::new(RwLock::new(CareerData::default()));
    ams2_championship::data_store::persist(&store, &career).unwrap();
    (store, career)
}

fn chart_entries() -> Vec<ams2_championship::data_store::LapChartEntry> {
    vec![
        ams2_championship::data_store::LapChartEntry {
            lap: 1,
            driver: "Nightrat".into(),
            position: 1,
        },
        ams2_championship::data_store::LapChartEntry {
            lap: 1,
            driver: "Berg".into(),
            position: 2,
        },
    ]
}

#[test]
fn test_route_career_no_longer_ships_every_lap_chart() {
    // The bug this fixes: the career payload carried a copy of every chart, so opening the tab
    // downloaded all of them to draw one. They are the bulk of a career.
    let (store, career) = make_folder_store("career_payload");
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds = vec![Round {
            session_ids: vec!["s1".into()],
        }];
        data.championships.push(champ);
        let mut s = win_session("s1");
        s.lap_chart = chart_entries();
        data.sessions.push(s);
    }
    let resp = get(store, career.clone(), "/api/career");
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert!(
        !body(&resp).contains("lap_chart"),
        "the career payload must not carry charts: {resp}"
    );

    let _ = std::fs::remove_dir_all(career.parent().unwrap().parent().unwrap());
}

#[test]
fn test_route_lap_chart_is_served_one_session_at_a_time() {
    let (store, career) = make_folder_store("fetch_one");
    ams2_championship::lap_charts::write(&career, "s1", &chart_entries()).unwrap();
    store.write().unwrap().sessions.push(win_session("s1"));

    let resp = get(store, career.clone(), "/api/sessions/s1/lap-chart");
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);
    assert_eq!(v.as_array().unwrap().len(), 2, "{resp}");
    assert_eq!(v[0]["driver"], "Nightrat");
    assert_eq!(v[1]["position"], 2);

    let _ = std::fs::remove_dir_all(career.parent().unwrap().parent().unwrap());
}

#[test]
fn test_route_a_session_with_no_chart_answers_with_an_empty_one() {
    // A practice session never has a chart. That is ordinary, not a failure.
    let (store, career) = make_folder_store("no_chart");
    store.write().unwrap().sessions.push(win_session("s1"));

    let resp = get(store, career.clone(), "/api/sessions/s1/lap-chart");
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(body_json(&resp).as_array().unwrap().len(), 0);

    let _ = std::fs::remove_dir_all(career.parent().unwrap().parent().unwrap());
}

#[test]
fn test_route_a_flat_saves_chart_still_comes_back_from_the_career() {
    // A legacy flat save has nowhere to keep a chart beside itself, so it keeps it inline and
    // the route has to fall back to it — those saves are never migrated.
    let (store, path) = make_test_store();
    {
        let mut s = win_session("s1");
        s.lap_chart = chart_entries();
        store.write().unwrap().sessions.push(s);
    }
    let resp = get(store, path.clone(), "/api/sessions/s1/lap-chart");
    assert_eq!(body_json(&resp).as_array().unwrap().len(), 2, "{resp}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_purging_unassigned_sessions_takes_their_charts_too() {
    // A chart nothing points at any more is a stray file, and charts are the big ones.
    let (store, career) = make_folder_store("purge");
    ams2_championship::lap_charts::write(&career, "keep", &chart_entries()).unwrap();
    ams2_championship::lap_charts::write(&career, "drop", &chart_entries()).unwrap();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds = vec![Round {
            session_ids: vec!["keep".into()],
        }];
        data.championships.push(champ);
        data.sessions.push(win_session("keep"));
        data.sessions.push(win_session("drop"));
    }

    let resp = delete(store, career.clone(), "/api/sessions/unassigned");
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(body_json(&resp)["removed"], 1);
    assert_eq!(
        ams2_championship::lap_charts::read(&career, "keep").len(),
        2
    );
    assert!(ams2_championship::lap_charts::read(&career, "drop").is_empty());

    let _ = std::fs::remove_dir_all(career.parent().unwrap().parent().unwrap());
}

// ── The calendar a salary is paid out across ─────────────────────────────────

#[test]
fn test_route_a_singleplayer_season_must_declare_its_calendar() {
    // Rounds appear as they are raced, so a season in progress cannot say how far through it
    // is. Declaring the calendar up front is what lets a wage be paid race by race instead of
    // in one lump at the end — so wherever there is a salary, it is required.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"1986","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml"}"#,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(resp.contains("how many races"), "{resp}");
    assert!(store.read().unwrap().championships.is_empty());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_a_zero_calendar_is_refused_like_a_missing_one() {
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"1986","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml","planned_rounds":0}"#,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(store.read().unwrap().championships.is_empty());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_the_calendar_is_recorded_on_the_season() {
    let (store, path) = mode_store(CareerMode::Singleplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"1986","points_system":[],"manufacturer_scoring":false,"custom_ai_file":"F-Classic_Gen1.xml","planned_rounds":16}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(store.read().unwrap().championships[0].planned_rounds, Some(16));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_a_multiplayer_season_needs_no_calendar() {
    // No contracts, so no wage to split across one. Asking for it would be a field that
    // answers nothing.
    let (store, path) = mode_store(CareerMode::Multiplayer);
    store.write().unwrap().championships.clear();
    let resp = post(
        store.clone(),
        path.clone(),
        "/api/championships",
        br#"{"name":"Friday night","points_system":[],"manufacturer_scoring":false}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(store.read().unwrap().championships[0].planned_rounds, None);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_patch_sets_a_calendar_on_a_season_that_had_none() {
    // The one way a season created before calendars existed starts paying per race. Not locked
    // by the first session the way the roster and the seat are: the wage is capped and never
    // topped up, so resizing only changes the instalments still to come.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    let id = store.read().unwrap().championships[0].id.clone();
    assert_eq!(store.read().unwrap().championships[0].planned_rounds, None);
    let resp = patch(
        store.clone(),
        path.clone(),
        &format!("/api/championships/{id}"),
        br#"{"planned_rounds":12}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(store.read().unwrap().championships[0].planned_rounds, Some(12));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_patch_floors_a_calendar_at_one_race() {
    // It is the denominator a wage is divided by, and config.json is not the only hand-edited
    // file here.
    let (store, path) = mode_store(CareerMode::Singleplayer);
    let id = store.read().unwrap().championships[0].id.clone();
    let resp = patch(
        store.clone(),
        path.clone(),
        &format!("/api/championships/{id}"),
        br#"{"planned_rounds":0}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(store.read().unwrap().championships[0].planned_rounds, Some(1));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_leaving_the_calendar_out_of_a_patch_keeps_it() {
    let (store, path) = mode_store(CareerMode::Singleplayer);
    let id = store.read().unwrap().championships[0].id.clone();
    store.write().unwrap().championships[0].planned_rounds = Some(16);
    let resp = patch(
        store.clone(),
        path.clone(),
        &format!("/api/championships/{id}"),
        br#"{"name":"1986 Season"}"#,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");
    assert_eq!(store.read().unwrap().championships[0].planned_rounds, Some(16));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

// ── GET /api/championships/:id/grid-check ────────────────────────────────────
//
// The Manage tab's half of the grid warning: which of a season's recorded sessions were
// actually raced on the roster the season is judged against. [`OFFER_ROSTER`] is a four-car
// grid — two Williams, two Osella — so a session with fewer cars is short, and one carrying
// names the roster does not know is padded with stock AI.

/// A recorded race with `names` as the AI and the player added on the end.
fn grid_session(id: &str, names: &[&str]) -> RecordedSession {
    let driver = |name: &str, pos: u32, is_player: bool| SessionResult {
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
    let mut results: Vec<SessionResult> = names
        .iter()
        .enumerate()
        .map(|(i, n)| driver(n, i as u32 + 1, false))
        .collect();
    results.push(driver("Nightrat", names.len() as u32 + 1, true));
    RecordedSession {
        id: id.into(),
        recorded_at: 1,
        track: "Monza".into(),
        track_variation: String::new(),
        car_name: String::new(),
        car_class: "F-Classic_Gen1".into(),
        session_type: 5,
        results,
        lap_chart: vec![],
    }
}

/// A season on [`OFFER_ROSTER`] holding `sessions`, each in its own round.
fn grid_check_resp(sessions: Vec<RecordedSession>, config: &std::path::Path) -> String {
    let (store, data_path) = make_sp_store();
    let mut champ = rated_champ("c1");
    champ.rounds = sessions
        .iter()
        .map(|s| ams2_championship::data_store::Round {
            session_ids: vec![s.id.clone()],
        })
        .collect();
    {
        let mut data = store.write().unwrap();
        data.championships.push(champ);
        data.sessions = sessions;
    }
    call_with_config(
        store,
        data_path,
        "GET /api/championships/c1/grid-check HTTP/1.1\r\nHost: localhost\r\n\r\n"
            .to_string()
            .into_bytes(),
        Some(config.to_path_buf()),
    )
}

#[test]
fn test_route_grid_check_passes_a_session_that_raced_the_whole_roster() {
    let (root, config) = make_offer_fixture(true);
    let full = grid_session(
        "s1",
        &["Nigel Mansell", "Nelson Piquet", "Piercarlo Ghinzani"],
    );
    let resp = grid_check_resp(vec![full], &config);
    assert!(status_line(&resp).contains("200"), "{resp}");
    let v = body_json(&resp);

    assert_eq!(v["checked"], true);
    assert_eq!(v["seats"], 4);
    // A clean season says nothing rather than reassuring at length.
    assert!(v["summary"].is_null(), "{v}");
    assert!(v["sessions"][0]["note"].is_null(), "{v}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_route_grid_check_flags_a_short_grid_and_counts_it_in_the_summary() {
    let (root, config) = make_offer_fixture(true);
    let short = grid_session("s1", &["Nigel Mansell"]);
    let resp = grid_check_resp(vec![short], &config);
    let v = body_json(&resp);

    assert_eq!(v["checked"], true);
    let note = v["sessions"][0]["note"].as_str().unwrap_or_default();
    assert!(note.contains("Short grid"), "{v}");
    let summary = v["summary"].as_str().unwrap_or_default();
    assert!(summary.starts_with("1 of 1"), "{summary}");
    assert!(summary.contains("4-car"), "{summary}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_route_grid_check_flags_a_session_raced_on_another_roster() {
    let (root, config) = make_offer_fixture(true);
    let foreign = grid_session("s1", &["Someone Else", "Another One", "A Third"]);
    let resp = grid_check_resp(vec![foreign], &config);
    let v = body_json(&resp);

    let note = v["sessions"][0]["note"].as_str().unwrap_or_default();
    assert!(note.contains("Not raced on this roster"), "{v}");
    let _ = std::fs::remove_dir_all(&root);
}

/// Practice is not rated, so a short practice grid is a choice rather than a mistake. Leaving
/// it in would warn about the one session type the warning cannot apply to.
#[test]
fn test_route_grid_check_ignores_practice() {
    let (root, config) = make_offer_fixture(true);
    let mut practice = grid_session("s1", &["Nigel Mansell"]);
    practice.session_type = ams2_championship::data_store::SESSION_PRACTICE;
    let resp = grid_check_resp(vec![practice], &config);
    let v = body_json(&resp);

    assert_eq!(v["checked"], true);
    assert_eq!(v["sessions"].as_array().unwrap().len(), 0, "{v}");
    assert!(v["summary"].is_null(), "{v}");
    let _ = std::fs::remove_dir_all(&root);
}

/// Nothing to check against must not read as a clean bill of health.
#[test]
fn test_route_grid_check_without_a_roster_says_why() {
    let (store, path) = make_test_store();
    store.write().unwrap().championships.push(make_champ("c1"));
    let resp = get(store, path.clone(), "/api/championships/c1/grid-check");
    let v = body_json(&resp);

    assert_eq!(v["checked"], false);
    assert!(!v["reason"].as_str().unwrap_or_default().is_empty(), "{v}");
    assert!(v["summary"].is_null(), "{v}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_grid_check_unknown_championship_is_404() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/api/championships/nope/grid-check");
    assert!(status_line(&resp).contains("404"));
    let _ = std::fs::remove_file(&path);
}

// ── The live banner ──────────────────────────────────────────────────────────
//
// The live tab is the only place a grid problem can still be fixed, so this is the one warning
// that interrupts.

fn live_grid<'a>(names: &'a [(&'a str, bool)]) -> Vec<ams2_championship::custom_ai::GridEntry<'a>> {
    names
        .iter()
        .map(|(name, is_player)| ams2_championship::custom_ai::GridEntry {
            name,
            car_name: "",
            is_player: *is_player,
        })
        .collect()
}

#[test]
fn test_live_warning_names_a_short_grid_while_there_is_still_time_to_fix_it() {
    let dir = make_live_teams_dir();
    let grid = live_grid(&[("Carl Charlie", false), ("Me", true)]);
    let out = resolve_live_teams(&dir, &two_seasons(), &grid, true);

    let grid = out.grid.expect("a short grid is worth interrupting for");
    assert!(!grid.ok, "{grid:?}");
    assert!(grid.text.contains("Short grid"), "{grid:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Only a career judged on a roster is nagged about one: multiplayer races people.
#[test]
fn test_live_warning_is_silent_for_a_career_with_no_roster() {
    let dir = make_live_teams_dir();
    let grid = live_grid(&[("Carl Charlie", false), ("Me", true)]);
    let out = resolve_live_teams(&dir, &two_seasons(), &grid, false);

    assert!(out.grid.is_none());
    // The team names still resolve — nothing about the warning switches those off.
    assert!(!out.teams.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_live_warning_when_no_championship_is_active() {
    let dir = make_live_teams_dir();
    let mut champs = two_seasons();
    for c in &mut champs {
        c.status = ChampionshipStatus::Progress;
    }
    let grid = live_grid(&[("Carl Charlie", false), ("Me", true)]);
    let out = resolve_live_teams(&dir, &champs, &grid, true);

    let grid = out.grid.expect("no Active season is worth saying");
    assert!(!grid.ok, "{grid:?}");
    assert!(grid.text.contains("Active"), "{grid:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An empty grid is the menu, not a bad grid — warning there would leave the banner on screen
/// whenever the app is open.
#[test]
fn test_live_warning_says_nothing_before_a_session_loads() {
    let dir = make_live_teams_dir();
    let out = resolve_live_teams(&dir, &two_seasons(), &[], true);

    // Not `ok: false` and not `ok: true` — there is nothing to report on yet, and saying
    // either would be a claim about a grid that has not loaded.
    assert!(out.grid.is_none());
    assert!(out.fit.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The served page must actually carry the elements the scripts fill in. They are wired by id
/// across three files — markup in `championship_html.rs`, the fetch in one asset, the render in
/// another — so a rename that misses one would leave a warning that is computed and never seen.
#[test]
fn test_the_page_carries_the_hooks_the_grid_warning_needs() {
    let html = ams2_championship::championship_html::build_base_html();
    for id in [
        "live-grid-warning",   // live banner, filled by live.js
        "champ-grid-panel",    // Manage tab season panel, filled by manage.js
    ] {
        assert!(html.contains(id), "the page is missing #{id}");
    }
}

/// The other half of the banner, and the reason it is one field: a grid that is right says so.
/// Silence cannot distinguish "checked, all good" from "never checked", and the live tab is
/// the last place the driver can act on the difference.
#[test]
fn test_live_says_so_when_the_grid_is_the_roster() {
    let dir = make_live_teams_dir();
    // CORE_XML fields three cars; the player takes one of them.
    let grid = live_grid(&[("Alan Alpha", false), ("Ben Bravo", false), ("Me", true)]);
    let out = resolve_live_teams(&dir, &two_seasons(), &grid, true);

    let status = out.grid.expect("a full grid is worth confirming");
    assert!(status.ok, "{status:?}");
    assert!(status.text.contains("Full grid"), "{status:?}");
    assert!(status.text.contains('3'), "it names the size: {status:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Removing per-track entries through the tab ───────────────────────────────

/// Two cars, each with a per-track entry: one that retunes its own driver and one that fields
/// a stand-in, which are the two kinds a roster carries.
const TRACK_ENTRY_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<custom_ai_drivers>
    <driver livery_name="Williams #5 N. Mansell">
        <name>Nigel Mansell</name>
        <race_skill>0.95</race_skill>
    </driver>
    <driver livery_name="Williams #5 N. Mansell" tracks="Monza_1991">
        <race_skill>0.98</race_skill>
    </driver>
    <driver livery_name="AGS #31 I. Capelli">
        <name>Ivan Capelli</name>
        <race_skill>0.70</race_skill>
    </driver>
    <driver livery_name="AGS #31 I. Capelli" tracks="Interlagos_Historic">
        <name>Stand In</name>
        <race_skill>0.60</race_skill>
    </driver>
</custom_ai_drivers>
"#;

fn make_track_entry_fixture() -> (std::path::PathBuf, std::path::PathBuf) {
    let (dir, config) = make_perf_fixture();
    std::fs::write(dir.join("F-Test.xml"), TRACK_ENTRY_XML).unwrap();
    (dir, config)
}

fn delete_driver_perf(path: &str, body: &str, config: &std::path::Path) -> String {
    let (store, data_path) = make_test_store();
    let mut req = format!(
        "DELETE {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body.as_bytes());
    call_with_config(store, data_path, req, Some(config.to_path_buf()))
}

#[test]
fn test_route_removes_one_per_track_entry_and_returns_the_whole_class() {
    let (dir, config) = make_track_entry_fixture();
    let resp = delete_driver_perf(
        "/api/driver-performance",
        r#"{"class":"F-Test","index":1,"driver":"Nigel Mansell"}"#,
        &config,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    // The whole class, because every index below the removed row has just shifted.
    let drivers = body_json(&resp)["classes"][0]["drivers"].clone();
    let rows = drivers.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter().all(|r| r["tracks"] != "Monza_1991"),
        "{drivers}"
    );
    // Mansell keeps his own value and his car.
    assert_eq!(rows[0]["driver"], "Nigel Mansell");
    assert_eq!(body_json(&resp)["classes"][0]["cars"], 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The guard that makes the button safe to put next to a driver: it cannot delete the driver.
#[test]
fn test_route_refuses_to_remove_a_regular_entry() {
    let (dir, config) = make_track_entry_fixture();
    let resp = delete_driver_perf(
        "/api/driver-performance",
        r#"{"class":"F-Test","index":0,"driver":"Nigel Mansell"}"#,
        &config,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    assert!(
        body_json(&resp)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("per-track"),
        "{resp}"
    );
    // Nothing was written.
    let xml = std::fs::read_to_string(dir.join("F-Test.xml")).unwrap();
    assert!(xml.contains("Nigel Mansell"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_refuses_a_stale_index() {
    let (dir, config) = make_track_entry_fixture();
    let resp = delete_driver_perf(
        "/api/driver-performance",
        r#"{"class":"F-Test","index":1,"driver":"Somebody Else"}"#,
        &config,
    );
    assert!(status_line(&resp).contains("400"), "{resp}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_clears_every_per_track_entry_in_a_class() {
    let (dir, config) = make_track_entry_fixture();
    let resp = delete_driver_perf(
        "/api/driver-performance/track-entries",
        r#"{"class":"F-Test"}"#,
        &config,
    );
    assert!(status_line(&resp).contains("200"), "{resp}");

    let cls = body_json(&resp)["classes"][0].clone();
    let rows = cls["drivers"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 2, "{cls}");
    assert!(rows.iter().all(|r| r["tracks"].is_null()), "{cls}");
    // The grid is the grid it was: both cars, both regular drivers.
    assert_eq!(cls["cars"], 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The roster is backed up before the first write, so the Car Performance tab's reset undoes
/// this like any other edit.
#[test]
fn test_removing_takes_the_same_one_time_backup_every_writer_does() {
    let (dir, config) = make_track_entry_fixture();
    let _ = delete_driver_perf(
        "/api/driver-performance/track-entries",
        r#"{"class":"F-Test"}"#,
        &config,
    );
    let backup = std::fs::read_to_string(dir.join("F-Test.xml.bak")).unwrap();
    assert!(backup.contains("Monza_1991"), "the baseline still has them");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_route_patch_config_round_trips_a_class_season_year() {
    // The whole point of the setting: a class the shipped table cannot answer for gets an
    // answer, and GET lists it back as an override rather than as the table's own.
    let (store, path) = make_saves_dir("cfg_class_years");
    let config = path.parent().unwrap().join("config.json");
    let body = br#"{"port":8080,"host":"127.0.0.1","poll_ms":200,"record_practice":true,
        "record_qualify":true,"record_race":true,"show_track_map":true,
        "track_map_max_points":5000,"saves_dir":null,
        "class_years":{"FE-G1":1995,"F-Retro_Gen1":1974}}"#;
    let resp = patch_config_with(store.clone(), path.clone(), body, config.clone());
    assert!(status_line(&resp).contains("200"), "got {resp}");
    let v = body_json(&resp);
    assert_eq!(v["config"]["class_years"]["FE-G1"], 1995);
    assert!(
        v["config"]["class_years"].get("F-Retro_Gen1").is_none(),
        "a year that only repeats the built-in one is not an override: {resp}"
    );

    let resp = call_with_config(
        store,
        path.clone(),
        b"GET /api/config HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
        Some(config),
    );
    let v = body_json(&resp);
    let row = v["classes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["class"] == "FE-G1")
        .cloned()
        .unwrap_or_else(|| panic!("FE-G1 not listed: {resp}"));
    assert_eq!(row["year"], 1995);
    assert_eq!(row["overridden"], true);
    assert!(
        row["builtin"].is_null(),
        "the table has no year of its own for a fictional car: {resp}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_patch_config_omitting_class_years_leaves_them_alone() {
    // `config_body` sends none at all, exactly like a form rendered before the field existed.
    // An omitted map carries the stored overrides through; only an explicit empty one clears.
    let (store, path) = make_saves_dir("cfg_class_years_absent");
    let config = path.parent().unwrap().join("config.json");
    std::fs::write(&config, r#"{"class_years":{"FE-G1":1995}}"#).unwrap();

    let resp = patch_config_with(
        store.clone(),
        path.clone(),
        &config_body("null"),
        config.clone(),
    );
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert_eq!(body_json(&resp)["config"]["class_years"]["FE-G1"], 1995);

    let cleared = br#"{"port":8080,"host":"127.0.0.1","poll_ms":200,"record_practice":true,
        "record_qualify":true,"record_race":true,"show_track_map":true,
        "track_map_max_points":5000,"saves_dir":null,"class_years":{}}"#;
    let resp = patch_config_with(store, path.clone(), cleared, config);
    assert!(status_line(&resp).contains("200"), "got {resp}");
    assert!(
        body_json(&resp)["config"]["class_years"]
            .as_object()
            .unwrap()
            .is_empty(),
        "an empty map is a deliberate clear, not a stale form: {resp}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

// ── livery previews ───────────────────────────────────────────────────────────

/// The smallest real DDS there is: one 4×4 DXT1 block of flat red.
fn tiny_dds() -> Vec<u8> {
    let mut dds = vec![0u8; 128];
    dds[0..4].copy_from_slice(b"DDS ");
    dds[4..8].copy_from_slice(&124u32.to_le_bytes());
    dds[12..16].copy_from_slice(&4u32.to_le_bytes()); // height
    dds[16..20].copy_from_slice(&4u32.to_le_bytes()); // width
    dds[80..84].copy_from_slice(&0x4u32.to_le_bytes()); // DDPF_FOURCC
    dds[84..88].copy_from_slice(b"DXT1");
    let red = 0xF800u16; // 5:6:5 red, and the larger endpoint, so no punch-through
    dds.extend_from_slice(&red.to_le_bytes());
    dds.extend_from_slice(&0u16.to_le_bytes());
    dds.extend_from_slice(&0u32.to_le_bytes()); // every pixel on endpoint 0
    dds
}

/// [`make_perf_fixture`], plus a livery mod that declares a preview picture for the Williams and
/// nothing for the AGS — and the Custom AI folder nested where the install root can be derived
/// from it.
fn make_preview_fixture() -> (std::path::PathBuf, std::path::PathBuf) {
    let (dir, config) = make_perf_fixture();
    let model = dir
        .join("Vehicles")
        .join("Textures")
        .join("CustomLiveries")
        .join("Overrides")
        .join("williams_fw14");
    std::fs::create_dir_all(model.join("Previews")).unwrap();
    std::fs::write(
        model.join("williams_fw14.xml"),
        r#"<USER_OVERRIDES>
        <LIVERY_OVERRIDE LIVERY="1" NAME="Williams #5 N. Mansell" BASELIVERY="Default">
            <PREVIEWIMAGE PATH="Previews\five.dds" />
            <TEXTURE NAME="BODY" PATH="Previews\body.dds" />
        </LIVERY_OVERRIDE>
        <LIVERY_OVERRIDE LIVERY="2" NAME="AGS #31 I. Capelli" BASELIVERY="Default" />
        </USER_OVERRIDES>"#,
    )
    .unwrap();
    std::fs::write(model.join("Previews").join("five.dds"), tiny_dds()).unwrap();
    let ai = dir.join("UserData").join("CustomAIDrivers");
    std::fs::create_dir_all(&ai).unwrap();
    std::fs::copy(dir.join("F-Test.xml"), ai.join("F-Test.xml")).unwrap();
    std::fs::write(
        &config,
        format!(
            "{{\"custom_ai_dir\":{}}}",
            serde_json::to_string(&ai.display().to_string()).unwrap()
        ),
    )
    .unwrap();
    (dir, config)
}

fn get_bytes_with_config(path: &str, config: &std::path::Path) -> Vec<u8> {
    let (store, data_path) = make_test_store();
    let saves_dir = data_path.parent().unwrap().to_path_buf();
    call_full_raw(
        store,
        data_path,
        saves_dir,
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes(),
        Some(config.to_path_buf()),
    )
}

#[test]
fn test_route_car_performance_carries_the_preview_each_car_has_one_for() {
    let (dir, config) = make_preview_fixture();
    let cars = body_json(&get_with_config("/api/car-performance", &config))["classes"][0]["cars"]
        .clone();
    let by_team: std::collections::HashMap<String, serde_json::Value> = cars
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["team"].as_str().unwrap().to_string(), c.clone()))
        .collect();
    assert_eq!(
        by_team["Williams"]["preview"],
        "williams_fw14/Previews/five.dds"
    );
    // The AGS entry declares no PREVIEWIMAGE, which is not an error — it simply has no picture.
    assert!(by_team["AGS"]["preview"].is_null(), "{cars}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_livery_preview_answers_with_a_png() {
    let (dir, config) = make_preview_fixture();
    let resp = get_bytes_with_config(
        "/api/livery-preview/williams_fw14%2FPreviews%2Ffive.dds",
        &config,
    );
    let head = String::from_utf8_lossy(&resp[..resp.len().min(200)]).into_owned();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("Content-Type: image/png"), "{head}");
    // The one route that may be cached: the file it reads is part of the game's install.
    assert!(head.contains("Cache-Control: public"), "{head}");
    let body_at = resp.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    assert_eq!(
        &resp[body_at..body_at + 8],
        &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A],
        "the body is a PNG"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_livery_preview_refuses_to_read_outside_the_overrides_folder() {
    let (dir, config) = make_preview_fixture();
    // The config.json two levels up is a real file, and readable — the path check is the only
    // thing standing between a typed URL and it.
    let escape = "/api/livery-preview/..%2F..%2F..%2F..%2Fconfig.dds";
    let resp = get_with_config(escape, &config);
    assert!(status_line(&resp).contains("404"), "{resp}");

    // A path inside the folder that is not a texture is refused on the same grounds.
    let resp = get_with_config("/api/livery-preview/williams_fw14%2Fwilliams_fw14.xml", &config);
    assert!(status_line(&resp).contains("404"), "{resp}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_route_livery_preview_is_a_plain_404_without_a_custom_ai_folder() {
    let (store, data_path) = make_test_store();
    let resp = get(store, data_path, "/api/livery-preview/anything%2Fat-all.dds");
    assert!(status_line(&resp).contains("404"), "{resp}");
}

#[test]
fn test_route_offers_carry_a_picture_of_each_car_on_offer() {
    // Nested the way a real install is, so the Overrides folder can be derived by climbing two
    // levels out of the Custom AI folder.
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ams2_offer_prev_{ns}"));
    let ai_dir = root.join("UserData").join("CustomAIDrivers");
    std::fs::create_dir_all(&ai_dir).unwrap();
    std::fs::write(ai_dir.join("F-Classic_Gen1.xml"), OFFER_ROSTER).unwrap();
    let model = root
        .join("Vehicles")
        .join("Textures")
        .join("CustomLiveries")
        .join("Overrides")
        .join("williams_fw11");
    std::fs::create_dir_all(model.join("Previews")).unwrap();
    // Only the Williams has a picture; the Osella declares none, which is not an error.
    std::fs::write(
        model.join("williams_fw11.xml"),
        r#"<USER_OVERRIDES>
        <LIVERY_OVERRIDE LIVERY="1" NAME="1986 Williams #5 - N. Mansell" BASELIVERY="Default">
            <PREVIEWIMAGE PATH="Previews\w5.dds" />
        </LIVERY_OVERRIDE>
        <LIVERY_OVERRIDE LIVERY="2" NAME="1986 Osella #21 - P. Ghinzani" BASELIVERY="Default" />
        </USER_OVERRIDES>"#,
    )
    .unwrap();
    let config = root.join("config.json");
    std::fs::write(
        &config,
        format!(
            "{{\"custom_ai_dir\":{}}}",
            serde_json::to_string(&ai_dir.display().to_string()).unwrap()
        ),
    )
    .unwrap();

    let v = body_json(&offers_resp(rated_champ("c1"), &config));
    assert_eq!(v["rated"], true, "{v}");
    assert_eq!(v["previews"]["Williams"], "williams_fw11/Previews/w5.dds");
    assert!(v["previews"]["Osella"].is_null(), "{}", v["previews"]);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_route_offers_carry_an_empty_picture_map_without_a_livery_mod() {
    let (root, config) = make_offer_fixture(true);
    let v = body_json(&offers_resp(rated_champ("c1"), &config));
    // Nothing installed to look in: a map with no entries, not a missing field the client would
    // have to guard against.
    assert!(v["previews"].is_object(), "{v}");
    assert_eq!(v["previews"].as_object().unwrap().len(), 0, "{v}");
    std::fs::remove_dir_all(&root).ok();
}
