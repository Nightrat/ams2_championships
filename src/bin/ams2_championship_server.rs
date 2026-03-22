use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use ams2_championship::ams2_shared_memory::read_live_session;
use ams2_championship::data_store::{Championship, SharedStore, persist};

// ── HTTP primitives ───────────────────────────────────────────────────────────

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn parse_request(buf: &[u8]) -> Request {
    let line_end = buf
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(buf.len());
    let first_line = std::str::from_utf8(&buf[..line_end]).unwrap_or("");
    let mut parts = first_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("GET").to_owned();
    let path = parts.next().unwrap_or("/").to_owned();
    // Body follows the blank line (\r\n\r\n) that ends the headers.
    let body = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| buf[pos + 4..].to_vec())
        .unwrap_or_default();
    Request { method, path, body }
}

fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn json_ok(stream: &mut TcpStream, body: &[u8]) {
    send_response(stream, "200 OK", "application/json", body);
}

fn json_err(stream: &mut TcpStream, status: &str, msg: &str) {
    let body = format!("{{\"error\":\"{msg}\"}}");
    send_response(stream, status, "application/json", body.as_bytes());
}

// ── Request handler ───────────────────────────────────────────────────────────

fn handle(mut stream: TcpStream, html: Arc<Vec<u8>>, store: SharedStore, data_path: Arc<PathBuf>) {
    let mut buf = [0u8; 65536];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = parse_request(&buf[..n]);
    let path = req.path.as_str();
    let method = req.method.as_str();

    // GET /live — real-time telemetry
    if path == "/live" {
        let data = read_live_session();
        let json = serde_json::to_vec(&data).unwrap_or_else(|_| b"{}".to_vec());
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/sessions
    if method == "GET" && path == "/api/sessions" {
        let data = store.read().unwrap();
        let json = serde_json::to_vec(&data.sessions).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/championships
    if method == "GET" && path == "/api/championships" {
        let data = store.read().unwrap();
        let json = serde_json::to_vec(&data.championships).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // POST /api/championships — create
    if method == "POST" && path == "/api/championships" {
        #[derive(serde::Deserialize)]
        struct Body {
            name: String,
            #[serde(default)]
            points_system: Vec<i32>,
            #[serde(default)]
            manufacturer_scoring: bool,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        let champ = Championship {
            id,
            name: body.name,
            status: "Active".into(),
            points_system: if body.points_system.is_empty() {
                vec![25, 18, 15, 12, 10, 8, 6, 4, 2, 1]
            } else {
                body.points_system
            },
            manufacturer_scoring: body.manufacturer_scoring,
            rounds: vec![],
            session_ids: vec![],
        };
        let json = serde_json::to_vec(&champ).unwrap_or_default();
        store.write().unwrap().championships.push(champ);
        persist(&store, &data_path);
        json_ok(&mut stream, &json);
        return;
    }

    // DELETE /api/sessions/unassigned — remove all sessions not in any round
    if method == "DELETE" && path == "/api/sessions/unassigned" {
        let mut data = store.write().unwrap();
        let assigned: std::collections::HashSet<String> = data
            .championships
            .iter()
            .flat_map(|c| c.rounds.iter())
            .flat_map(|r| r.session_ids.iter().cloned())
            .collect();
        let before = data.sessions.len();
        data.sessions.retain(|s| assigned.contains(&s.id));
        let removed = before - data.sessions.len();
        drop(data);
        persist(&store, &data_path);
        let body = format!("{{\"removed\":{removed}}}");
        json_ok(&mut stream, body.as_bytes());
        return;
    }

    // Routes with path segments: /api/championships/:id[/sessions/:sid]
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    // PATCH /api/championships/:id
    if method == "PATCH" && segs.len() == 3 && segs[0] == "api" && segs[1] == "championships" {
        let id = segs[2];
        #[derive(serde::Deserialize)]
        struct Body {
            name: Option<String>,
            status: Option<String>,
            points_system: Option<Vec<i32>>,
            manufacturer_scoring: Option<bool>,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        let mut data = store.write().unwrap();
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        if let Some(name) = body.name { champ.name = name; }
        if let Some(status) = body.status { champ.status = status; }
        if let Some(ps) = body.points_system { champ.points_system = ps; }
        if let Some(ms) = body.manufacturer_scoring { champ.manufacturer_scoring = ms; }
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &data_path);
        json_ok(&mut stream, &json);
        return;
    }

    // DELETE /api/championships/:id
    if method == "DELETE" && segs.len() == 3 && segs[0] == "api" && segs[1] == "championships" {
        let id = segs[2];
        let mut data = store.write().unwrap();
        let before = data.championships.len();
        data.championships.retain(|c| c.id != id);
        if data.championships.len() == before {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        }
        drop(data);
        persist(&store, &data_path);
        json_ok(&mut stream, b"{}");
        return;
    }

    // POST /api/championships/:id/rounds — add a new empty round
    if method == "POST"
        && segs.len() == 4
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "rounds"
    {
        let id = segs[2];
        let mut data = store.write().unwrap();
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        champ.rounds.push(ams2_championship::data_store::Round::default());
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &data_path);
        json_ok(&mut stream, &json);
        return;
    }

    // DELETE /api/championships/:id/rounds/:ridx — remove a round
    if method == "DELETE"
        && segs.len() == 5
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "rounds"
    {
        let (id, ridx) = (segs[2], segs[4].parse::<usize>().unwrap_or(usize::MAX));
        let mut data = store.write().unwrap();
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        if ridx >= champ.rounds.len() {
            json_err(&mut stream, "404 Not Found", "round not found");
            return;
        }
        champ.rounds.remove(ridx);
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &data_path);
        json_ok(&mut stream, &json);
        return;
    }

    // POST /api/championships/:id/rounds/:ridx/sessions/:sid — add session to round
    if method == "POST"
        && segs.len() == 7
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "rounds"
        && segs[5] == "sessions"
    {
        let (id, ridx, sid) = (segs[2], segs[4].parse::<usize>().unwrap_or(usize::MAX), segs[6]);
        let mut data = store.write().unwrap();
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        if ridx >= champ.rounds.len() {
            json_err(&mut stream, "404 Not Found", "round not found");
            return;
        }
        let round = &mut champ.rounds[ridx];
        if !round.session_ids.contains(&sid.to_string()) {
            round.session_ids.push(sid.to_string());
        }
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &data_path);
        json_ok(&mut stream, &json);
        return;
    }

    // DELETE /api/championships/:id/rounds/:ridx/sessions/:sid — remove session from round
    if method == "DELETE"
        && segs.len() == 7
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "rounds"
        && segs[5] == "sessions"
    {
        let (id, ridx, sid) = (segs[2], segs[4].parse::<usize>().unwrap_or(usize::MAX), segs[6]);
        let mut data = store.write().unwrap();
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        if ridx >= champ.rounds.len() {
            json_err(&mut stream, "404 Not Found", "round not found");
            return;
        }
        champ.rounds[ridx].session_ids.retain(|s| s != sid);
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &data_path);
        json_ok(&mut stream, &json);
        return;
    }

    // Default: serve the static championship HTML
    send_response(&mut stream, "200 OK", "text/html; charset=utf-8", &html);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn req(raw: &[u8]) -> Request {
        parse_request(raw)
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
        let body = b"{\"status\":\"Finished\"}";
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
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(2).and_then(|p| p.parse().ok()).unwrap_or(8080);

    // career.json lives in a "championships" subfolder next to the executable.
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let champ_dir = exe_dir.join("championships");
    if let Err(e) = std::fs::create_dir_all(&champ_dir) {
        eprintln!("Failed to create championships directory: {e}");
        std::process::exit(1);
    }
    let career_path = champ_dir.join("ams2_career.json");

    let store = ams2_championship::data_store::load_store(&career_path);
    {
        let data = store.read().unwrap();
        println!(
            "Career data:    {} ({} championship(s), {} session(s))",
            career_path.display(),
            data.championships.len(),
            data.sessions.len()
        );
    }
    ams2_championship::session_recorder::start(store.clone(), career_path.clone());

    let html = Arc::new(match args.get(1) {
        Some(xml_path) => match ams2_championship::build_html_from_xml(xml_path) {
            Ok(h) => h.into_bytes(),
            Err(e) => {
                eprintln!("Warning: could not read XML ({e}) — SecondMonitor Import tab will be empty");
                ams2_championship::build_base_html().into_bytes()
            }
        },
        None => ams2_championship::build_base_html().into_bytes(),
    });

    let listener = match TcpListener::bind(format!("127.0.0.1:{port}")) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to port {port}: {e}");
            std::process::exit(1);
        }
    };

    println!("Serving at http://127.0.0.1:{port}/  (Ctrl+C to stop)");
    println!("Live endpoint:  http://127.0.0.1:{port}/live");
    println!("Career API:     http://127.0.0.1:{port}/api/sessions  |  /api/championships");

    let data_path = Arc::new(career_path);
    for stream in listener.incoming().flatten() {
        let html = Arc::clone(&html);
        let store = store.clone();
        let data_path = Arc::clone(&data_path);
        std::thread::spawn(move || handle(stream, html, store, data_path));
    }
}
