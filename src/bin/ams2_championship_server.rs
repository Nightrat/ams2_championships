use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use ams2_championship::ams2_shared_memory::read_live_session;
use ams2_championship::data_store::{
    compute_career_full, persist, Championship, ChampionshipStatus, SavePath, SharedStore,
};
use ams2_championship::http::{
    json_err, json_ok, read_full_request, send_response, track_slug, url_decode,
};
use ams2_championship::spotter::Focus;
use ams2_championship::websocket::handle_websocket;

/// The configured Custom AI Drivers folder, if one is set and non-empty.
fn cfg_custom_ai_dir(config_path: &std::path::Path) -> Option<PathBuf> {
    ams2_championship::config::load_or_create(config_path)
        .custom_ai_dir
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from)
}

/// Everything needed to rate and rank one car class.
struct ClassData {
    perf: ams2_championship::custom_ai::ClassPerformance,
    ctx: ams2_championship::driver_rating::RatingContext,
    /// Weaker incumbent's `race_skill` per team.
    skills: std::collections::HashMap<String, f32>,
}

/// Loads every readable car class from the configured Custom AI Drivers folder.
///
/// Ratings are measured against where a car *should* finish, which is a function of how many
/// cars are ahead of it — so the seat list has to be the grid AMS2 will actually field. Entries
/// whose livery the game does not own never start, and counting them stretches every expected
/// position down the order, making the whole field look easier to beat than it was.
fn load_classes(config_path: &std::path::Path) -> Vec<ClassData> {
    use ams2_championship::{custom_ai, driver_rating};
    let Some(dir) = cfg_custom_ai_dir(config_path) else {
        return vec![];
    };
    // One scan for every class: the manifests are per car model, not per class.
    let installed = ams2_championship::liveries::installed_livery_names(&dir);
    custom_ai::class_performance(&dir)
        .into_iter()
        .map(|perf| {
            let file = format!("{}.xml", perf.class);
            let path = dir.join(&file);
            let pace: std::collections::HashMap<String, f32> = perf
                .cars
                .iter()
                .map(|c| (c.team.clone(), c.pace_delta_pct))
                .collect();
            let ctx = driver_rating::RatingContext::new(
                &perf.class,
                roster_seats_with(&dir, &file, installed.as_ref()),
                &pace,
            );
            ClassData {
                skills: custom_ai::parse_team_skills(&path),
                ctx,
                perf,
            }
        })
        .collect()
}

/// The `/api/car-performance` payload: every class ranked, each team's required rating, and the
/// career rating of every human recorded in any of those classes.
///
/// Built fresh from the XML files on each call, so a scalar edit is reflected immediately —
/// nothing about this table is cached.
fn car_performance_json(config_path: &std::path::Path, store: &SharedStore) -> Vec<u8> {
    use ams2_championship::{custom_ai, driver_rating};

    #[derive(serde::Serialize)]
    struct CarRow {
        #[serde(flatten)]
        car: custom_ai::CarPerformanceRow,
        /// Reputation needed to claim this seat, 0–100. `None` when the team has no seat AMS2
        /// can actually field, so there is no seat to claim — distinct from a requirement of 0,
        /// which would read as "anyone may drive this".
        required_rating: Option<f32>,
        /// True when every one of the team's entries names a livery AMS2 does not own.
        phantom: bool,
    }
    #[derive(serde::Serialize)]
    struct PlayerRow {
        name: String,
        rating: f32,
        sp_races: u32,
        mp_races: u32,
        mp_wins: u32,
        mp_losses: u32,
    }
    #[derive(serde::Serialize)]
    struct ClassRow {
        class: String,
        year: Option<u16>,
        cars: Vec<CarRow>,
    }
    #[derive(serde::Serialize)]
    struct Body {
        /// Career ratings across every class, not per class.
        players: Vec<PlayerRow>,
        /// Classes the user is actually racing, for the tab's initial filter selection.
        active_classes: Vec<String>,
        /// The range the writer will accept, so the table enforces the same bounds the server
        /// does instead of keeping its own copy that can drift out of step.
        scalar_min: f32,
        scalar_max: f32,
        classes: Vec<ClassRow>,
    }

    let classes = load_classes(config_path);
    let params = ams2_championship::config::load_or_create(config_path).rating_params();
    // Only sessions committed to a championship are rated.
    let (sessions, active) = {
        let data = store.read().unwrap();
        (
            driver_rating::assigned_sessions(&data.championships, &data.sessions),
            ams2_championship::data_store::active_classes(&data.championships),
        )
    };

    let class_rows: Vec<ClassRow> = classes
        .iter()
        .map(|cd| {
            let required: std::collections::HashMap<String, f32> =
                driver_rating::team_requirements_with(&params, &cd.ctx.expected, &cd.skills)
                    .into_iter()
                    .collect();
            // `ctx.seats` is the phantom-filtered list, so a team missing from it has no car
            // AMS2 will field. The ranking table still lists it — it is in the roster, and its
            // scalars are still editable — but it has no seat to demand a rating for.
            let real_teams: std::collections::HashSet<&str> =
                cd.ctx.seats.iter().map(|s| s.team.as_str()).collect();
            ClassRow {
                class: cd.perf.class.clone(),
                year: cd.perf.year,
                cars: cd
                    .perf
                    .cars
                    .iter()
                    .map(|c| {
                        let real = real_teams.contains(c.team.as_str());
                        CarRow {
                            required_rating: if real {
                                Some(required.get(&c.team).copied().unwrap_or(0.0))
                            } else {
                                None
                            },
                            phantom: !real,
                            car: c.clone(),
                        }
                    })
                    .collect(),
            }
        })
        .collect();

    let contexts: Vec<driver_rating::RatingContext> = classes.into_iter().map(|c| c.ctx).collect();
    let players: Vec<PlayerRow> = driver_rating::recorded_players_global(&sessions, &contexts)
        .into_iter()
        .map(|name| {
            let r = driver_rating::compute_reputation_global_with(
                Some(&name),
                &sessions,
                &contexts,
                None,
                &params,
            );
            PlayerRow {
                name,
                rating: r.value,
                sp_races: r.sp_races,
                mp_races: r.mp_races,
                mp_wins: r.mp_wins,
                mp_losses: r.mp_losses,
            }
        })
        .collect();

    serde_json::to_vec(&Body {
        players,
        active_classes: active,
        scalar_min: custom_ai::SCALAR_MIN,
        scalar_max: custom_ai::SCALAR_MAX,
        classes: class_rows,
    })
    .unwrap_or_default()
}

/// Resolves `<class>.xml` inside the configured Custom AI Drivers folder.
///
/// A class name reaches us as a file name, so it goes through the same gate as a save name.
/// `Err` carries a status and message ready for `json_err`.
fn class_file(
    config_path: &std::path::Path,
    class: &str,
) -> Result<PathBuf, (&'static str, String)> {
    let dir = cfg_custom_ai_dir(config_path).ok_or((
        "400 Bad Request",
        "no Custom AI Drivers folder is configured".to_string(),
    ))?;
    let name = ams2_championship::saves::sanitize_name(class)
        .ok_or(("400 Bad Request", "invalid class name".to_string()))?;
    let file = dir.join(format!("{name}.xml"));
    if !file.is_file() {
        return Err(("404 Not Found", "no such car class file".to_string()));
    }
    Ok(file)
}

/// Seats a roster actually offers: the Custom AI file's entries minus any whose livery AMS2 does
/// not own.
///
/// Everything that reasons about the grid goes through here — both enforcement paths and the
/// rating contexts. A phantom seat can never be occupied by an AI, so leaving it in makes it look
/// free in every session forever (corrupting the elimination `infer_player_seat` relies on) and
/// inflates the field size every expected finishing position is derived from.
///
/// Takes the installed set rather than reading it, so a caller looping over every class scans the
/// manifests once instead of once per class.
fn roster_seats_with(
    dir: &std::path::Path,
    file: &str,
    installed: Option<&std::collections::HashSet<String>>,
) -> Vec<ams2_championship::custom_ai::SeatEntry> {
    let seats = ams2_championship::custom_ai::parse_seats(&dir.join(file));
    ams2_championship::custom_ai::without_phantom_seats(seats, installed)
}

/// [`roster_seats_with`] for a one-off lookup, reading the manifests itself.
fn roster_seats(dir: &std::path::Path, file: &str) -> Vec<ams2_championship::custom_ai::SeatEntry> {
    let installed = ams2_championship::liveries::installed_livery_names(dir);
    roster_seats_with(dir, file, installed.as_ref())
}

/// The `/api/live-teams` payload.
#[derive(serde::Serialize, Default, Debug)]
struct LiveTeams {
    /// Driver name -> livery/team name, from the winning championship's Custom AI file.
    teams: std::collections::HashMap<String, String>,
    /// Manual override for the player's row — their profile name won't be in the file.
    player_team: Option<String>,
}

/// Team names for the live timing grid, taken from the **active** championship.
///
/// AMS2's shared memory carries no team field, so the names come from a championship's Custom AI
/// Driver file. Which championship that is is not a guess: exactly one may be `Active` at a time —
/// `PATCH /api/championships/{id}` demotes any other to `Progress` — and that is the one being
/// raced, which is why the Manage tab opens on it too.
///
/// Scoring rosters against whoever happens to be on track cannot beat that, and was the earlier
/// bug here: two historic seasons of one series share most of their drivers, so a variant roster
/// carrying a few extra optional entries out-scores the season actually being driven and takes its
/// player team away with it — leaving the player's row showing the AMS2 car model.
///
/// Empty when no championship is active or the active one has no Custom AI file assigned; the grid
/// then falls back to the car names AMS2 reports.
fn resolve_live_teams(dir: &std::path::Path, champs: &[Championship]) -> LiveTeams {
    let Some(champ) = champs
        .iter()
        .find(|c| c.status == ChampionshipStatus::Active)
    else {
        return LiveTeams::default();
    };
    let Some(file) = champ.custom_ai_file.as_deref() else {
        return LiveTeams::default();
    };
    LiveTeams {
        teams: ams2_championship::custom_ai::parse_driver_teams(&dir.join(file)),
        player_team: champ.player_team.clone().filter(|t| !t.trim().is_empty()),
    }
}

/// The `/api/driver-performance` payload: every class in the same chronological order the Car
/// Performance tab uses, each with its named driver entries.
///
/// `attrs` is the server's own editable-tag list, so the table's columns and the writer's
/// allowlist cannot drift apart.
fn driver_performance_json(config_path: &std::path::Path, store: &SharedStore) -> Vec<u8> {
    use ams2_championship::custom_ai;

    #[derive(serde::Serialize)]
    struct ClassRow {
        class: String,
        year: Option<u16>,
        drivers: Vec<custom_ai::DriverAttributes>,
    }
    #[derive(serde::Serialize)]
    struct Body {
        attrs: Vec<&'static str>,
        /// What each attribute contributes to the Rating column, so the table can show its own
        /// formula instead of asking the reader to take the number on trust.
        rating_weights: Vec<(&'static str, f32)>,
        /// Per-attribute accepted range, since vehicle_reliability is documented as the one
        /// that may leave 0–1. Each is (field, min, max).
        attr_ranges: Vec<(&'static str, f32, f32)>,

        /// Classes the user is actually racing, for the tab's initial filter selection.
        active_classes: Vec<String>,
        classes: Vec<ClassRow>,
    }

    let classes = match cfg_custom_ai_dir(config_path) {
        Some(dir) => {
            // Read once for the whole payload: the manifests cover every car model at once, and
            // a livery belongs to a model rather than to a class.
            let installed = ams2_championship::liveries::installed_livery_names(&dir);
            custom_ai::class_performance(&dir)
                .into_iter()
                .map(|c| {
                    let mut drivers =
                        custom_ai::parse_driver_attributes(&dir.join(format!("{}.xml", c.class)));
                    custom_ai::mark_phantom_entries(&mut drivers, installed.as_ref());
                    ClassRow {
                        drivers,
                        class: c.class,
                        year: c.year,
                    }
                })
                .collect()
        }
        None => vec![],
    };
    serde_json::to_vec(&Body {
        attrs: custom_ai::DRIVER_ATTRS.to_vec(),
        rating_weights: custom_ai::RATING_WEIGHTS.to_vec(),
        attr_ranges: custom_ai::attr_ranges(),

        active_classes: ams2_championship::data_store::active_classes(
            &store.read().unwrap().championships,
        ),
        classes,
    })
    .unwrap_or_default()
}

/// Driver rating and per-team eligibility for a championship.
///
/// `None` when the championship has no Custom AI file, or no folder is configured — without a
/// roster there are no teams, no car pace figures, and nothing to rate against.
fn champ_eligibility(
    config_path: &std::path::Path,
    champ: &Championship,
    champs: &[Championship],
    sessions: &[ams2_championship::data_store::RecordedSession],
) -> Option<(
    ams2_championship::driver_rating::Reputation,
    Vec<ams2_championship::driver_rating::TeamEligibility>,
)> {
    use ams2_championship::driver_rating;

    let file = champ.custom_ai_file.as_deref()?;
    let class = std::path::Path::new(file).file_stem()?.to_str()?;
    let classes = load_classes(config_path);
    let own = classes.iter().position(|c| c.perf.class == class)?;
    let expected = classes[own].ctx.expected.clone();
    let skills = classes[own].skills.clone();

    // The rating spans the driver's whole career — a seat is earned by racing, not by racing
    // this particular car — while the requirement comes from this class's own grid. Only
    // sessions committed to a championship count toward it.
    let contexts: Vec<driver_rating::RatingContext> = classes.into_iter().map(|c| c.ctx).collect();
    let rated = driver_rating::assigned_sessions(champs, sessions);
    let params = ams2_championship::config::load_or_create(config_path).rating_params();
    let reputation = driver_rating::compute_reputation_global_with(
        None,
        &rated,
        &contexts,
        champ.player_team.as_deref(),
        &params,
    );
    let eligibility =
        driver_rating::team_eligibility_with(&params, reputation.value, &expected, &skills);
    Some((reputation, eligibility))
}

/// The currently active save file. Cloned out of the shared lock so the guard is never held
/// across a file write.
fn cur(data_path: &SavePath) -> PathBuf {
    data_path.read().map(|p| p.clone()).unwrap_or_default()
}

/// Persist `data_file` back to config.json so the active save survives a restart.
/// Mirrors the inline write used by `PATCH /api/spotter`.
fn store_active_save(config_path: &std::path::Path, file: &std::path::Path) -> Result<(), String> {
    let mut cfg = ams2_championship::config::load_or_create(config_path);
    cfg.data_file = Some(file.display().to_string());
    let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(config_path, text).map_err(|e| e.to_string())
}

/// `{ active, saves: [...] }` — the payload every /api/saves route answers with.
fn saves_payload(saves_dir: &std::path::Path, active: &std::path::Path) -> Vec<u8> {
    #[derive(serde::Serialize)]
    struct SavesResponse {
        active: String,
        /// Folder in use by the running server — may lag config.json until a restart.
        dir: String,
        saves: Vec<ams2_championship::saves::SaveInfo>,
    }
    let saves = ams2_championship::saves::list_saves(saves_dir, active);
    let active_name = saves
        .iter()
        .find(|s| s.active)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let resp = SavesResponse {
        active: active_name,
        dir: saves_dir.display().to_string(),
        saves,
    };
    serde_json::to_vec(&resp).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn handle(
    mut stream: TcpStream,
    html: Arc<Vec<u8>>,
    store: SharedStore,
    data_path: SavePath,
    saves_dir: Arc<PathBuf>,
    layouts_dir: Arc<PathBuf>,
    config_path: Arc<PathBuf>,
    poll_ms: u64,
    spotter_focus: Focus,
) {
    let req = read_full_request(&mut stream);
    let path = req.path.as_str();
    let method = req.method.as_str();

    // WebSocket upgrade — /ws
    if path == "/ws"
        && req.headers.lines().any(|l| {
            let l = l.to_ascii_lowercase();
            l.starts_with("upgrade:") && l.contains("websocket")
        })
    {
        handle_websocket(stream, &req.headers, poll_ms);
        return;
    }

    // GET /api/live-teams — driver -> team/livery names for the live timing grid, from the
    // active championship's Custom AI file. See `resolve_live_teams`. Without a Custom AI folder
    // there is no roster at all, and the grid falls back to the car names AMS2 reports.
    if method == "GET" && path == "/api/live-teams" {
        let best = match cfg_custom_ai_dir(&config_path) {
            Some(dir) => resolve_live_teams(&dir, &store.read().unwrap().championships),
            None => LiveTeams::default(),
        };
        let json = serde_json::to_vec(&best).unwrap_or_default();
        json_ok(&mut stream, &json);
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
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let ai_dir = cfg.custom_ai_dir.as_deref().map(PathBuf::from);
        let career = compute_career_full(&data.championships, &data.sessions, ai_dir.as_deref());
        let json = serde_json::to_vec(&career).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/custom-ai-files — list *.xml files in the configured Custom AI Drivers folder
    if method == "GET" && path == "/api/custom-ai-files" {
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let files = match cfg.custom_ai_dir {
            Some(dir) => ams2_championship::custom_ai::list_files(std::path::Path::new(&dir)),
            None => vec![],
        };
        let json = serde_json::to_vec(&files).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/car-performance — ranked power/weight/drag table per car class, each team's
    // required driver rating, and the rating of every human recorded in that class.
    if method == "GET" && path == "/api/car-performance" {
        json_ok(&mut stream, &car_performance_json(&config_path, &store));
        return;
    }

    // PATCH /api/car-performance — write one team's scalars back into its Custom AI XML file.
    // Answers with the whole recomputed table: an edit shifts the class baseline, every other
    // car's pace delta, and the ratings derived from them.
    if method == "PATCH" && path == "/api/car-performance" {
        #[derive(serde::Deserialize)]
        struct Body {
            class: String,
            team: String,
            power_scalar: f32,
            weight_scalar: f32,
            drag_scalar: f32,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        let file = match class_file(&config_path, &body.class) {
            Ok(f) => f,
            Err((status, msg)) => {
                json_err(&mut stream, status, &msg);
                return;
            }
        };
        let scalars = ams2_championship::custom_ai::Scalars {
            power: body.power_scalar,
            weight: body.weight_scalar,
            drag: body.drag_scalar,
        };
        if let Err(e) = ams2_championship::custom_ai::set_team_scalars(&file, &body.team, scalars) {
            json_err(&mut stream, "400 Bad Request", &e);
            return;
        }
        json_ok(&mut stream, &car_performance_json(&config_path, &store));
        return;
    }

    // GET /api/driver-performance — every named driver entry per class with its AI attributes.
    if method == "GET" && path == "/api/driver-performance" {
        json_ok(&mut stream, &driver_performance_json(&config_path, &store));
        return;
    }

    // PATCH /api/driver-performance — write one attribute of one driver entry back to its XML.
    // Nothing else in the table derives from it, so only the edited row comes back.
    if method == "PATCH" && path == "/api/driver-performance" {
        #[derive(serde::Deserialize)]
        struct Body {
            class: String,
            /// Position among the file's named `<driver>` blocks, from the GET payload.
            index: usize,
            /// The `<name>` that position is expected to hold — a stale-index guard.
            driver: String,
            field: String,
            value: f32,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        let file = match class_file(&config_path, &body.class) {
            Ok(f) => f,
            Err((status, msg)) => {
                json_err(&mut stream, status, &msg);
                return;
            }
        };
        if let Err(e) = ams2_championship::custom_ai::set_driver_attr(
            &file,
            body.index,
            &body.driver,
            &body.field,
            body.value,
        ) {
            json_err(&mut stream, "400 Bad Request", &e);
            return;
        }
        // Read the row back rather than echoing the request: the stored value is what the file
        // now says, after the writer's own number formatting.
        let row = ams2_championship::custom_ai::parse_driver_attributes(&file)
            .into_iter()
            .nth(body.index);
        let json = serde_json::to_vec(&row).unwrap_or_default();
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
            status: ChampionshipStatus::Progress,
            points_system: if body.points_system.is_empty() {
                vec![25, 18, 15, 12, 10, 8, 6, 4, 2, 1]
            } else {
                body.points_system
            },
            manufacturer_scoring: body.manufacturer_scoring,
            rounds: vec![],
            session_ids: vec![],
            custom_ai_file: None,
            player_team: None,
        };
        let json = serde_json::to_vec(&champ).unwrap_or_default();
        store.write().unwrap().championships.push(champ);
        persist(&store, &cur(&data_path));
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
        persist(&store, &cur(&data_path));
        let body = format!("{{\"removed\":{removed}}}");
        json_ok(&mut stream, body.as_bytes());
        return;
    }

    // Routes with path segments: /api/championships/:id[/...]
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    // GET /api/championships/:id/teams — distinct team names from the championship's
    // assigned Custom AI Drivers file, for the "player team" picker.
    if method == "GET"
        && segs.len() == 4
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "teams"
    {
        let id = segs[2];
        let data = store.read().unwrap();
        let Some(champ) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let teams = match (cfg.custom_ai_dir, &champ.custom_ai_file) {
            (Some(dir), Some(file)) => {
                ams2_championship::custom_ai::list_teams(&std::path::Path::new(&dir).join(file))
            }
            _ => vec![],
        };
        let json = serde_json::to_vec(&teams).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/championships/:id/team-eligibility — driver rating plus which teams it opens.
    if method == "GET"
        && segs.len() == 4
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "team-eligibility"
    {
        #[derive(serde::Serialize)]
        struct Body {
            /// False when the config checkbox is off — tiers are then advisory only.
            enforced: bool,
            /// Whether the picker should drop locked teams rather than showing what they ask.
            hide_locked: bool,
            /// False when the championship has no Custom AI file to rate against.
            rated: bool,
            reputation: ams2_championship::driver_rating::Reputation,
            teams: Vec<ams2_championship::driver_rating::TeamEligibility>,
        }
        let id = segs[2];
        let data = store.read().unwrap();
        let Some(champ) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let enforced = cfg.enforce_team_eligibility;
        let hide_locked = cfg.hide_locked_teams;
        let body = match champ_eligibility(&config_path, champ, &data.championships, &data.sessions)
        {
            Some((reputation, teams)) => Body {
                enforced,
                hide_locked,
                rated: true,
                reputation,
                teams,
            },
            None => Body {
                enforced,
                hide_locked,
                rated: false,
                reputation: Default::default(),
                teams: vec![],
            },
        };
        let json = serde_json::to_vec(&body).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/championships/:id/session-eligibility — which recorded sessions may join this
    // championship. `enforced` is false unless a Custom AI file *and* a player team are both
    // set; the session picker only hides anything when it is true.
    if method == "GET"
        && segs.len() == 4
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "session-eligibility"
    {
        #[derive(serde::Serialize)]
        struct Eligibility {
            enforced: bool,
            blocked: std::collections::HashMap<String, String>,
        }
        let id = segs[2];
        let data = store.read().unwrap();
        let Some(champ) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        let mut out = Eligibility {
            enforced: false,
            blocked: Default::default(),
        };
        if let (Some(dir), Some(file), Some(team)) = (
            cfg_custom_ai_dir(&config_path),
            champ.custom_ai_file.as_deref(),
            champ
                .player_team
                .as_deref()
                .filter(|t| !t.trim().is_empty()),
        ) {
            let seats = roster_seats(&dir, file);
            out.enforced = true;
            for s in &data.sessions {
                let grid: Vec<ams2_championship::custom_ai::GridEntry> = s
                    .results
                    .iter()
                    .map(|r| ams2_championship::custom_ai::GridEntry {
                        name: &r.name,
                        car_name: &r.car_name,
                        is_player: r.is_player,
                    })
                    .collect();
                if let ams2_championship::custom_ai::TeamCheck::Failed(reason) =
                    ams2_championship::custom_ai::check_player_team(&seats, &grid, team)
                {
                    out.blocked.insert(s.id.clone(), reason);
                }
            }
        }
        let json = serde_json::to_vec(&out).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // PATCH /api/championships/:id
    if method == "PATCH" && segs.len() == 3 && segs[0] == "api" && segs[1] == "championships" {
        let id = segs[2];
        #[derive(serde::Deserialize)]
        struct Body {
            name: Option<String>,
            status: Option<ChampionshipStatus>,
            points_system: Option<Vec<i32>>,
            manufacturer_scoring: Option<bool>,
            // Outer Option = key present or not (leave unchanged if absent);
            // inner Option = explicit null clears the assignment.
            #[serde(default, deserialize_with = "double_option")]
            custom_ai_file: Option<Option<String>>,
            #[serde(default, deserialize_with = "double_option")]
            player_team: Option<Option<String>>,
        }
        /// Distinguishes an absent key from an explicit `null`.
        ///
        /// Plain `Option<Option<T>>` cannot: serde collapses a `null` into the *outer* `None`,
        /// which reads as "leave unchanged" and makes the assignment impossible to clear.
        fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
        where
            D: serde::Deserializer<'de>,
            T: serde::Deserialize<'de>,
        {
            serde::Deserialize::deserialize(de).map(Some)
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        let mut data = store.write().unwrap();
        let Some(current) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };

        // The championship as it would be after this request, used by both checks below so a
        // rejection leaves it untouched.
        let mut prospective = current.clone();
        if let Some(caf) = body.custom_ai_file.clone() {
            prospective.custom_ai_file = caf;
            if prospective.custom_ai_file.is_none() {
                prospective.player_team = None;
            }
        }
        if let Some(pt) = body.player_team.clone() {
            prospective.player_team = if prospective.custom_ai_file.is_some() {
                pt
            } else {
                None
            };
        }
        // Whether the championship is under way. The first assigned session commits both the
        // roster and the seat: the results already scored were measured against that grid.
        let started = current.rounds.iter().any(|r| !r.session_ids.is_empty());

        // ── The Custom AI file is committed once the championship is under way ─
        // Every expected position, team requirement and rating figure is relative to this
        // file's grid, so swapping it mid-season would silently re-score the rounds already
        // run. Like the team lock below, this is a championship integrity rule rather than a
        // rating one, so the Config switch does not disable it.
        if prospective.custom_ai_file != current.custom_ai_file && started {
            json_err(
                &mut stream,
                "409 Conflict",
                "The Custom AI Drivers file is locked once a championship has its first \
                 session. Remove the assigned sessions first to change it.",
            );
            return;
        }

        let claimed = prospective
            .player_team
            .clone()
            .filter(|t| !t.trim().is_empty());
        // Only a *change* of team is gated; leaving an existing one alone must keep working even
        // if the rating has since dropped, or the roster has changed underneath it.
        let changed = claimed.as_deref() != current.player_team.as_deref();

        // ── The team is committed once the championship is under way ─────────
        // Swapping seats mid-season would rewrite the meaning of results already scored, so the
        // first assigned session locks it in. This is a championship integrity rule rather than
        // a rating one, so the Config switch does not disable it.
        if changed && started {
            json_err(
                &mut stream,
                "409 Conflict",
                "The team is locked once a championship has its first session. \
                 Remove the assigned sessions first to change it.",
            );
            return;
        }

        // ── Team eligibility enforcement ─────────────────────────────────────
        // Claiming a seat the driver rating has not earned is refused, unless the user has
        // switched enforcement off in Config.
        if ams2_championship::config::load_or_create(&config_path).enforce_team_eligibility {
            if let (true, Some(team)) = (changed, claimed) {
                let refused = champ_eligibility(
                    &config_path,
                    &prospective,
                    &data.championships,
                    &data.sessions,
                )
                .filter(|(_, elig)| !ams2_championship::driver_rating::is_allowed(elig, &team))
                .map(|(rep, elig)| {
                    let need = elig
                        .iter()
                        .find(|e| e.team.eq_ignore_ascii_case(team.trim()))
                        .map(|e| e.required)
                        .unwrap_or(100.0);
                    format!(
                        "{team} needs a driver rating of {need:.0}; yours is {:.0}. \
                             Race for a slower team first, or turn off team enforcement in Config.",
                        rep.value
                    )
                });
                if let Some(reason) = refused {
                    json_err(&mut stream, "409 Conflict", &reason.replace('"', "'"));
                    return;
                }
            }
        }

        // Only one championship may be Active at a time.
        if body.status == Some(ChampionshipStatus::Active) {
            for c in data.championships.iter_mut() {
                if c.id != id && c.status == ChampionshipStatus::Active {
                    c.status = ChampionshipStatus::Progress;
                }
            }
        }
        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        if let Some(name) = body.name {
            champ.name = name;
        }
        if let Some(status) = body.status {
            champ.status = status;
        }
        if let Some(ps) = body.points_system {
            champ.points_system = ps;
        }
        if let Some(ms) = body.manufacturer_scoring {
            champ.manufacturer_scoring = ms;
        }
        if let Some(caf) = body.custom_ai_file {
            champ.custom_ai_file = caf;
            // A player team is only meaningful against a Custom AI roster — it is what the
            // seat inference checks it against — so unassigning the file also clears the team.
            if champ.custom_ai_file.is_none() {
                champ.player_team = None;
            }
        }
        if let Some(pt) = body.player_team {
            champ.player_team = if champ.custom_ai_file.is_some() {
                pt
            } else {
                None
            };
        }
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &cur(&data_path));
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
        persist(&store, &cur(&data_path));
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
        champ
            .rounds
            .push(ams2_championship::data_store::Round::default());
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &cur(&data_path));
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
        persist(&store, &cur(&data_path));
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
        let (id, ridx, sid) = (
            segs[2],
            segs[4].parse::<usize>().unwrap_or(usize::MAX),
            segs[6],
        );
        let mut data = store.write().unwrap();
        let Some(champ) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        if ridx >= champ.rounds.len() {
            json_err(&mut stream, "404 Not Found", "round not found");
            return;
        }

        // ── Player-team enforcement ──────────────────────────────────────────
        // Applies only when the championship has a Custom AI file, which is also the only way
        // a player team can be set (see the PATCH route). Without that roster there is nothing
        // to infer the player's seat from, so the session is accepted unchecked.
        let rejection: Option<String> = (|| {
            let dir = cfg_custom_ai_dir(&config_path)?;
            let file = champ.custom_ai_file.as_deref()?;
            let team = champ
                .player_team
                .as_deref()
                .filter(|t| !t.trim().is_empty())?;
            let session = data.sessions.iter().find(|s| s.id == sid)?;
            let seats = roster_seats(&dir, file);
            let grid: Vec<ams2_championship::custom_ai::GridEntry> = session
                .results
                .iter()
                .map(|r| ams2_championship::custom_ai::GridEntry {
                    name: &r.name,
                    car_name: &r.car_name,
                    is_player: r.is_player,
                })
                .collect();
            match ams2_championship::custom_ai::check_player_team(&seats, &grid, team) {
                ams2_championship::custom_ai::TeamCheck::Failed(reason) => Some(reason),
                _ => None,
            }
        })();
        if let Some(reason) = rejection {
            // json_err interpolates the message straight into JSON — keep quotes out of it.
            json_err(&mut stream, "409 Conflict", &reason.replace('"', "'"));
            return;
        }

        let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        let round = &mut champ.rounds[ridx];
        if !round.session_ids.contains(&sid.to_string()) {
            round.session_ids.push(sid.to_string());
        }
        let json = serde_json::to_vec(&*champ).unwrap_or_default();
        drop(data);
        persist(&store, &cur(&data_path));
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
        let (id, ridx, sid) = (
            segs[2],
            segs[4].parse::<usize>().unwrap_or(usize::MAX),
            segs[6],
        );
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
        persist(&store, &cur(&data_path));
        json_ok(&mut stream, &json);
        return;
    }

    // POST /api/record-session — manually capture the current live session
    if method == "POST" && path == "/api/record-session" {
        match ams2_championship::session_recorder::capture_current(&store, &cur(&data_path)) {
            Ok(()) => json_ok(&mut stream, b"{\"ok\":true}"),
            Err(e) => json_err(&mut stream, "409 Conflict", &e),
        }
        return;
    }

    // ── Career save files ─────────────────────────────────────────────────────
    // A save is a *.json career file in the championships/ folder. Exactly one is active;
    // switching swaps the in-memory store in place so the recorder thread — which shares the
    // same Arc — follows along without a restart.

    // GET /api/saves — list every save with its counts, and which one is active
    if method == "GET" && path == "/api/saves" {
        let json = saves_payload(&saves_dir, &cur(&data_path));
        json_ok(&mut stream, &json);
        return;
    }

    // POST /api/saves — create a new empty career and switch to it
    // POST /api/saves/activate — switch to an existing career
    // POST /api/saves/duplicate — copy a career under a new name (stays on the current one)
    if method == "POST"
        && (path == "/api/saves" || path == "/api/saves/activate" || path == "/api/saves/duplicate")
    {
        use ams2_championship::saves::{sanitize_name, save_path};

        #[derive(serde::Deserialize)]
        struct SaveReq {
            name: String,
            #[serde(default)]
            new_name: String,
        }
        let body: SaveReq = match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => {
                json_err(&mut stream, "400 Bad Request", &e.to_string());
                return;
            }
        };
        let Some(name) = sanitize_name(&body.name) else {
            json_err(&mut stream, "400 Bad Request", "invalid save name");
            return;
        };
        let target = save_path(&saves_dir, &name);

        // Duplicate copies the file and leaves the active save alone.
        if path == "/api/saves/duplicate" {
            let Some(new_name) = sanitize_name(&body.new_name) else {
                json_err(&mut stream, "400 Bad Request", "invalid new save name");
                return;
            };
            let dest = save_path(&saves_dir, &new_name);
            if !target.exists() {
                json_err(&mut stream, "404 Not Found", "save not found");
                return;
            }
            if dest.exists() {
                json_err(
                    &mut stream,
                    "409 Conflict",
                    "a save with that name already exists",
                );
                return;
            }
            if let Err(e) = std::fs::copy(&target, &dest) {
                json_err(&mut stream, "500 Internal Server Error", &e.to_string());
                return;
            }
            let json = saves_payload(&saves_dir, &cur(&data_path));
            json_ok(&mut stream, &json);
            return;
        }

        if path == "/api/saves" {
            if target.exists() {
                json_err(
                    &mut stream,
                    "409 Conflict",
                    "a save with that name already exists",
                );
                return;
            }
            if let Err(e) = std::fs::write(
                &target,
                "{\n  \"sessions\": [],\n  \"championships\": []\n}",
            ) {
                json_err(&mut stream, "500 Internal Server Error", &e.to_string());
                return;
            }
        } else if !target.exists() {
            json_err(&mut stream, "404 Not Found", "save not found");
            return;
        }

        // Flush the outgoing career before repointing, then swap the store's *contents* —
        // never the Arc itself, which the recorder thread also holds.
        persist(&store, &cur(&data_path));
        {
            let mut active = data_path.write().unwrap();
            *store.write().unwrap() = ams2_championship::data_store::load_data(&target);
            *active = target.clone();
        }
        if let Err(e) = store_active_save(&config_path, &target) {
            json_err(&mut stream, "500 Internal Server Error", &e);
            return;
        }
        let json = saves_payload(&saves_dir, &target);
        json_ok(&mut stream, &json);
        return;
    }

    // PATCH /api/saves/:name — rename a save
    if method == "PATCH" && segs.len() == 3 && segs[0] == "api" && segs[1] == "saves" {
        use ams2_championship::saves::{sanitize_name, save_path};

        #[derive(serde::Deserialize)]
        struct RenameReq {
            new_name: String,
        }
        let body: RenameReq = match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => {
                json_err(&mut stream, "400 Bad Request", &e.to_string());
                return;
            }
        };
        let (Some(name), Some(new_name)) = (
            sanitize_name(&url_decode(segs[2])),
            sanitize_name(&body.new_name),
        ) else {
            json_err(&mut stream, "400 Bad Request", "invalid save name");
            return;
        };
        let from = save_path(&saves_dir, &name);
        let to = save_path(&saves_dir, &new_name);
        if !from.exists() {
            json_err(&mut stream, "404 Not Found", "save not found");
            return;
        }
        if to.exists() {
            json_err(
                &mut stream,
                "409 Conflict",
                "a save with that name already exists",
            );
            return;
        }
        // Flush first: an active save with unwritten changes would otherwise be renamed out
        // from under the next persist().
        let was_active = cur(&data_path) == from;
        if was_active {
            persist(&store, &from);
        }
        if let Err(e) = std::fs::rename(&from, &to) {
            json_err(&mut stream, "500 Internal Server Error", &e.to_string());
            return;
        }
        if was_active {
            *data_path.write().unwrap() = to.clone();
            if let Err(e) = store_active_save(&config_path, &to) {
                json_err(&mut stream, "500 Internal Server Error", &e);
                return;
            }
        }
        let json = saves_payload(&saves_dir, &cur(&data_path));
        json_ok(&mut stream, &json);
        return;
    }

    // DELETE /api/saves/:name — delete a save (never the active one)
    if method == "DELETE" && segs.len() == 3 && segs[0] == "api" && segs[1] == "saves" {
        use ams2_championship::saves::{sanitize_name, save_path};

        let Some(name) = sanitize_name(&url_decode(segs[2])) else {
            json_err(&mut stream, "400 Bad Request", "invalid save name");
            return;
        };
        let target = save_path(&saves_dir, &name);
        if !target.exists() {
            json_err(&mut stream, "404 Not Found", "save not found");
            return;
        }
        if cur(&data_path) == target {
            json_err(
                &mut stream,
                "400 Bad Request",
                "cannot delete the active save — switch to another one first",
            );
            return;
        }
        if let Err(e) = std::fs::remove_file(&target) {
            json_err(&mut stream, "500 Internal Server Error", &e.to_string());
            return;
        }
        let json = saves_payload(&saves_dir, &cur(&data_path));
        json_ok(&mut stream, &json);
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
        // `data_file` is deliberately absent — the active save is owned by /api/saves.
        #[derive(serde::Deserialize)]
        struct PatchConfig {
            port: u16,
            host: String,
            poll_ms: u64,
            record_practice: bool,
            record_qualify: bool,
            record_race: bool,
            show_track_map: bool,
            track_map_max_points: u32,
            #[serde(default)]
            saves_dir: Option<String>,
            #[serde(default)]
            custom_ai_dir: Option<String>,
            #[serde(default = "yes")]
            enforce_team_eligibility: bool,
            #[serde(default)]
            hide_locked_teams: bool,
            // Rating tuning. Each falls back to the default rather than to zero, so a form that
            // predates these fields — or one that fails to send them — leaves the rating alone
            // instead of silently resetting every driver to a rating of 0.
            #[serde(default = "default_start")]
            starting_rating: f32,
            #[serde(default)]
            rating_strictness: f32,
            #[serde(default)]
            eligibility_gates: ams2_championship::driver_rating::Gates,
            #[serde(default = "default_half_life")]
            rating_half_life: f32,
            #[serde(default = "yes")]
            count_retirements: bool,
            #[serde(default = "default_retire_laps")]
            retirement_min_laps_down: u32,
            #[serde(default = "default_retire_distance")]
            retirement_distance_pct: f32,
        }
        fn yes() -> bool {
            true
        }
        fn default_start() -> f32 {
            ams2_championship::driver_rating::RatingParams::default().starting_rating
        }
        fn default_half_life() -> f32 {
            ams2_championship::driver_rating::RatingParams::default().recency_half_life
        }
        fn default_retire_laps() -> u32 {
            ams2_championship::driver_rating::RatingParams::default().retirement_min_laps_down
        }
        fn default_retire_distance() -> f32 {
            ams2_championship::driver_rating::RatingParams::default().retirement_distance * 100.0
        }
        let req_body: PatchConfig = match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => {
                json_err(&mut stream, "400 Bad Request", &e.to_string());
                return;
            }
        };

        let old_cfg = ams2_championship::config::load_or_create(&config_path);

        let mut restart_required: Vec<&'static str> = vec![];
        if req_body.port != old_cfg.port {
            restart_required.push("port");
        }
        if req_body.host != old_cfg.host {
            restart_required.push("host");
        }

        // The saves folder is only read at startup — the running server keeps its current
        // folder and active save until restarted. Create the folder now so a typo surfaces
        // here rather than at the next launch.
        let blank = |s: &Option<String>| s.as_deref().map(str::trim).unwrap_or("").is_empty();
        let saves_dir_changed = !(blank(&req_body.saves_dir) && blank(&old_cfg.saves_dir))
            && req_body.saves_dir != old_cfg.saves_dir;
        if saves_dir_changed {
            if let Some(dir) = req_body
                .saves_dir
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if let Err(e) = std::fs::create_dir_all(dir) {
                    json_err(
                        &mut stream,
                        "400 Bad Request",
                        &format!("cannot use that saves folder: {e}"),
                    );
                    return;
                }
            }
            restart_required.push("saves_dir");
        }

        let new_cfg = ams2_championship::config::Config {
            port: req_body.port,
            host: req_body.host,
            saves_dir: req_body.saves_dir,
            // A remembered save in the old folder means nothing in the new one — drop it so the
            // next startup picks a save from the folder itself.
            data_file: if saves_dir_changed {
                None
            } else {
                old_cfg.data_file
            },
            poll_ms: req_body.poll_ms,
            record_practice: req_body.record_practice,
            record_qualify: req_body.record_qualify,
            record_race: req_body.record_race,
            show_track_map: req_body.show_track_map,
            track_map_max_points: req_body.track_map_max_points,
            spotter_enabled: old_cfg.spotter_enabled,
            spotter_voice: old_cfg.spotter_voice,
            spotter_name: old_cfg.spotter_name,
            custom_ai_dir: req_body.custom_ai_dir,
            enforce_team_eligibility: req_body.enforce_team_eligibility,
            hide_locked_teams: req_body.hide_locked_teams,
            starting_rating: req_body.starting_rating.clamp(0.0, 100.0),
            rating_strictness: req_body.rating_strictness.clamp(-50.0, 50.0),
            eligibility_gates: req_body.eligibility_gates,
            rating_half_life: req_body.rating_half_life.max(0.0),
            count_retirements: req_body.count_retirements,
            retirement_min_laps_down: req_body.retirement_min_laps_down.min(50),
            retirement_distance_pct: req_body.retirement_distance_pct.clamp(0.0, 100.0),
        };
        match serde_json::to_string_pretty(&new_cfg) {
            Ok(text) => {
                if let Err(e) = std::fs::write(config_path.as_ref(), text) {
                    json_err(&mut stream, "500 Internal Server Error", &e.to_string());
                    return;
                }
            }
            Err(e) => {
                json_err(&mut stream, "500 Internal Server Error", &e.to_string());
                return;
            }
        }

        #[derive(serde::Serialize)]
        struct PatchResponse<'a> {
            config: &'a ams2_championship::config::Config,
            restart_required: Vec<&'static str>,
        }
        let resp = PatchResponse {
            config: &new_cfg,
            restart_required,
        };
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

    // GET /api/spotter/voices
    if path == "/api/spotter/voices" && method == "GET" {
        let voices = ams2_championship::spotter::list_voices();
        let json = serde_json::to_vec(&voices).unwrap_or_else(|_| b"[]".to_vec());
        json_ok(&mut stream, &json);
        return;
    }

    // GET /api/spotter
    if path == "/api/spotter" && method == "GET" {
        let cfg = spotter_focus.lock().unwrap().clone();
        let player_json = match cfg.name {
            Some(n) => serde_json::Value::String(n).to_string(),
            None => "null".to_string(),
        };
        let voice_json = match cfg.voice {
            Some(v) => serde_json::Value::String(v).to_string(),
            None => "null".to_string(),
        };
        let body = format!(
            "{{\"enabled\":{},\"player\":{player_json},\"voice\":{voice_json}}}",
            cfg.enabled
        );
        json_ok(&mut stream, body.as_bytes());
        return;
    }

    // PATCH /api/spotter — set enabled, focused player, and/or voice
    if path == "/api/spotter" && method == "PATCH" {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&req.body) {
            let mut cfg = spotter_focus.lock().unwrap();
            if let Some(serde_json::Value::Bool(b)) = v.get("enabled") {
                cfg.enabled = *b;
            }
            if let Some(player) = v.get("player") {
                cfg.name = match player {
                    serde_json::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            if let Some(voice) = v.get("voice") {
                cfg.voice = match voice {
                    serde_json::Value::String(s) => Some(s.clone()),
                    _ => None,
                };
            }
            let player_json = match cfg.name.clone() {
                Some(n) => serde_json::Value::String(n).to_string(),
                None => "null".to_string(),
            };
            let voice_json = match cfg.voice.clone() {
                Some(v) => serde_json::Value::String(v).to_string(),
                None => "null".to_string(),
            };
            let body = format!(
                "{{\"enabled\":{},\"player\":{player_json},\"voice\":{voice_json}}}",
                cfg.enabled
            );
            let (s_enabled, s_voice, s_name) = (cfg.enabled, cfg.voice.clone(), cfg.name.clone());
            drop(cfg);
            // Persist to config file
            let mut file_cfg = ams2_championship::config::load_or_create(&config_path);
            file_cfg.spotter_enabled = s_enabled;
            file_cfg.spotter_voice = s_voice;
            file_cfg.spotter_name = s_name;
            if let Ok(text) = serde_json::to_string_pretty(&file_cfg) {
                let _ = std::fs::write(config_path.as_ref(), text);
            }
            json_ok(&mut stream, body.as_bytes());
        } else {
            json_err(&mut stream, "400 Bad Request", "invalid JSON");
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
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let config_path = exe_dir.join("config.json");
    let cfg = ams2_championship::config::load_or_create(&config_path);

    // The saves folder is configurable, so it has to be resolved before anything under it.
    let champ_dir = ams2_championship::saves::resolve_dir(&exe_dir, cfg.saves_dir.as_deref());
    if let Err(e) = std::fs::create_dir_all(&champ_dir) {
        eprintln!(
            "Failed to create saves directory {}: {e}",
            champ_dir.display()
        );
        std::process::exit(1);
    }
    let layouts_dir = Arc::new(champ_dir.join("track_layouts"));
    if let Err(e) = std::fs::create_dir_all(layouts_dir.as_ref()) {
        eprintln!("Failed to create track_layouts directory: {e}");
        std::process::exit(1);
    }
    println!("Saves folder:   {}", champ_dir.display());

    let career_path =
        ams2_championship::saves::resolve_active(&champ_dir, cfg.data_file.as_deref());

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
    // Shared so that switching saves at runtime repoints the recorder thread too.
    let data_path: SavePath = Arc::new(std::sync::RwLock::new(career_path));
    ams2_championship::session_recorder::start(
        store.clone(),
        data_path.clone(),
        cfg.record_practice,
        cfg.record_qualify,
        cfg.record_race,
    );
    let spotter_focus: Focus = Arc::new(std::sync::Mutex::new(
        ams2_championship::spotter::SpotterConfig {
            enabled: cfg.spotter_enabled,
            voice: cfg.spotter_voice.clone(),
            name: cfg.spotter_name.clone(),
        },
    ));
    ams2_championship::spotter::start(cfg.poll_ms, spotter_focus.clone());

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

    let config_path = Arc::new(config_path);
    let saves_dir = Arc::new(champ_dir);
    let poll_ms = cfg.poll_ms;
    for stream in listener.incoming().flatten() {
        let html = Arc::clone(&html);
        let store = store.clone();
        let data_path = Arc::clone(&data_path);
        let saves_dir = Arc::clone(&saves_dir);
        let layouts_dir = Arc::clone(&layouts_dir);
        let config_path = Arc::clone(&config_path);
        let focus = spotter_focus.clone();
        std::thread::spawn(move || {
            handle(
                stream,
                html,
                store,
                data_path,
                saves_dir,
                layouts_dir,
                config_path,
                poll_ms,
                focus,
            )
        });
    }
}
