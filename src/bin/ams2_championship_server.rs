use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use ams2_championship::ams2_shared_memory::read_live_session;
use ams2_championship::data_store::{Championship, ChampionshipStatus, SharedStore, persist, compute_career};

// ── HTTP primitives ───────────────────────────────────────────────────────────

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
    headers: String,
}

#[cfg(test)]
fn parse_request(buf: &[u8]) -> Request {
    let line_end = buf
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(buf.len());
    let first_line = std::str::from_utf8(&buf[..line_end]).unwrap_or("");
    let mut parts = first_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("GET").to_owned();
    let path = parts.next().unwrap_or("/").to_owned();
    let header_end = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4).unwrap_or(buf.len());
    let headers = std::str::from_utf8(&buf[..header_end]).unwrap_or("").to_owned();
    let body = buf.get(header_end..).unwrap_or(&[]).to_vec();
    Request { method, path, body, headers }
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

fn read_full_request(stream: &mut TcpStream) -> Request {
    let mut raw: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];

    // Read until we have the full headers (\r\n\r\n).
    loop {
        let n = stream.read(&mut tmp).unwrap_or(0);
        if n == 0 { break; }
        raw.extend_from_slice(&tmp[..n]);
        if raw.windows(4).any(|w| w == b"\r\n\r\n") { break; }
    }

    // Parse method and path from the first line.
    let line_end = raw.iter().position(|&b| b == b'\r' || b == b'\n').unwrap_or(raw.len());
    let first_line = std::str::from_utf8(&raw[..line_end]).unwrap_or("");
    let mut parts = first_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("GET").to_owned();
    let path   = parts.next().unwrap_or("/").to_owned();

    // Find where the body starts.
    let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(raw.len());

    // Parse Content-Length from headers.
    let headers_str = std::str::from_utf8(&raw[..header_end]).unwrap_or("");
    let content_length: usize = headers_str
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split_once(':').map(|x| x.1))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);

    // Read remaining body bytes until Content-Length is satisfied.
    let mut body = raw[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).unwrap_or(0);
        if n == 0 { break; }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);

    Request { method, path, body, headers: headers_str.to_owned() }
}

/// Turns a track name into a safe filename stem (e.g. "Spa – GP" → "spa_gp").
fn track_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 { msg.push(0); }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i*4], chunk[i*4+1], chunk[i*4+2], chunk[i*4+3]]);
        }
        for i in 16..80 { w[i] = (w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16]).rotate_left(1); }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        #[allow(clippy::needless_range_loop)]
        for i in 0..80 {
            let (f, k) = match i {
                0..=19  => ((b & c) | (!b & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d,           0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _       => (b ^ c ^ d,           0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(w[i]);
            e = d; d = c; c = b.rotate_left(30); b = a; a = t;
        }
        h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d); h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, &v) in h.iter().enumerate() { out[i*4..i*4+4].copy_from_slice(&v.to_be_bytes()); }
    out
}

fn base64_encode(data: &[u8]) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(C[(n >> 18) as usize] as char);
        out.push(C[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { C[((n >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { C[(n & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn ws_accept_key(key: &str) -> String {
    let mut combined = key.as_bytes().to_vec();
    combined.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64_encode(&sha1(&combined))
}

fn ws_send_text(stream: &mut TcpStream, payload: &[u8]) -> std::io::Result<()> {
    let len = payload.len();
    let mut frame = Vec::with_capacity(len + 10);
    frame.push(0x81); // FIN + text opcode
    if len < 126 {
        frame.push(len as u8);
    } else if len < 65536 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)
}

fn handle_websocket(mut stream: TcpStream, headers: &str, poll_ms: u64) {
    let key = headers.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("sec-websocket-key:"))
        .and_then(|l| l.split_once(':').map(|x| x.1))
        .map(|v| v.trim())
        .unwrap_or("");
    if key.is_empty() { return; }

    let accept = ws_accept_key(key);
    let handshake = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    if stream.write_all(handshake.as_bytes()).is_err() { return; }

    loop {
        let data = read_live_session();
        match serde_json::to_vec(&data) {
            Ok(json) => { if ws_send_text(&mut stream, &json).is_err() { break; } }
            Err(_)   => break,
        }
        std::thread::sleep(std::time::Duration::from_millis(poll_ms));
    }
}

fn handle(
    mut stream: TcpStream,
    html: Arc<Vec<u8>>,
    store: SharedStore,
    data_path: Arc<PathBuf>,
    layouts_dir: Arc<PathBuf>,
    config_path: Arc<PathBuf>,
    poll_ms: u64,
) {
    let req = read_full_request(&mut stream);
    let path = req.path.as_str();
    let method = req.method.as_str();

    // WebSocket upgrade — /ws
    if path == "/ws" && req.headers.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("upgrade:") && l.contains("websocket")
    }) {
        handle_websocket(stream, &req.headers, poll_ms);
        return;
    }

    // GET /live — real-time telemetry (kept for backwards compatibility)
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

    // GET /api/career — pre-computed standings, constructor standings, career stats
    if method == "GET" && path == "/api/career" {
        let data = store.read().unwrap();
        let career = compute_career(&data.championships, &data.sessions);
        let json = serde_json::to_vec(&career).unwrap_or_default();
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
            status: ChampionshipStatus::Active,
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
            status: Option<ChampionshipStatus>,
            points_system: Option<Vec<i32>>,
            manufacturer_scoring: Option<bool>,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        let mut data = store.write().unwrap();
        if !data.championships.iter().any(|c| c.id == id) {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        }
        // Only one championship may be Active at a time.
        if body.status == Some(ChampionshipStatus::Active) {
            for c in data.championships.iter_mut() {
                if c.id != id && c.status == ChampionshipStatus::Active {
                    c.status = ChampionshipStatus::Progress;
                }
            }
        }
        let champ = data.championships.iter_mut().find(|c| c.id == id).unwrap();
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


    // POST /api/record-session — manually capture the current live session
    if method == "POST" && path == "/api/record-session" {
        match ams2_championship::session_recorder::capture_current(&store, &data_path) {
            Ok(()) => json_ok(&mut stream, b"{\"ok\":true}"),
            Err(e) => json_err(&mut stream, "409 Conflict", &e),
        }
        return;
    }

    // GET /api/config
    if method == "GET" && path == "/api/config" {
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let json = serde_json::to_vec(&cfg).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // PATCH /api/config
    if method == "PATCH" && path == "/api/config" {
        #[derive(serde::Deserialize)]
        struct PatchConfig {
            port: u16, host: String, data_file: Option<String>,
            poll_ms: u64, record_practice: bool, record_qualify: bool, record_race: bool,
            show_track_map: bool, track_map_max_points: u32, move_data_file: bool,
        }
        let req_body: PatchConfig = match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => { json_err(&mut stream, "400 Bad Request", &e.to_string()); return; }
        };

        let old_cfg = ams2_championship::config::load_or_create(&config_path);

        // Determine which fields require a restart
        let mut restart_required: Vec<&'static str> = vec![];
        if req_body.port    != old_cfg.port                    { restart_required.push("port"); }
        if req_body.host    != old_cfg.host                    { restart_required.push("host"); }
        if req_body.data_file != old_cfg.data_file             { restart_required.push("data_file"); }

        // Optionally move the data file
        let mut moved = false;
        if req_body.move_data_file && req_body.data_file != old_cfg.data_file {
            let new_dest = req_body.data_file.as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    config_path.parent().unwrap_or_else(|| std::path::Path::new("."))
                        .join("championships").join("ams2_career.json")
                });
            if let Err(e) = std::fs::rename(data_path.as_ref(), &new_dest) {
                json_err(&mut stream, "500 Internal Server Error", &format!("move failed: {e}"));
                return;
            }
            moved = true;
        }

        let new_cfg = ams2_championship::config::Config {
            port: req_body.port,
            host: req_body.host,
            data_file: req_body.data_file,
            poll_ms: req_body.poll_ms,
            record_practice: req_body.record_practice,
            record_qualify:  req_body.record_qualify,
            record_race:     req_body.record_race,
            show_track_map: req_body.show_track_map,
            track_map_max_points: req_body.track_map_max_points,
        };
        match serde_json::to_string_pretty(&new_cfg) {
            Ok(text) => { if let Err(e) = std::fs::write(config_path.as_ref(), text) {
                json_err(&mut stream, "500 Internal Server Error", &e.to_string());
                return;
            }}
            Err(e) => { json_err(&mut stream, "500 Internal Server Error", &e.to_string()); return; }
        }

        #[derive(serde::Serialize)]
        struct PatchResponse<'a> {
            config: &'a ams2_championship::config::Config,
            restart_required: Vec<&'static str>,
            moved: bool,
        }
        let resp = PatchResponse { config: &new_cfg, restart_required, moved };
        let json = serde_json::to_vec(&resp).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/track-layout/:track — load saved layout points from file
    if method == "GET" && segs.len() == 3 && segs[0] == "api" && segs[1] == "track-layout" {
        let file = layouts_dir.join(format!("{}.json", track_slug(segs[2])));
        if file.exists() {
            let content = std::fs::read(&file).unwrap_or_default();
            json_ok(&mut stream, &content);
        } else {
            json_ok(&mut stream, b"null");
        }
        return;
    }

    // POST /api/track-layout/:track — save layout points to file
    if method == "POST" && segs.len() == 3 && segs[0] == "api" && segs[1] == "track-layout" {
        // Reject payloads with too few points (must have at least 300 entries).
        let count = serde_json::from_slice::<serde_json::Value>(&req.body)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0);
        if count < 300 {
            json_err(&mut stream, "400 Bad Request", "too few points");
            return;
        }
        let file = layouts_dir.join(format!("{}.json", track_slug(segs[2])));
        if let Err(e) = std::fs::write(&file, &req.body) {
            json_err(&mut stream, "500 Internal Server Error", &e.to_string());
        } else {
            json_ok(&mut stream, b"{}");
        }
        return;
    }


    // Default: serve the static championship HTML
    send_response(&mut stream, "200 OK", "text/html; charset=utf-8", &html);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../tests/server.rs"]
mod tests;

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    // ── Directories (next to the executable) ─────────────────────────────────
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let champ_dir = exe_dir.join("championships");
    if let Err(e) = std::fs::create_dir_all(&champ_dir) {
        eprintln!("Failed to create championships directory: {e}");
        std::process::exit(1);
    }
    let layouts_dir = Arc::new(champ_dir.join("track_layouts"));
    if let Err(e) = std::fs::create_dir_all(layouts_dir.as_ref()) {
        eprintln!("Failed to create track_layouts directory: {e}");
        std::process::exit(1);
    }

    // ── Config ────────────────────────────────────────────────────────────────
    let config_path = exe_dir.join("config.json");
    let cfg = ams2_championship::config::load_or_create(&config_path);

    let career_path = cfg.data_file
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| champ_dir.join("ams2_career.json"));

    // ── Data store ────────────────────────────────────────────────────────────
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
    ams2_championship::session_recorder::start(store.clone(), career_path.clone(), cfg.record_practice, cfg.record_qualify, cfg.record_race);

    // ── HTTP server ───────────────────────────────────────────────────────────
    let html = Arc::new(ams2_championship::build_base_html().into_bytes());
    let addr = format!("{}:{}", cfg.host, cfg.port);

    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to {addr}: {e}");
            std::process::exit(1);
        }
    };

    println!("Serving at http://{addr}/  (Ctrl+C to stop)");
    println!("Live endpoint:  http://{addr}/live");
    println!("Career API:     http://{addr}/api/sessions  |  /api/championships");

    let data_path = Arc::new(career_path);
    let config_path = Arc::new(config_path);
    let poll_ms = cfg.poll_ms;
    for stream in listener.incoming().flatten() {
        let html = Arc::clone(&html);
        let store = store.clone();
        let data_path = Arc::clone(&data_path);
        let layouts_dir = Arc::clone(&layouts_dir);
        let config_path = Arc::clone(&config_path);
        std::thread::spawn(move || handle(stream, html, store, data_path, layouts_dir, config_path, poll_ms));
    }
}
