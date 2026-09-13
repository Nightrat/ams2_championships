use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use ams2_championship::ams2_shared_memory::read_live_session;
use ams2_championship::data_store::{
    compute_career_full, persist, CareerData, CareerMode, Championship, ChampionshipStatus,
    SavePath, SharedStore,
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

/// The car class a championship runs in — its Custom AI file's stem, empty when it has none.
///
/// A team name only identifies a team *within* a class, so this is what scopes an incumbency:
/// "Ferrari" appears in seven of the eight shipped rosters, spanning thirty years.
fn champ_class(champ: &Championship) -> String {
    champ
        .custom_ai_file
        .as_deref()
        .map(ams2_championship::custom_ai::class_of_file)
        .unwrap_or_default()
        .to_string()
}

/// The career ledger and the negotiating position it leaves the driver in.
///
/// Both routes that deal in offers need both — the money to say what a seat can be bought with,
/// the standing to say who is re-signing whom — and deriving them together keeps the two from
/// ever disagreeing about which season was the last one completed.
fn career_standing(
    cfg: &ams2_championship::config::Config,
    data: &ams2_championship::data_store::CareerData,
) -> (
    ams2_championship::contracts::Finances,
    ams2_championship::contracts::Standing,
) {
    let ledger = ams2_championship::contracts::finances(
        &data.contracts,
        &data.championships,
        &data.sessions,
        None,
        // The save's own figure, not the config's — see `CareerData::starting_balance`.
        data.starting_balance.max(0),
        &cfg.prize_params(),
    );
    let standing = ams2_championship::contracts::standing(&ledger);
    (ledger, standing)
}

/// Persists the store, answering with a 500 instead of the caller's success body if the write
/// was refused.
///
/// Returns false when the caller should stop. A change that reached memory but not the file is
/// not a change the user may be told succeeded — that is precisely the silent failure the guard
/// in [`persist`] exists to surface.
fn persisted(store: &SharedStore, path: &PathBuf, stream: &mut std::net::TcpStream) -> bool {
    match persist(store, path) {
        Ok(()) => true,
        Err(e) => {
            json_err(stream, "500 Internal Server Error", &e.replace('"', "'"));
            false
        }
    }
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
    ams2_championship::config::save(config_path, &cfg)
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

    // GET /api/career/finances — every contracted season and what it paid. Derived in full on
    // each call: only the agreed terms are stored, so salaries, prize money and the balance all
    // follow the results as they stand right now.
    if method == "GET" && path == "/api/career/finances" {
        #[derive(serde::Serialize)]
        struct Body {
            /// False when the config switch is off — the figures are then advisory only.
            enabled: bool,
            /// The seat held and what was done with it, which is what next season's offers turn
            /// on. Shown here so the ledger explains the renewals rather than only listing pay.
            standing: ams2_championship::contracts::Standing,
            #[serde(flatten)]
            finances: ams2_championship::contracts::Finances,
        }
        let data = store.read().unwrap();
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let (finances, standing) = career_standing(&cfg, &data);
        let body = Body {
            enabled: data.mode.uses_contracts(),
            standing,
            finances,
        };
        let json = serde_json::to_vec(&body).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // PATCH /api/career/mode — settle a career that predates career modes.
    //
    // The *only* transition there is. A mode is picked when a career is created and kept, because
    // the two kinds allow different things: switching an established career would leave it
    // holding seasons it could not have made. `Unset` is the exception because those saves were
    // never asked, so the UI asks once and this records the answer.
    if method == "PATCH" && path == "/api/career/mode" {
        #[derive(serde::Deserialize)]
        struct Body {
            mode: CareerMode,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        if body.mode == CareerMode::Unset {
            json_err(
                &mut stream,
                "400 Bad Request",
                "choose singleplayer or multiplayer",
            );
            return;
        }
        {
            let mut data = store.write().unwrap();
            if data.mode != CareerMode::Unset {
                let msg = format!(
                    "this career is already {} and cannot be changed. \
                     Create a new career to race the other way.",
                    data.mode.label()
                );
                json_err(&mut stream, "409 Conflict", &msg);
                return;
            }
            data.mode = body.mode;
        }
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
        let body = format!("{{\"mode\":\"{}\"}}", body.mode.label());
        json_ok(&mut stream, body.as_bytes());
        return;
    }

    // GET /api/custom-ai-files — the *.xml files in the configured Custom AI Drivers folder whose
    // name AMS2 recognises as a car class. A file the game never reads cannot shape a session, so
    // it is not offered as a championship roster.
    if method == "GET" && path == "/api/custom-ai-files" {
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let files = match cfg.custom_ai_dir {
            Some(dir) => ams2_championship::custom_ai::list_files_for_known_classes(
                std::path::Path::new(&dir),
            ),
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
            /// The roster this season runs against. Required in singleplayer — a season is
            /// defined by the grid it is raced on — and refused in multiplayer.
            #[serde(default)]
            custom_ai_file: Option<String>,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };

        // ── What this career's mode allows ───────────────────────────────────
        let mode = store.read().unwrap().mode;
        let roster = body.custom_ai_file.filter(|f| !f.trim().is_empty());
        if !mode.uses_roster() && roster.is_some() {
            json_err(
                &mut stream,
                "409 Conflict",
                "a multiplayer career races people, not a Custom AI roster.",
            );
            return;
        }
        if mode == CareerMode::Singleplayer && roster.is_none() {
            json_err(
                &mut stream,
                "400 Bad Request",
                "choose a Custom AI Drivers file — a singleplayer season is defined by the \
                 grid it is raced on.",
            );
            return;
        }
        // One season at a time: the next drive is offered on the strength of the last one, so
        // the last one has to be over before there is anything to offer against.
        if mode.one_season_at_a_time() {
            let unfinished: Vec<String> = store
                .read()
                .unwrap()
                .championships
                .iter()
                .filter(|c| c.status != ChampionshipStatus::Final)
                .map(|c| c.name.clone())
                .collect();
            if !unfinished.is_empty() {
                let msg = format!(
                    "finish the current season first — {} is still running. \
                     Mark it Final on the Manage tab.",
                    unfinished.join(", ")
                );
                json_err(&mut stream, "409 Conflict", &msg.replace('"', "'"));
                return;
            }
        }

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
            custom_ai_file: roster,
            // The seat is taken by signing, never by creating — see POST .../sign.
            player_team: None,
        };
        let json = serde_json::to_vec(&champ).unwrap_or_default();
        store.write().unwrap().championships.push(champ);
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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

    // GET /api/championships/:id/offers — the seats on the table for this season, with terms.
    //
    // Nothing here is stored: an offer is only what a team would say today, so it is rebuilt
    // from the same eligibility the team picker uses. `open` is what the caller should act on —
    // a season that already has a team, or that has been raced, is no longer taking offers.
    if method == "GET"
        && segs.len() == 4
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "offers"
    {
        use ams2_championship::contracts;

        #[derive(serde::Serialize)]
        struct Body<'a> {
            /// False when the config switch is off — the offers are then advisory only.
            enabled: bool,
            /// False when the championship has no Custom AI file to rate against.
            rated: bool,
            /// Whether a seat may still be taken for this season.
            open: bool,
            reputation: f32,
            /// This season's car class. Shown so a client can explain why an incumbency did not
            /// carry: a seat is only held inside the series it was held in.
            class: String,
            /// What the career is worth, so the client can tell which buy-ins are affordable
            /// without a second request. The server checks it again on signing regardless.
            balance: i64,
            /// Teams on the grid, offering or not. `Offer::rank` is a position within this, so
            /// without it a client can say "9th fastest" but not "9th of 21" — and the teams
            /// that make no offer are exactly the ones missing from the list.
            teams: usize,
            /// The seat held going into this season, and what was done with it.
            standing: contracts::Standing,
            /// The deal already agreed for this season, if there is one.
            signed: Option<&'a contracts::Contract>,
            offers: Vec<contracts::Offer>,
        }
        let id = segs[2];
        let data = store.read().unwrap();
        let Some(champ) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let class = champ_class(champ);
        let signed = contracts::for_championship(&data.contracts, id);
        // A season stops taking offers once it is signed, or once it has been raced — the same
        // first-session rule that locks the team picker. A `player_team` picked directly is not
        // a commitment and does not close it: `POST .../sign` may still replace it, exactly as
        // `PATCH` may before the first session.
        let open = signed.is_none() && champ.rounds.iter().all(|r| r.session_ids.is_empty());

        let (ledger, standing) = career_standing(&cfg, &data);
        let rated = champ_eligibility(&config_path, champ, &data.championships, &data.sessions);
        let body = match &rated {
            Some((rep, eligibility)) => Body {
                enabled: data.mode.uses_contracts(),
                rated: true,
                open,
                reputation: rep.value,
                balance: ledger.balance,
                class: class.clone(),
                teams: eligibility.len(),
                offers: contracts::offers_for_with(
                    id,
                    &class,
                    rep.value,
                    eligibility,
                    &standing,
                    &cfg.offer_params(),
                ),
                standing,
                signed,
            },
            None => Body {
                enabled: data.mode.uses_contracts(),
                rated: false,
                open,
                reputation: 0.0,
                balance: ledger.balance,
                class,
                teams: 0,
                standing,
                signed,
                offers: vec![],
            },
        };
        let json = serde_json::to_vec(&body).unwrap_or_default();
        json_ok(&mut stream, &json);
        return;
    }

    // POST /api/championships/:id/sign — take one of the offers on the table.
    //
    // The client names only the team. Terms are regenerated server-side from the same inputs
    // that produced the offer list, so a salary cannot be dictated by the caller, and the deal
    // recorded is the one the grid would actually give today.
    //
    // This is the team picker plus a record of what was agreed: it applies the same locks
    // `PATCH /api/championships/:id` does rather than working around them.
    if method == "POST"
        && segs.len() == 4
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "sign"
    {
        use ams2_championship::contracts;

        #[derive(serde::Deserialize)]
        struct Body {
            team: String,
        }
        let Ok(body) = serde_json::from_slice::<Body>(&req.body) else {
            json_err(&mut stream, "400 Bad Request", "invalid body");
            return;
        };
        let id = segs[2].to_string();
        let mut data = store.write().unwrap();
        let Some(current) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        let current = current.clone();

        // Only a singleplayer career signs for seats. A multiplayer one races people, and an
        // unset one predates the question — recording a deal in either would put a salary in a
        // ledger the user never opted into.
        if !data.mode.uses_contracts() {
            let msg = format!(
                "contracts belong to a singleplayer career; this one is {}.",
                data.mode.label()
            );
            json_err(&mut stream, "409 Conflict", &msg);
            return;
        }
        if contracts::for_championship(&data.contracts, &id).is_some() {
            json_err(
                &mut stream,
                "409 Conflict",
                "This season is already signed. Tear up the contract first to change seat.",
            );
            return;
        }
        // The same first-session rule as the team lock: results already scored were measured
        // against the seat they were scored in.
        if current.rounds.iter().any(|r| !r.session_ids.is_empty()) {
            json_err(
                &mut stream,
                "409 Conflict",
                "The season has already started. Remove its assigned sessions first.",
            );
            return;
        }
        if current.custom_ai_file.is_none() {
            json_err(
                &mut stream,
                "409 Conflict",
                "Assign a Custom AI Drivers file first — without a roster there are no teams \
                 to sign for.",
            );
            return;
        }

        let Some((reputation, eligibility)) =
            champ_eligibility(&config_path, &current, &data.championships, &data.sessions)
        else {
            json_err(
                &mut stream,
                "409 Conflict",
                "No car performance data for this roster, so no terms can be offered.",
            );
            return;
        };
        let cfg = ams2_championship::config::load_or_create(&config_path);
        let (ledger, standing) = career_standing(&cfg, &data);
        let offers = contracts::offers_for_with(
            &id,
            &champ_class(&current),
            reputation.value,
            &eligibility,
            &standing,
            &cfg.offer_params(),
        );
        let wanted = body.team.trim();
        let Some(offer) = offers.iter().find(|o| o.team.eq_ignore_ascii_case(wanted)) else {
            // No offer means the team is out of reach, or is not on this grid at all. The
            // requirement is the useful half of the answer, so it is quoted when there is one.
            let reason = match eligibility
                .iter()
                .find(|e| e.team.eq_ignore_ascii_case(wanted))
            {
                Some(e) => format!(
                    "{wanted} is not offering a seat: they want a driver rating of {:.0} and \
                     yours is {:.0}. Race for a slower team first.",
                    e.required, reputation.value
                ),
                None => format!("{wanted} is not a team on this grid."),
            };
            json_err(&mut stream, "409 Conflict", &reason.replace('"', "'"));
            return;
        };

        // ── Paying for a seat the rating has not earned ──────────────────────
        // The price is the server's, and so is the check: the balance is derived from results,
        // so a client cannot talk its way into a car by naming its own figure.
        if offer.buy_in > 0 && ledger.balance < offer.buy_in {
            let reason = format!(
                "{} wants {} in sponsorship to take you, and the career is worth {}. \
                 Win some prize money first, or race for a team that will have you.",
                offer.team, offer.buy_in, ledger.balance
            );
            json_err(&mut stream, "409 Conflict", &reason.replace('"', "'"));
            return;
        }

        let signed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut contract = contracts::Contract::from_offer(&id, offer, signed_at);
        contract.bought_for = offer.buy_in;

        if let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) {
            champ.player_team = Some(contract.team.clone());
        }
        data.contracts.push(contract.clone());

        #[derive(serde::Serialize)]
        struct Signed<'a> {
            contract: &'a contracts::Contract,
            championship: &'a Championship,
        }
        let json = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .map(|champ| {
                serde_json::to_vec(&Signed {
                    contract: &contract,
                    championship: champ,
                })
                .unwrap_or_default()
            })
            .unwrap_or_default();
        drop(data);
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
        json_ok(&mut stream, &json);
        return;
    }

    // DELETE /api/championships/:id/sign — tear up an unraced contract.
    //
    // Without this a mis-click would be permanent: a signed season stops taking offers, so there
    // would be no way back to the list. Allowed on exactly the same terms as changing the team —
    // until the season's first session.
    if method == "DELETE"
        && segs.len() == 4
        && segs[0] == "api"
        && segs[1] == "championships"
        && segs[3] == "sign"
    {
        let id = segs[2].to_string();
        let mut data = store.write().unwrap();
        let Some(champ) = data.championships.iter().find(|c| c.id == id) else {
            json_err(&mut stream, "404 Not Found", "not found");
            return;
        };
        if champ.rounds.iter().any(|r| !r.session_ids.is_empty()) {
            json_err(
                &mut stream,
                "409 Conflict",
                "The season has already started, so its contract stands. Remove its assigned \
                 sessions first.",
            );
            return;
        }
        if !data.contracts.iter().any(|c| c.champ_id == id) {
            json_err(&mut stream, "404 Not Found", "no contract for this season");
            return;
        }
        data.contracts.retain(|c| c.champ_id != id);
        // The seat went with the deal. Clearing it puts the season back exactly where signing
        // found it, so the offer list opens again.
        if let Some(champ) = data.championships.iter_mut().find(|c| c.id == id) {
            champ.player_team = None;
        }
        drop(data);
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
        json_ok(&mut stream, b"{\"released\":true}");
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
        let mode = data.mode;

        // ── What this career's mode allows ───────────────────────────────────
        if !mode.uses_roster()
            && (prospective.custom_ai_file != current.custom_ai_file
                || prospective.player_team != current.player_team)
        {
            json_err(
                &mut stream,
                "409 Conflict",
                "a multiplayer career has no roster and no team.",
            );
            return;
        }
        // In singleplayer the seat comes from a signed contract and nowhere else, so the team
        // picker is closed. `POST .../sign` is the way in; `DELETE .../sign` is the way out.
        if mode.uses_contracts() && prospective.player_team != current.player_team {
            json_err(
                &mut stream,
                "409 Conflict",
                "sign for a team instead — a singleplayer seat comes from a contract.",
            );
            return;
        }
        // A finished singleplayer season has paid out, and the next one was created on the
        // strength of it being over. Reopening it would unpick both.
        if mode.final_is_terminal()
            && current.status == ChampionshipStatus::Final
            && body.status.as_ref().is_some_and(|s| *s != ChampionshipStatus::Final)
        {
            json_err(
                &mut stream,
                "409 Conflict",
                "a finished season stays finished.",
            );
            return;
        }

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
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
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
            /// Required when creating: a career picks its kind up front and keeps it.
            #[serde(default)]
            mode: CareerMode,
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
            // A new career must say which kind it is: every rule below turns on it, and `Unset`
            // exists only to describe saves written before the question was asked.
            if body.mode == CareerMode::Unset {
                json_err(
                    &mut stream,
                    "400 Bad Request",
                    "choose singleplayer or multiplayer for the new career",
                );
                return;
            }
            // The balance is recorded on the save now and never re-read from config, so a
            // career keeps what it was founded with however the setting moves afterwards.
            let fresh = CareerData {
                mode: body.mode,
                starting_balance: ams2_championship::config::load_or_create(&config_path)
                    .starting_balance
                    .max(0),
                ..CareerData::default()
            };
            let text = serde_json::to_string_pretty(&fresh).unwrap_or_default();
            if let Err(e) = std::fs::write(&target, text) {
                json_err(&mut stream, "500 Internal Server Error", &e.to_string());
                return;
            }
        } else if !target.exists() {
            json_err(&mut stream, "404 Not Found", "save not found");
            return;
        }

        // Read the incoming career *before* touching anything. Switching to a save that cannot
        // be parsed would put an empty career in front of the user under that save's name; the
        // file itself is protected by `persist`, but the switch is still the wrong answer.
        let incoming = match ams2_championship::data_store::try_load_data(&target) {
            Ok(data) => data,
            Err(e) => {
                let msg = format!(
                    "{} could not be read ({e}). It has not been changed, and the current \
                     career is still active.",
                    target.display()
                );
                json_err(&mut stream, "409 Conflict", &msg.replace('"', "'"));
                return;
            }
        };

        // Flush the outgoing career before repointing, then swap the store's *contents* —
        // never the Arc itself, which the recorder thread also holds.
        if !persisted(&store, &cur(&data_path), &mut stream) {
            return;
        }
        {
            let mut active = data_path.write().unwrap();
            *store.write().unwrap() = incoming;
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
        if was_active && !persisted(&store, &from, &mut stream) {
            // Refused: the file could not be read, so it must not be renamed either — moving it
            // would leave the unreadable career under a name the switcher offers as this one.
            return;
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
            // Optional so a form that omits one carries the stored value through rather than
            // resetting it behind the user — the same reason the spotter fields are not in this
            // body at all. The consequence here would be worse: an omitted salary would zero the
            // whole economy.
            // salary would reset the whole economy to zero rather than leave it alone.
            #[serde(default)]
            contract_top_salary: Option<i64>,
            #[serde(default)]
            contract_floor_salary: Option<i64>,
            #[serde(default)]
            contract_buy_in_per_point: Option<i64>,
            #[serde(default)]
            champion_prize: Option<i64>,
            #[serde(default)]
            last_place_prize: Option<i64>,
            #[serde(default)]
            starting_balance: Option<i64>,
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
            #[serde(default = "default_margin")]
            offer_margin: f32,
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
        fn default_margin() -> f32 {
            ams2_championship::driver_rating::RatingParams::default().offer_margin
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
            // Taken raw here and clamped by `normalize_economy` below, which reads the bounds off
            // `Config::offer_params` / `prize_params`. Clamping each field on its own could not
            // enforce the relationship *between* a pair: a floor above its top would be stored
            // and shown by the Config tab while the economy quietly ran on something else.
            contract_top_salary: req_body
                .contract_top_salary
                .unwrap_or(old_cfg.contract_top_salary),
            contract_floor_salary: req_body
                .contract_floor_salary
                .unwrap_or(old_cfg.contract_floor_salary),
            contract_buy_in_per_point: req_body
                .contract_buy_in_per_point
                .unwrap_or(old_cfg.contract_buy_in_per_point),
            champion_prize: req_body.champion_prize.unwrap_or(old_cfg.champion_prize),
            last_place_prize: req_body
                .last_place_prize
                .unwrap_or(old_cfg.last_place_prize),
            // Only the default for the *next* career — existing saves carry their own figure,
            // so moving this never changes a balance that has already been founded.
            starting_balance: req_body
                .starting_balance
                .unwrap_or(old_cfg.starting_balance)
                .clamp(0, 1_000_000_000),
            starting_rating: req_body.starting_rating.clamp(0.0, 100.0),
            rating_strictness: req_body.rating_strictness.clamp(-50.0, 50.0),
            eligibility_gates: req_body.eligibility_gates,
            rating_half_life: req_body.rating_half_life.max(0.0),
            count_retirements: req_body.count_retirements,
            retirement_min_laps_down: req_body.retirement_min_laps_down.min(50),
            retirement_distance_pct: req_body.retirement_distance_pct.clamp(0.0, 100.0),
            offer_margin: req_body.offer_margin.clamp(0.0, 100.0),
        };
        // Store what the economy will actually run on, so the form cannot show one thing while
        // the grid uses another.
        let mut new_cfg = new_cfg;
        new_cfg.normalize_economy();

        if let Err(e) = ams2_championship::config::save(config_path.as_ref(), &new_cfg) {
            json_err(&mut stream, "500 Internal Server Error", &e.replace('"', "'"));
            return;
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
            // Guarded and atomic: never writes defaults over a config that could not be read.
            let _ = ams2_championship::config::save(config_path.as_ref(), &file_cfg);
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
    // The one place the config file is rewritten to pick up fields added since the last run.
    // Doing that on every read is what left the file momentarily empty for other threads.
    let cfg = ams2_championship::config::load_and_upgrade(&config_path);

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
