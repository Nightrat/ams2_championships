//! The HTTP front end: routes each request to an [`App`] operation and answers in JSON.
//!
//! Every rule lives in [`ams2_championship::service`]. What is left here is transport — matching
//! the path, parsing the body, mapping an [`AppError`] to a status code — so a second front end
//! calling the same `App` gets exactly the same behaviour.

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use ams2_championship::game::shared_memory::read_live_session;
use ams2_championship::career::data_store::CareerMode;
use ams2_championship::web::http::{
    json_err, json_ok, read_full_request, send_download, send_response, send_with_cache, url_decode,
};
use ams2_championship::service::{
    exe_dir, startup, App, AppError, AppResult, ChampionshipPatch, ConfigPatch, DriverAttrEdit,
    NewChampionship, SignRequest, SpotterPatch, TeamScalarsEdit,
};
use ams2_championship::web::websocket::handle_websocket;

// ── Transport helpers ────────────────────────────────────────────────────────────────────────

fn status_of(e: &AppError) -> &'static str {
    match e {
        AppError::BadRequest(_) => "400 Bad Request",
        AppError::NotFound(_) => "404 Not Found",
        AppError::Conflict(_) => "409 Conflict",
        AppError::Unsupported(_) => "415 Unsupported Media Type",
        AppError::Internal(_) => "500 Internal Server Error",
    }
}

fn fail(stream: &mut TcpStream, e: AppError) {
    json_err(stream, status_of(&e), e.message());
}

/// Answers with `value` as JSON, or with the error's status and message.
fn reply<T: serde::Serialize>(stream: &mut TcpStream, result: AppResult<T>) {
    match result {
        Ok(value) => json_ok(stream, &serde_json::to_vec(&value).unwrap_or_default()),
        Err(e) => fail(stream, e),
    }
}

/// Answers with a fixed body on success — for operations whose answer is only that they worked.
fn reply_with(stream: &mut TcpStream, result: AppResult<()>, body: &[u8]) {
    match result {
        Ok(()) => json_ok(stream, body),
        Err(e) => fail(stream, e),
    }
}

/// The request body as `T`. A body that does not parse is answered with "invalid body"; the
/// routes that have always reported serde's own message use [`parse_verbose`].
fn parse<T: serde::de::DeserializeOwned>(body: &[u8]) -> AppResult<T> {
    serde_json::from_slice(body).map_err(|_| AppError::BadRequest("invalid body".into()))
}

fn parse_verbose<T: serde::de::DeserializeOwned>(body: &[u8]) -> AppResult<T> {
    serde_json::from_slice(body).map_err(|e| AppError::BadRequest(e.to_string()))
}

/// Parses the body and hands it to `op`, answering a parse failure as [`reply`] would.
macro_rules! with_body {
    ($stream:expr, $parsed:expr, |$body:ident| $op:expr) => {
        match $parsed {
            Ok($body) => $op,
            Err(e) => {
                fail(&mut $stream, e);
                return;
            }
        }
    };
}

#[derive(serde::Deserialize)]
struct ClassReq {
    class: String,
}

#[derive(serde::Deserialize)]
struct RemoveEntryReq {
    class: String,
    /// Position among the file's `<driver>` blocks, from the GET payload.
    index: usize,
    /// The name that position is listed under — a stale-index guard.
    driver: String,
}

#[derive(serde::Deserialize)]
struct ModeReq {
    mode: CareerMode,
}

#[derive(serde::Deserialize)]
struct SaveReq {
    name: String,
    #[serde(default)]
    new_name: String,
    /// Required when creating: a career picks its kind up front and keeps it.
    #[serde(default)]
    mode: CareerMode,
}

#[derive(serde::Deserialize)]
struct RenameReq {
    new_name: String,
}

/// The spotter body is read loosely, as it always has been: a key holding anything other than a
/// string clears that setting rather than failing the request.
fn spotter_patch(v: &serde_json::Value) -> SpotterPatch {
    let text = |key: &str| {
        v.get(key).map(|x| match x {
            serde_json::Value::String(s) => Some(s.clone()),
            _ => None,
        })
    };
    SpotterPatch {
        enabled: v.get("enabled").and_then(serde_json::Value::as_bool),
        player: text("player"),
        voice: text("voice"),
    }
}

// ── Routes ───────────────────────────────────────────────────────────────────────────────────

fn handle(mut stream: TcpStream, html: Arc<Vec<u8>>, app: &App, poll_ms: u64) {
    let req = read_full_request(&mut stream);
    let path = req.path.as_str();
    let method = req.method.as_str();
    let body = req.body.as_slice();

    // WebSocket upgrade — /ws
    if path == "/ws"
        && req.headers.lines().any(|l| {
            let l = l.to_ascii_lowercase();
            l.starts_with("upgrade:") && l.contains("websocket")
        })
    {
        handle_websocket(stream, &req.headers, poll_ms, || {
            serde_json::to_vec(&app.live_frame()).ok()
        });
        return;
    }

    // ── Fixed paths ──────────────────────────────────────────────────────────
    match (method, path) {
        ("GET", "/api/live-teams") => return reply(&mut stream, Ok(app.live_teams())),
        // Real-time telemetry, kept for backwards compatibility.
        (_, "/live") => {
            let json = serde_json::to_vec(&read_live_session()).unwrap_or_else(|_| b"{}".to_vec());
            return json_ok(&mut stream, &json);
        }
        ("POST", "/api/live/ride-height/reset") => {
            app.reset_ride_record();
            return json_ok(&mut stream, b"{}");
        }
        ("GET", "/api/sessions") => return reply(&mut stream, Ok(app.sessions())),
        ("GET", "/api/career") => return reply(&mut stream, Ok(app.career_view())),
        // The whole career as one HTML file, offered as a download — see `service::export`.
        ("GET", "/api/career/export") => {
            let html = app.career_export_html();
            let name = app.career_export_file_name();
            return send_download(&mut stream, "text/html; charset=utf-8", &name, html.as_bytes());
        }
        ("GET", "/api/career/finances") => return reply(&mut stream, Ok(app.finances())),
        ("PATCH", "/api/career/mode") => {
            let parsed = parse::<ModeReq>(body);
            with_body!(stream, parsed, |b| match app.set_career_mode(b.mode) {
                Ok(mode) => {
                    let json = format!("{{\"mode\":\"{}\"}}", mode.label());
                    json_ok(&mut stream, json.as_bytes())
                }
                Err(e) => fail(&mut stream, e),
            });
            return;
        }
        ("POST", "/api/career/rating/adopt") => return reply(&mut stream, app.adopt_rating()),
        ("GET", "/api/custom-ai-files") => return reply(&mut stream, Ok(app.custom_ai_files())),

        ("GET", "/api/car-performance") => return reply(&mut stream, Ok(app.car_performance())),
        ("PATCH", "/api/car-performance") => {
            let parsed = parse::<TeamScalarsEdit>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.set_team_scalars(&b)));
            return;
        }
        ("POST", "/api/car-performance/baseline") => {
            let parsed = parse::<ClassReq>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.adopt_class_baseline(&b.class)));
            return;
        }
        ("POST", "/api/car-performance/reset") => {
            let parsed = parse::<ClassReq>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.reset_class_to_baseline(&b.class)));
            return;
        }

        ("GET", "/api/driver-performance") => {
            return reply(&mut stream, Ok(app.driver_performance()))
        }
        ("PATCH", "/api/driver-performance") => {
            let parsed = parse::<DriverAttrEdit>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.set_driver_attr(&b)));
            return;
        }
        ("DELETE", "/api/driver-performance") => {
            let parsed = parse::<RemoveEntryReq>(body);
            with_body!(stream, parsed, |b| reply(
                &mut stream,
                app.remove_driver_entry(&b.class, b.index, &b.driver)
            ));
            return;
        }
        ("DELETE", "/api/driver-performance/track-entries") => {
            let parsed = parse::<ClassReq>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.remove_track_entries(&b.class)));
            return;
        }

        ("GET", "/api/championships") => return reply(&mut stream, Ok(app.championships())),
        ("POST", "/api/championships") => {
            let parsed = parse::<NewChampionship>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.create_championship(b)));
            return;
        }
        ("DELETE", "/api/sessions/unassigned") => {
            match app.purge_unassigned_sessions() {
                Ok(n) => json_ok(&mut stream, format!("{{\"removed\":{n}}}").as_bytes()),
                Err(e) => fail(&mut stream, e),
            }
            return;
        }
        ("POST", "/api/record-session") => {
            return reply_with(&mut stream, app.record_current_session(), b"{\"ok\":true}")
        }

        ("GET", "/api/saves") => return reply(&mut stream, Ok(app.saves())),
        ("POST", "/api/saves") | ("POST", "/api/saves/activate") | ("POST", "/api/saves/duplicate") => {
            let parsed = parse_verbose::<SaveReq>(body);
            with_body!(stream, parsed, |b| {
                let result = match path {
                    "/api/saves" => app.create_save(&b.name, b.mode),
                    "/api/saves/activate" => app.activate_save(&b.name),
                    _ => app.duplicate_save(&b.name, &b.new_name),
                };
                reply(&mut stream, result)
            });
            return;
        }

        ("GET", "/api/config") => return reply(&mut stream, Ok(app.config_view())),
        ("PATCH", "/api/config") => {
            let parsed = parse_verbose::<ConfigPatch>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.patch_config(b)));
            return;
        }

        ("GET", "/api/spotter/voices") => {
            let voices = ams2_championship::game::spotter::list_voices();
            let json = serde_json::to_vec(&voices).unwrap_or_else(|_| b"[]".to_vec());
            return json_ok(&mut stream, &json);
        }
        ("GET", "/api/spotter") => return reply(&mut stream, Ok(app.spotter_status())),
        ("PATCH", "/api/spotter") => {
            match serde_json::from_slice::<serde_json::Value>(body) {
                Ok(v) => reply(&mut stream, Ok(app.patch_spotter(spotter_patch(&v)))),
                Err(_) => json_err(&mut stream, "400 Bad Request", "invalid JSON"),
            }
            return;
        }
        _ => {}
    }

    // The livery mod's own picture of one car, as a PNG. The one route that may be cached: the
    // file only changes when a livery mod is reinstalled, and until then every repaint of the
    // table would otherwise decode the whole grid again.
    if method == "GET" {
        if let Some(rest) = path.strip_prefix("/api/livery-preview/") {
            let requested = url_decode(rest.split('?').next().unwrap_or(""));
            match app.livery_preview_png(&requested) {
                Ok(png) => send_with_cache(
                    &mut stream,
                    "200 OK",
                    "image/png",
                    "public, max-age=3600",
                    &png,
                ),
                Err(e) => fail(&mut stream, e),
            }
            return;
        }
    }

    // ── Paths with segments ──────────────────────────────────────────────────
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let index = |s: &str| s.parse::<usize>().unwrap_or(usize::MAX);

    match (method, segs.as_slice()) {
        ("GET", ["api", "sessions", id, "lap-chart"]) => {
            reply(&mut stream, Ok(app.lap_chart(&url_decode(id))))
        }

        ("PATCH", ["api", "championships", id]) => {
            let parsed = parse::<ChampionshipPatch>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.patch_championship(id, b)));
        }
        ("DELETE", ["api", "championships", id]) => {
            reply_with(&mut stream, app.delete_championship(id), b"{}")
        }
        ("GET", ["api", "championships", id, "teams"]) => {
            reply(&mut stream, app.championship_teams(id))
        }
        ("GET", ["api", "championships", id, "team-eligibility"]) => {
            reply(&mut stream, app.team_eligibility(id))
        }
        ("GET", ["api", "championships", id, "offers"]) => reply(&mut stream, app.offers(id)),
        ("POST", ["api", "championships", id, "sign"]) => {
            let parsed = parse::<SignRequest>(body);
            with_body!(stream, parsed, |b| reply(&mut stream, app.sign(id, &b)));
        }
        ("DELETE", ["api", "championships", id, "sign"]) => {
            reply_with(&mut stream, app.unsign(id), b"{\"released\":true}")
        }
        ("POST", ["api", "championships", id, "roster", "restore"]) => {
            reply_with(&mut stream, app.restore_roster(id), b"{\"restored\":true}")
        }
        ("GET", ["api", "championships", id, "grid-check"]) => {
            reply(&mut stream, app.grid_check(id))
        }
        ("GET", ["api", "championships", id, "available-sessions"]) => {
            reply(&mut stream, app.available_sessions(id))
        }
        ("GET", ["api", "championships", id, "session-eligibility"]) => {
            reply(&mut stream, app.session_eligibility(id))
        }
        ("POST", ["api", "championships", id, "rounds"]) => reply(&mut stream, app.add_round(id)),
        ("DELETE", ["api", "championships", id, "rounds", ridx]) => {
            reply(&mut stream, app.remove_round(id, index(ridx)))
        }
        ("POST", ["api", "championships", id, "rounds", ridx, "sessions", sid]) => {
            reply(&mut stream, app.add_session_to_round(id, index(ridx), sid))
        }
        ("DELETE", ["api", "championships", id, "rounds", ridx, "sessions", sid]) => {
            reply(&mut stream, app.remove_session_from_round(id, index(ridx), sid))
        }

        ("PATCH", ["api", "saves", name]) => {
            let parsed = parse_verbose::<RenameReq>(body);
            with_body!(stream, parsed, |b| reply(
                &mut stream,
                app.rename_save(&url_decode(name), &b.new_name)
            ));
        }
        ("DELETE", ["api", "saves", name]) => {
            reply(&mut stream, app.delete_save(&url_decode(name)))
        }

        ("GET", ["api", "track-layout", track]) => match app.track_layout(track) {
            Some(content) => json_ok(&mut stream, &content),
            None => json_ok(&mut stream, b"null"),
        },
        ("POST", ["api", "track-layout", track]) => {
            reply_with(&mut stream, app.save_track_layout(track, body), b"{}")
        }

        // Default: serve the static championship HTML
        _ => send_response(&mut stream, "200 OK", "text/html; charset=utf-8", &html),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../tests/server.rs"]
mod tests;

// ── Entry point ───────────────────────────────────────────────────────────────

/// The address a local browser should use. A wildcard bind is reachable on loopback, but
/// `http://0.0.0.0/` is not an address a browser can open.
fn browser_url(host: &str, port: u16) -> String {
    let host = match host.trim() {
        "" | "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        h => h,
    };
    format!("http://{host}:{port}/")
}

/// Hands the URL to the system's default browser. A failure is reported and otherwise ignored:
/// the server is just as usable from a browser opened by hand.
fn open_in_browser(url: &str) {
    #[cfg(windows)]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = result {
        eprintln!("Could not open a browser at {url}: {e}");
    }
}

fn main() {
    let started = match startup(&exe_dir()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    // Held until the process ends: this server owns the saves folder while it runs.
    let _lock = started.lock;
    let cfg = started.config;
    let app = Arc::new(started.app);

    let html = Arc::new(ams2_championship::web::html::build_base_html().into_bytes());
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

    // Only after the bind succeeded, so the page never opens onto a server that is not there.
    if cfg.open_browser {
        open_in_browser(&browser_url(&cfg.host, cfg.port));
    }

    let poll_ms = cfg.poll_ms;
    for stream in listener.incoming().flatten() {
        let html = Arc::clone(&html);
        let app = Arc::clone(&app);
        std::thread::spawn(move || handle(stream, html, &app, poll_ms));
    }
}
