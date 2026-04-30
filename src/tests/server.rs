use super::*;
use ams2_championship::data_store::{CareerData, Championship, ChampionshipStatus, Round};
use ams2_championship::http::{Request, parse_request, track_slug};
use ams2_championship::websocket::{sha1, base64_encode, ws_accept_key, ws_send_text};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, RwLock};

fn req(raw: &[u8]) -> Request {
    parse_request(raw)
}

// ── HTTP route integration helpers ────────────────────────────────────────────

fn make_test_store() -> (ams2_championship::data_store::SharedStore, std::path::PathBuf) {
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let html = Arc::new(b"<html/>".to_vec());
    let dp = Arc::new(data_path);
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let layouts_dir = Arc::new(std::env::temp_dir().join(format!("ams2_layouts_{ns}")));
    std::fs::create_dir_all(layouts_dir.as_ref()).unwrap();
    let config_path = Arc::new(std::env::temp_dir().join(format!("ams2_cfg_{ns}.json")));
    let s = store;
    std::thread::spawn(move || {
        let (conn, _) = listener.accept().unwrap();
        handle(conn, html, s, dp, layouts_dir, config_path, 200);
    });
    let mut client = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    client.write_all(&request_bytes).unwrap();
    let mut resp = Vec::new();
    client.read_to_end(&mut resp).unwrap();
    String::from_utf8_lossy(&resp).into_owned()
}

fn get(store: ams2_championship::data_store::SharedStore, data_path: std::path::PathBuf, path: &str) -> String {
    call(store, data_path, format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes())
}

fn post(store: ams2_championship::data_store::SharedStore, data_path: std::path::PathBuf, path: &str, body: &[u8]) -> String {
    let mut req = format!("POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    req.extend_from_slice(body);
    call(store, data_path, req)
}

fn patch(store: ams2_championship::data_store::SharedStore, data_path: std::path::PathBuf, path: &str, body: &[u8]) -> String {
    let mut req = format!("PATCH {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    req.extend_from_slice(body);
    call(store, data_path, req)
}

fn delete(store: ams2_championship::data_store::SharedStore, data_path: std::path::PathBuf, path: &str) -> String {
    call(store, data_path, format!("DELETE {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes())
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
        id: id.into(), name: "Test Champ".into(),
        status: ChampionshipStatus::Active,
        points_system: vec![25, 18, 15],
        manufacturer_scoring: false,
        rounds: vec![],
        session_ids: vec![],
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
fn test_route_default_returns_html() {
    let (store, path) = make_test_store();
    let resp = get(store, path.clone(), "/");
    assert!(status_line(&resp).contains("200"));
    assert!(body(&resp).contains("<html"), "default route should return HTML");
    let _ = std::fs::remove_file(&path);
}

// ── POST /api/championships ────────────────────────────────────────────────────

#[test]
fn test_route_post_championships_creates_championship() {
    let (store, path) = make_test_store();
    let resp = post(store.clone(), path.clone(), "/api/championships",
        b"{\"name\":\"My Champ\"}");
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
    let resp = post(store.clone(), path.clone(), "/api/championships", b"{\"name\":\"X\"}");
    let v = body_json(&resp);
    let pts: Vec<i64> = v["points_system"].as_array().unwrap()
        .iter().map(|x| x.as_i64().unwrap()).collect();
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
    let resp = patch(store.clone(), path.clone(),
        "/api/championships/42", b"{\"name\":\"Renamed\"}");
    assert!(status_line(&resp).contains("200"));
    assert_eq!(store.read().unwrap().championships[0].name, "Renamed");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_patch_championship_not_found_returns_404() {
    let (store, path) = make_test_store();
    let resp = patch(store, path.clone(), "/api/championships/999", b"{\"name\":\"X\"}");
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
    let resp = patch(store.clone(), path.clone(),
        "/api/championships/2", b"{\"status\":\"Active\"}");
    assert!(status_line(&resp).contains("200"));
    let data = store.read().unwrap();
    assert_eq!(data.championships.iter().find(|c| c.id == "1").unwrap().status, ChampionshipStatus::Progress);
    assert_eq!(data.championships.iter().find(|c| c.id == "2").unwrap().status, ChampionshipStatus::Active);
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
    let resp = post(store.clone(), path.clone(), "/api/championships/c1/rounds", b"");
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
    let resp = delete(store.clone(), path.clone(), "/api/championships/c1/rounds/0");
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

#[test]
fn test_route_post_session_to_round_adds_it() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds.push(Round::default());
        data.championships.push(champ);
    }
    let resp = post(store.clone(), path.clone(),
        "/api/championships/c1/rounds/0/sessions/sess1", b"");
    assert!(status_line(&resp).contains("200"));
    let data = store.read().unwrap();
    assert!(data.championships[0].rounds[0].session_ids.contains(&"sess1".to_string()));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_route_post_session_to_round_deduplicates() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds.push(Round { session_ids: vec!["sess1".into()] });
        data.championships.push(champ);
    }
    post(store.clone(), path.clone(),
        "/api/championships/c1/rounds/0/sessions/sess1", b"");
    assert_eq!(store.read().unwrap().championships[0].rounds[0].session_ids.len(), 1);
    let _ = std::fs::remove_file(&path);
}

// ── DELETE /api/championships/:id/rounds/:r/sessions/:sid ────────────────────

#[test]
fn test_route_delete_session_from_round_removes_it() {
    let (store, path) = make_test_store();
    {
        let mut data = store.write().unwrap();
        let mut champ = make_champ("c1");
        champ.rounds.push(Round { session_ids: vec!["s1".into(), "s2".into()] });
        data.championships.push(champ);
    }
    let resp = delete(store.clone(), path.clone(),
        "/api/championships/c1/rounds/0/sessions/s1");
    assert!(status_line(&resp).contains("200"));
    let data = store.read().unwrap();
    assert!(!data.championships[0].rounds[0].session_ids.contains(&"s1".to_string()));
    assert!(data.championships[0].rounds[0].session_ids.contains(&"s2".to_string()));
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
        champ.rounds.push(Round { session_ids: vec!["s1".into()] });
        data.championships.push(champ);
        for id in &["s1", "s2"] {
            data.sessions.push(RecordedSession {
                id: (*id).into(), recorded_at: 1000,
                track: "Spa".into(), track_variation: "GP".into(),
                car_name: String::new(), car_class: String::new(),
                session_type: 5, results: vec![], lap_chart: vec![],
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
    let few: serde_json::Value = serde_json::Value::Array(vec![serde_json::json!([0,0]); 10]);
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
    assert_eq!(segs, ["api", "championships", "42", "rounds", "0", "sessions", "7"]);
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
    assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
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
    assert_eq!(hex(&sha1(input)), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
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
    assert_eq!(u64::from_be_bytes(frame[2..10].try_into().unwrap()) as usize, 70_000);
    assert_eq!(&frame[10..], payload.as_slice());
}
