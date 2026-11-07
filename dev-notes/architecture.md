# Architecture in full

The full text of the sections CLAUDE.md keeps in short form: the application layer, the HTTP server, the hidden-attribute rule and the test files. Read when adding a route, changing how the service answers, or looking for which test file covers what.

## The application layer (`src/service.rs`, `src/service/`)

**Every operation the app offers is a method on `service::App`, and the HTTP binary is only transport.** That split keeps every rule testable without TCP (`src/tests/service.rs`) and keeps rules out of the browser: anything a page needs computed becomes an `App` method and arrives as a value.

- `App::new(store, data_path, saves_dir, layouts_dir, config_path, spotter_focus)` — every field a shared handle or a path, so it is cheap to clone and every clone sees the same career. Built by `service::startup`. Built through `new` because it also owns private shared state (the ride-height record) that no caller should have to supply.
- Methods answer `AppResult<T>`; `AppError` is `BadRequest | NotFound | Conflict | Unsupported | Internal`, and the binary's `status_of` is the **only** place those become 400/404/409/415/500. Write the sentence once, in the service; never format an HTTP status there.
- **Request and view types live in the service and are the wire format** — `NewChampionship`, `ChampionshipPatch`, `ConfigPatch`, `SignRequest`, `CarPerformance`, `OffersView`, `LiveTeams`, … derive `Deserialize`/`Serialize` with their serde attributes, so the JSON is defined once. A view returned *owned* (e.g. `OffersView.signed` is a cloned `Contract`) so a caller is not tied to the store's lock.
- The split by subject: `championships.rs` (seasons, rounds, session assignment, `check_session_seat`, `reseat`, grid check), `signing.rs` (offers, sign/unsign, roster restore, finances), `career_saves.rs`, `performance.rs` (both performance tabs and the livery preview), `live.rs` (`resolve_live_teams`, the banners), `live_frame.rs` (what `/ws` streams each poll), `settings.rs` (config, rating adoption, spotter, track layouts), `story.rs` (a season's story — see *A season's story*), `lineage.rs` (teams carried over between seasons, and the standing the offers read). Roster/rating helpers shared between them (`champ_roster_file`, `season_contexts`, `champ_eligibility`, …) sit in `service.rs`.
- A route that answers a fixed body (`{"ok":true}`, `{"removed":N}`, `{"mode":"…"}`) keeps that literal in the binary — the service returns `()` / the count / the mode, and the envelope is transport.
- Body-parse failures are answered by the binary: most routes say `invalid body`; the saves and config routes report serde's own message (`parse_verbose`); the spotter route reads its body loosely as a `Value` (a non-string clears a setting) and builds a `SpotterPatch`.
- `src/tests/service.rs` drives `App` directly with no TCP. Route behaviour stays covered end to end in `src/tests/server.rs`.


## HTTP server notes

- All routes are matched inside a single `handle()` — first a `match (method, path)` for fixed paths, then the livery-preview prefix, then a `match (method, segs.as_slice())` for paths with segments, ending in the HTML fallback. Add new routes to the right match, and put the logic in a new `App` method rather than in the arm.
- `App::data_path` is `SavePath` (`Arc<RwLock<PathBuf>>`) — the active save, swappable at runtime
- WebSocket (`/ws`) streams `LiveSessionData` JSON at configurable poll interval
- **A request from a page on another site is refused (403) before any route**, the WebSocket included. `http::from_another_site` compares the browser-set `Origin` with `Host`, so the app works from whichever address it was opened at (`127.0.0.1`, `localhost`, a LAN address). No `Origin` at all (curl, scripts, the tests) passes; `Origin: null` (a file, a sandboxed frame) does not. Before this, any open web page could send a plain-text POST — no preflight — that signed contracts or reset rosters.
- **The one exception is `GET /live`**, the README's snapshot for outside tools, which alone still sends `Access-Control-Allow-Origin: *` (`http::json_ok_any_origin`). Every other answer carries no CORS header: the app's page is same-origin and never needed one, and `*` let any site read the career.
- Not closed: **DNS rebinding.** An attacker's hostname resolved to 127.0.0.1 makes `Origin` and `Host` agree. A `Host` allow-list would close it, but it would also break opening the app by machine name from another device.
- CI runs on `windows-latest` only (shared memory code is Windows-specific)


## An author-level `display` cancels the `hidden` attribute

**An author-level `display` cancels the `hidden` attribute.** `[hidden] { display: none }` lives in the UA stylesheet, so any rule of ours that gives the same element a `display` outranks it and the element is never hidden — it just renders empty, which for a bordered box is a stray strip and for a badge is an alarm that will not go out. A class hidden by attribute therefore needs `.<class>[hidden] { display: none; }` spelled out. This has bitten three times: the collapsed `<details>` on the lap charts, `.live-grid-warning`, and `.tab-warn`. `test_nothing_hidden_by_attribute_is_kept_visible_by_its_own_display_rule` now scans the served page for elements carrying `hidden`, checks each of their classes for a `display` rule, and fails naming any that has no opt-out — so it catches the next one rather than these. It is deliberately blind to the `…-hidden` *class* mechanism (`.tab-panel-hidden` and friends), which has no conflict to resolve.


## Test organisation

Test files live in `src/tests/` and are wired into their parent module with `#[path = "tests/filename.rs"]` — **not** in the top-level `tests/` directory. This gives tests access to `pub(crate)` items.

- `src/tests/career/data_store.rs` — `compute_career`, standings, the countback, track stats
- `src/tests/game/recorder.rs` — `capture()`, `should_capture()`, and `RecorderState::poll` driven a poll at a time (replay freeze, restarts, disconnects)
- `src/tests/config.rs` — config load/create/defaults, clamping, `normalize_economy`
- `src/tests/format.rs` — lap times, the UTC calendar maths behind local dates, credits, ordinals
- `src/tests/career/saves.rs` — both save layouts, name gating, delete and the startup husk sweep
- `src/tests/career/lap_charts.rs` — externalising a chart, reading it back, the legacy inline fallback
- `src/tests/career/signed_rosters.rs` — a signed season's roster copy: read instead of the game's file, mismatch, restore, flat saves
- `src/tests/career/driver_rating.rs` — ratings and team requirements, incl. `test_reference_career_rating_snapshot`
- `src/tests/career/contracts.rs` — offers, renewals, pay-driver seats, salary instalments, sealing
- `src/tests/career/lineage.rs` — proposing links by constructor (unique on both sides, same name always), the player's choice winning, and the shipped 1983 → 1984 rosters
- `src/tests/career/story/` — `facts.rs` (building `SeasonFacts`, the player's earlier history), `teammate.rs`, `templates.rs` (every placeholder is one a rule fills), `headlines.rs` (the walker driven with fake rules: caps, order, determinism), `headlines/{player,field,title,rival,mood}.rs` (one test per rule), `tally.rs` (the countback ordering, rounds raced), `rival.rs` (two races before a rival, the hand-over hysteresis, teammates excluded), `targets.rs` (the slack on top-N, void when the teammate sat out, bring-it-home only after a retirement, the next round), `mood.rs` (each factor moves it its way, bounded, the drift back to neutral, a car expected behind the whole grid), `review.rs` (champion, place, best and worst weekend, top headlines), `biography.rs` (season order, each first once, a title only when finished) and `title.rs` (clinching on the last available point, a tie only the name would settle, a season past its calendar, the clinch-next margin, the objective). All build facts by hand through `story::test_support` (`tests/career/story/support.rs`) — no roster file
- `src/tests/career/seed.rs` — `hash64`, pinned: salaries and headline wording hang off it
- `src/tests/roster/custom_ai.rs` — roster parsing/writing, baselines
- `src/tests/roster/liveries.rs`, `src/tests/roster/season_years.rs` — team-name resolution, and the season-year table plus the user's overrides of it
- `src/tests/service.rs` — `App` operations called directly: mode rules, season creation, not-found, the spotter patch
- `src/tests/server.rs` — integration tests for HTTP routes via real TCP loopback (`TcpListener::bind("127.0.0.1:0")`)


