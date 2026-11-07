# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository. It holds what most tasks need; the detail behind each subject is in `dev-notes/` (index at the end) — read the matching note before changing that subject.

## Commands

```bash
cargo build                        # build
cargo test                         # run all tests
cargo test test_name               # run a single test by name (substring match)
cargo test data_store              # run all tests in a module
cargo clippy                       # lint
```

No separate frontend build step — JS/CSS are embedded at compile time via `include_str!` (see below).

## Architecture

This is a single-binary Rust application (`src/bin/ams2_championship_server.rs`, a thin HTTP layer over `service::App`) that:
- Reads AMS2 telemetry from a Windows shared memory segment (`$pcars2`)
- Auto-records race/qualify/practice sessions into the active career save (`<saves dir>/<name>/career.json`, plus its lap charts in `laps/`)
- Serves a single-page HTML app over a hand-rolled TCP HTTP server (no framework, no async runtime)
- The entire UI — HTML, CSS, all JS — is compiled into the binary via `include_str!` in `web/html.rs` from `web/assets/`
- usage of javascript shall be minimized

### Key data flow

```
AMS2 shared memory
  └─ game/shared_memory.rs   (reads LiveSessionData via Windows MapViewOfFile)
  └─ game/recorder.rs        (background thread: polls every N ms, calls capture() on session end)
       └─ career/data_store.rs   (CareerData persisted to the active save's career.json)
       └─ career/lap_charts.rs   (each session's chart written beside it, never held in the store)

HTTP request
  └─ bin/ams2_championship_server.rs  (handle(): match path → parse body → call App → JSON; no rules)
       └─ service.rs + service/   (App: every operation and every cross-module rule, typed)
            └─ career/, roster/, game/, config.rs  (domain modules)
       └─ web::html::build_base_html()  (serves the compiled SPA on every non-API route)
```

### Source layout

`src/` holds four subject folders, plus the layer that strings them together and the HTTP front end. Each folder has a parent file (`src/game.rs`, …) that declares its modules — the same Rust 2018 layout as `service.rs` + `service/`, no `mod.rs`. Paths are always spelled in full (`crate::career::data_store`); there are no root re-exports, because they would hide the grouping again.

| Folder | Subject | Modules |
|---|---|---|
| `game/` | The running AMS2 process — the only Windows-specific and `unsafe` code | `shared_memory`, `recorder`, `spotter` |
| `roster/` | The AI grid AMS2 can field, read from its install | `custom_ai`, `liveries`, `livery_image`, `season_years` |
| `career/` | The save and everything judged on it | `data_store`, `lap_charts`, `saves`, `signed_rosters`, `driver_rating`, `contracts`, `lineage`, `story`, `seed` |
| `config.rs` | `config.json` — top-level because every folder reads it | |
| `format.rs` | How dates (local time), lap times, credits and track names are written — for anything rendered in Rust (the HTML export), matching `utils.js` | |
| `service/` | Every user operation as `App` methods — see below | |
| `web/` | The HTTP front end only: transport, plus the browser UI it serves (`web/assets/`, embedded by `html`) | `http`, `websocket`, `html` |
| `bin/` | The HTTP front end executable | |

`src/tests/` mirrors the folders (`tests/career/data_store.rs`, …), so a test file sits at the same path as its module. `career/signed_rosters` is in `career/`, not `roster/`, on purpose: it is the copy of a roster a signed season keeps *inside the save*.

### The application layer (`src/service.rs`, `src/service/`)

**Every operation the app offers is a method on `service::App`, and the HTTP binary is only transport.** That keeps every rule testable without TCP and keeps rules out of the browser: anything a page needs computed becomes an `App` method and arrives as a value.

- Methods answer `AppResult<T>`; `AppError` is `BadRequest | NotFound | Conflict | Unsupported | Internal`, and the binary's `status_of` is the **only** place those become 400/404/409/415/500. Write the sentence once, in the service; never format an HTTP status there.
- **Request and view types live in the service and are the wire format** (`NewChampionship`, `OffersView`, `SeasonStory`, …): the JSON is defined once. Views are returned *owned*, so a caller is not tied to the store's lock.
- By subject: `championships.rs` (seasons, rounds, session assignment, seat checks, grid check), `signing.rs` (offers, sign/unsign, roster restore, finances), `lineage.rs` (teams carried over between seasons), `career_saves.rs`, `performance.rs` (both performance tabs, livery preview), `live.rs` (live teams, banners), `live_frame.rs` (what `/ws` streams), `settings.rs` (config, rating adoption, spotter, track layouts), `story.rs` (the Paddock), `export.rs` (HTML export). Shared roster/rating helpers (`champ_roster_file`, `champ_eligibility`, …) sit in `service.rs`.
- A route that answers a fixed body (`{"ok":true}`, `{"removed":N}`) keeps that literal in the binary; the service returns `()` / the count.
- `src/tests/service.rs` drives `App` directly with no TCP; routes are covered end to end in `src/tests/server.rs`.

### HTTP server

- All routes are matched inside a single `handle()` — first a `match (method, path)` for fixed paths, then the livery-preview prefix, then a `match (method, segs.as_slice())` for paths with segments, ending in the HTML fallback. Add new routes to the right match, and put the logic in a new `App` method rather than in the arm.
- **A request from a page on another site is refused (403) before any route**, the WebSocket included (`http::from_another_site` compares `Origin` with `Host`; no `Origin` passes, `Origin: null` does not). New routes get this for free. Only `GET /live` sends `Access-Control-Allow-Origin: *`; nothing else carries a CORS header. DNS rebinding is knowingly not closed.
- CI runs on `windows-latest` only (shared memory code is Windows-specific).

### Frontend (src/web/assets/)

All JS files are concatenated into a single `<script>` block each — no bundler, no modules, plain ES5. Load order matters and is defined in `web/html.rs`:

1. `utils.js` — shared helpers (`esc`, `fmtLapTime`, `SESSION_TYPE_LABELS`, sortable tables)
2. `telemetry.js` — tyre/setup data helpers
3. `track_map.js` — canvas radar rendering
4. `live.js` — live timing tab (WebSocket to `/ws`, one `LiveFrame` per poll)
5. `career.js` — career/championships/track-stats tab, lap charts
6. `manage.js` — championship management tab
7. `contracts.js` — team lineage and seat offers (Manage), the Finances sub-tab
8. `story.js` — Career → Paddock: the season's story, the career, the Live tab's title/target lines
9. `config.js` — server config tab
10. `saves.js` — career save switcher (header dropdown + Config tab list), `applyCareerMode()`
11. `car_performance.js` — Car Performance tab (per-class scalars, baseline/reset)
12. `driver_performance.js` — Driver Performance tab (per-driver skills)
13. `main.js` — tab switching, sub-tab init, `showTab(name)`

**An author-level `display` cancels the `hidden` attribute.** Any rule of ours that gives an element a `display` outranks the UA stylesheet's `[hidden] { display: none }`, so the element never hides — it renders empty. A class hidden by attribute therefore needs `.<class>[hidden] { display: none; }` spelled out. `test_nothing_hidden_by_attribute_is_kept_visible_by_its_own_display_rule` scans the served page and fails naming any class that lacks it (it ignores the `…-hidden` *class* mechanism, which has no conflict).

### include_str! caching gotcha

`cargo` does **not** always detect changes to `include_str!` files when only the asset file changes. If CSS/JS edits aren't appearing, touch `web/html.rs` or run `cargo build` with `--` to force a rebuild.

### Test organisation

Test files live in `src/tests/` and are wired into their parent module with `#[path = "tests/filename.rs"]` — **not** in the top-level `tests/` directory. This gives tests access to `pub(crate)` items.

- `src/tests/career/` — `data_store` (standings, countback, track stats), `saves`, `lap_charts`, `signed_rosters`, `driver_rating` (incl. `test_reference_career_rating_snapshot`), `contracts` (incl. the reference-career offer tests), `lineage`, `seed` (`hash64`, pinned: salaries and headline wording hang off it)
- `src/tests/career/story/` — one file per story module; facts are built by hand through `story::test_support` (`support.rs`), no roster file
- `src/tests/roster/`, `src/tests/game/recorder.rs`, `src/tests/config.rs`, `src/tests/format.rs`, `src/tests/web/assets.rs` (every script loads)
- `src/tests/service.rs` — `App` called directly; `src/tests/server.rs` — HTTP routes over real TCP loopback (`TcpListener::bind("127.0.0.1:0")`)

## Career mode (`CareerMode` in `career/data_store.rs`)

Every save holds a `mode`, chosen when the career is created and **never changed after**. Ask `CareerMode`'s methods rather than testing the variant:

| | `Singleplayer` | `Multiplayer` | `Unset` |
|---|---|---|---|
| `uses_roster()` | yes | **no** | yes |
| `uses_contracts()` | **yes** | no | no |
| `one_season_at_a_time()` | **yes** | no | no |
| `final_is_terminal()` | **yes** | no | no |

`Unset` (saves from before modes) may be set once; SP ↔ MP is refused. SP seasons are created with a roster and a race count, `Active` at once, and the seat comes only from signing; MP has no roster, no team and no contracts, and the UI hides what MP cannot use. Full rules: `dev-notes/saves-and-persistence.md`.

## Rules most tasks need

- **No rule in JavaScript.** The server builds views and sentences; the browser prints them. A page that needs something computed gets a new `App` method.
- **Every write of a career goes through `persist` / `App::persist`.** Lock order is `SAVING` → store → active path; never take `SAVING` while holding a store guard (a non-reentrant mutex makes the mistake a hang). A handler that writes into the career's folder reads the path while holding the store. → `dev-notes/saves-and-persistence.md`
- **A file that cannot be read is never written over** — a career save or `config.json`. Reading config never writes it. A leading UTF-8 BOM is stripped wherever JSON is read.
- **One season, one roster file:** read a season's roster only through `champ_roster_file` / `signed_rosters::roster_file` (a signed season reads its own copy). Seats come from `roster_seats_with` (phantom liveries removed); count cars with `car_count`, never `len()`. A team is `DriverBlock::team`, never `extract_team_name` directly. → `dev-notes/rosters.md`
- **Practice never counts** for anything derived (rating, standings, grid warnings, the story).
- **Derived, not stored.** The rating, offers, standings and the whole story are recomputed from results on every request. Only what results cannot recover is persisted: agreed contract terms, the career's stamped tuning and balance, the player's team-lineage choices. Persisted fields get `#[serde(default)]` so old saves load.
- **Contracts never import the story;** the service hands values across (e.g. `TeamMood`). Seeded variety goes through `career::seed::hash64`, which is pinned.
- **Test against copies of real saves, never the originals.** Don't edit CRLF roster or source files with Git Bash `sed -i` (it rewrites line endings); the repo is CRLF in the working tree.

## Developer notes (`dev-notes/`)

Read the matching file before changing its subject — each holds the reasoning and edge cases behind the rules above.

| File | Read when touching |
|---|---|
| `dev-notes/saves-and-persistence.md` | startup and the saves-folder lock, save layouts and names, deleting saves (Google Drive), `persist`, switching/renaming careers, lap-chart files, `config.json`, career mode in full |
| `dev-notes/rosters.md` | Custom AI files, signed roster copies, liveries and phantom entries, livery previews, team annotations, per-track overrides, the grid check (`GridFit`), the season's car (`player_seat`), roster baselines, season years |
| `dev-notes/rating.md` | the driver rating, its calibration and tuning, team requirements, standings order and the countback |
| `dev-notes/contracts.md` | offers, signing, salaries, prizes, sealing, renewals, pay drivers, team lineage, the economy settings, the Finances UI |
| `dev-notes/story.md` | anything under `career/story/` or `service/story.rs`: the Paddock, headlines, templates, title fight, rival, targets, mood, interest, season review, career biography |
| `dev-notes/live-and-game.md` | shared memory offsets, the recorder and replays, the spotter, the telemetry panel and ride height, the live frame, the `Active` championship |
| `dev-notes/architecture.md` | the full text of the application layer, HTTP server, hidden-attribute rule and per-file test list kept short above |
| `dev-notes/history.md` | why the service layer exists (the dropped egui front end) and fixes made alongside it |
