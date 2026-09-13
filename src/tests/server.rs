use super::*;
use ams2_championship::data_store::{CareerData, Championship, ChampionshipStatus, Round};
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let html = Arc::new(b"<html/>".to_vec());
    // Saves live alongside the career file, as they do under championships/ in production.
    let saves_dir = Arc::new(
        data_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(std::env::temp_dir),
    );
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
    String::from_utf8_lossy(&resp).into_owned()
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
    ams2_championship::data_store::persist(&store, &path);
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
        br#"{"name":"GT3 Career"}"#,
    );
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert_eq!(v["active"], "GT3 Career");
    assert_eq!(v["saves"].as_array().unwrap().len(), 2);
    // The in-memory store was swapped to the new, empty career.
    assert!(store.read().unwrap().championships.is_empty());
    assert!(path.parent().unwrap().join("GT3 Career.json").exists());
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
    ams2_championship::data_store::persist(&other_store, &other);

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
    let copy = path.parent().unwrap().join("backup.json");
    assert!(copy.exists());
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

// ── PATCH /api/config — saves folder ──────────────────────────────────────────

fn config_body(saves_dir: &str) -> Vec<u8> {
    format!(
        r#"{{"port":8080,"host":"127.0.0.1","poll_ms":200,"record_practice":true,
             "record_qualify":true,"record_race":true,"show_track_map":true,
             "track_map_max_points":5000,"saves_dir":{saves_dir}}}"#
    )
    .into_bytes()
}

#[test]
fn test_route_patch_config_saves_dir_requires_restart_and_clears_data_file() {
    let (store, path) = make_saves_dir("cfg_saves_dir");
    let new_dir = path.parent().unwrap().join("elsewhere");

    let body = config_body(&serde_json::Value::String(new_dir.display().to_string()).to_string());
    let resp = patch(store, path.clone(), "/api/config", &body);
    assert!(status_line(&resp).contains("200"));
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
        v["config"]["data_file"].is_null(),
        "the remembered save lived in the old folder, so it is dropped"
    );
    assert!(new_dir.is_dir(), "the folder is created eagerly");

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn test_route_patch_config_unchanged_saves_dir_keeps_data_file() {
    let (store, path) = make_saves_dir("cfg_saves_same");
    let resp = patch(store, path.clone(), "/api/config", &config_body("null"));
    assert!(status_line(&resp).contains("200"));
    let v = body_json(&resp);
    assert!(
        !v["restart_required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("saves_dir")),
        "null == unset, so nothing changed"
    );

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
    let out = resolve_live_teams(&dir, &two_seasons());

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

    let out = resolve_live_teams(&dir, &champs);
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

    let out = resolve_live_teams(&dir, &champs);
    assert!(out.teams.is_empty());
    assert_eq!(out.player_team, None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_live_teams_empty_when_the_active_championship_has_no_roster() {
    let dir = make_live_teams_dir();
    let mut champs = two_seasons();
    champs[0].custom_ai_file = None;

    let out = resolve_live_teams(&dir, &champs);
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

    let out = resolve_live_teams(&dir, &champs);
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
