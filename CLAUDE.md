# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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

This is a single-binary Rust application (`src/bin/ams2_championship_server.rs`) that:
- Reads AMS2 telemetry from a Windows shared memory segment (`$pcars2`)
- Auto-records race/qualify/practice sessions into the active career save (`<saves dir>/<name>/career.json`, plus its lap charts in `laps/`)
- Serves a single-page HTML app over a hand-rolled TCP HTTP server (no framework, no async runtime)
- The entire UI — HTML, CSS, all JS — is compiled into the binary via `include_str!` in `championship_html.rs`
- usage of javascript shall be minimized

### Key data flow

```
AMS2 shared memory
  └─ ams2_shared_memory.rs   (reads LiveSessionData via Windows MapViewOfFile)
  └─ session_recorder.rs     (background thread: polls every N ms, calls capture() on session end)
       └─ data_store.rs      (CareerData persisted to the active save's career.json)
       └─ lap_charts.rs      (each session's chart written beside it, never held in the store)

HTTP request
  └─ ams2_championship_server.rs  (handle() dispatches all routes manually, no router crate)
       └─ data_store::compute_career()  (derives standings/stats from raw session data)
       └─ championship_html::build_base_html()  (serves the compiled SPA on every non-API route)
```

### Frontend (src/assets/)

All JS files are concatenated into a single `<script>` block each — no bundler, no modules, plain ES5. Load order matters and is defined in `championship_html.rs`:

1. `utils.js` — shared helpers (`esc`, `fmtLapTime`, `sortChamps`, `SESSION_TYPE_LABELS`, sortable tables)
2. `telemetry.js` — tyre/setup data helpers
3. `track_map.js` — canvas radar rendering
4. `live.js` — live timing tab (WebSocket to `/ws`)
5. `career.js` — career/championships/track-stats tab, lap charts
6. `manage.js` — championship management tab
7. `contracts.js` — seat offers (Manage) and the Finances sub-tab
8. `config.js` — server config tab
9. `saves.js` — career save switcher (header dropdown + Config tab list), `applyCareerMode()`
10. `car_performance.js` — Car Performance tab (per-class scalars, baseline/reset)
11. `driver_performance.js` — Driver Performance tab (per-driver skills)
12. `main.js` — tab switching, sub-tab init, `showTab(name)`

### include_str! caching gotcha

`cargo` does **not** always detect changes to `include_str!` files when only the asset file changes. If CSS/JS edits aren't appearing, touch `championship_html.rs` or run `cargo build` with `--` to force a rebuild.

### Test organisation

Test files live in `src/tests/` and are wired into their parent module with `#[path = "tests/filename.rs"]` — **not** in the top-level `tests/` directory. This gives tests access to `pub(crate)` items.

- `src/tests/data_store.rs` — `compute_career`, standings, the countback, track stats
- `src/tests/session_recorder.rs` — `capture()`, `should_capture()`, and `RecorderState::poll` driven a poll at a time (replay freeze, restarts, disconnects)
- `src/tests/config.rs` — config load/create/defaults, clamping, `normalize_economy`
- `src/tests/saves.rs` — both save layouts, name gating, delete and the startup husk sweep
- `src/tests/lap_charts.rs` — externalising a chart, reading it back, the legacy inline fallback
- `src/tests/driver_rating.rs` — ratings and team requirements, incl. `test_reference_career_rating_snapshot`
- `src/tests/contracts.rs` — offers, renewals, pay-driver seats, salary instalments, sealing
- `src/tests/custom_ai.rs` — roster parsing/writing, baselines
- `src/tests/liveries.rs`, `src/tests/season_years.rs` — team-name resolution and season-year parsing
- `src/tests/server.rs` — integration tests for HTTP routes via real TCP loopback (`TcpListener::bind("127.0.0.1:0")`)

### Data files (saves folder — `championships/` next to the exe by default)

| Path | Purpose |
|---|---|
| `<name>/career.json` | One career save — sessions and championships. `ams2_career` is the default |
| `*.json` | **Legacy** flat save, written before folders. Still read, never migrated |
| `track_layouts/` | Per-track radar point arrays (`{slug}.json`), shared by all saves |

### Career mode: singleplayer or multiplayer (`CareerMode` in `data_store.rs`)

Every save holds a `mode`, chosen when the career is created and **never changed after** — the two kinds allow different things, so switching would leave a career holding seasons it could not have made. Ask `CareerMode`'s methods rather than testing the variant:

| | `Singleplayer` | `Multiplayer` | `Unset` |
|---|---|---|---|
| `uses_roster()` | yes | **no** | yes |
| `uses_contracts()` | **yes** | no | no |
| `one_season_at_a_time()` | **yes** | no | no |
| `final_is_terminal()` | **yes** | no | no |

- `Unset` is what every save written before this deserializes to, and it behaves exactly as the app did then. It may be set **once** via `PATCH /api/career/mode`; that is the only transition there is, and `saves.js` prompts for it when the active career is unset. SP ↔ MP is refused.
- **Mode decides whether contracts exist**, not config. The `contracts_enabled` switch is gone: the mode already answers the question and two ways to say it could disagree. `/offers`, `/finances` and `/sign` all read `data.mode.uses_contracts()`.
- SP season creation requires a `custom_ai_file` in the `POST /api/championships` body — a singleplayer season is defined by the grid it is raced on, so the roster is picked at creation rather than patched in after. MP refuses one.
- SP refuses `PATCH player_team` outright: the seat comes from `POST .../sign` and nowhere else, so the Manage tab's team picker is disabled. This is the gating that was deliberately left open when contracts were first added.
- SP refuses a new season while any existing one is not `Final`, and refuses to move a `Final` season back — it has paid out, and the next season was created on the strength of it being over.
- **SP has only two season states.** `Progress` means "started, but not the current one", and singleplayer never has a second unfinished season to tell apart — so `uses_progress_state()` is false there and `PATCH status: Progress` is refused. `new_season_status()` creates an SP season `Active`: it is the current one the moment it exists. Creating it `Progress` used to leave a fresh career with *nothing* `Active`, which is what `/api/live-teams` looks for, so the live grid showed no team names until the user found the dropdown.
- The one state decision left to the player is finishing a season, and it has to stay theirs: rounds are added as they are raced, and `planned_rounds` is a *plan* rather than a commitment — a season may stop short of it or run past it — so only the player knows when it is over. The Manage tab shows a single **Finish** button in SP rather than the three-way picker.
- MP refuses both `custom_ai_file` and `player_team`: it races people, so there is no roster and no team.
- **The UI hides what MP cannot use.** `saves.js` holds the active `careerMode` and `applyCareerMode()` hides the Car Performance and Driver Performance tabs plus the Career → Finances sub-tab, switching away first if one of them is showing. The Manage tab leaves the roster and team pickers out entirely rather than showing them disabled — so their change listeners are `if (el)`-guarded. All three describe a Custom AI roster, which an MP career does not have.
- `main.js` exposes `showTab(name)` so the hide logic can move off a tab it is about to remove.

### Multiple career saves (`src/saves.rs`)

- **A save is a folder holding `career.json`**, and the folder's name is the save's name. `config.saves_dir` picks the parent folder (resolved by `saves::resolve_dir` at startup only), `config.active_career` holds the **name** of the active career.
- `active_career` is a name, not a path, because that is how everything else addresses a save — the routes take names, `sanitize_name` gates them, `existing_save_path` resolves them. It replaced `data_file`, a full path that could contradict `saves_dir` and needed a special case in `list_saves` for an active save outside the folder. That case is gone. A config still carrying `data_file` is folded into `active_career` by `config::read_config` (in the parser, so every reader agrees whether or not the startup rewrite has run) and the old field is dropped on the next write.
- `saves::save_name_of` is the single place a save's name is derived from a path: the folder for a folder save, the file stem for a legacy flat one. Every folder save's file is `career.json`, so a name taken from the stem calls them all "career".
- **An empty saves folder means no active career, not an invented one.** `resolve_active` returns `Option` and yields `None`; `main` carries that as an empty `PathBuf`, which `cur()` and the recorder already produce on a poisoned lock. Naming a career and choosing its `CareerMode` are the user's decisions and the mode is *permanent* — an invented save arrives `Unset`, so a first-time user was greeted by a prompt to settle a question about a career they never asked for.
- With no active career: `persist` refuses ("no active career"), `capture` refuses and says so on the console rather than pushing a session into a store that creating the first career is about to replace, and `capture_current` answers with an error. `saves.js` renders "No careers yet" instead of an empty dropdown.
- The one route that must still work is `POST /api/saves` — it flushes the *outgoing* career before repointing, so that flush is skipped when there is none. Without that guard the app could never create its first career.
- The folder exists so a career can own things beside its sessions. The filename inside is fixed, so renaming a career moves one directory and everything under it travels along; a save named by its *file* stem would have to rename two things and keep them agreeing. `career_dir` is the way in.
- **Legacy flat saves (`<dir>/<name>.json`) still work and are never migrated.** `list_saves` recognises both layouts, `existing_save_path` finds either (folder wins), and `name_taken` checks both so a new folder save cannot shadow an old file. Only *new* saves are folders; renaming a flat save keeps it flat, because moving a career the user did not ask to move is how careers get lost. Duplicating one produces a folder save — a duplicate is a new save.
- A directory without a `career.json` is **not a save**, which is what keeps the shared `track_layouts/` out of the list without naming it. It is also what `delete_save` checks before what is now a *recursive* delete, along with the folder being a direct child of the saves dir.
**Deleting a save inside Google Drive (or OneDrive) cannot finish in the same process**

- The symptom: delete a career and the folder stays behind, empty. `remove_dir_all` removes `career.json` and is then denied the directory itself — `Access is denied. (os error 5)` — so the career vanishes from the switcher (no `career.json`, not a save) while its folder remains, and the route used to answer 500 on top.
- **Retrying inside the failing call never works**, which is the opposite of what the symptom suggests and rules out the two obvious fixes. Measured on a real saves folder in Google Drive: the delete retried for ~920 ms and was denied every time; a background sweep in the *same* process retried out to 63 s and was denied every time; a separate Rust process that had merely created the folder was still denied 90 s later, by `remove_dir_all` **and** by a bare `remove_dir`, on a directory already empty. A different process minutes later removes it on the first attempt, and it stays removed — Drive does not resurrect it.
- What the evidence does **not** support is a clean rule about when it lets go. A fresh process is usually able to remove a husk, but not always: one startup sweep cleared one of two husks and was denied the other, which had been made by the process it had just replaced. Treat it as intermittent, resistant to in-process retry, and reliably cleared by a later run. It only bites once Drive has taken the folder up, so a folder created and deleted within a second or two is fine — which is also why no test against a temp directory can see it.
- **The cure, rather than the mitigation, is to keep `saves_dir` off a synced drive.** Nothing here makes a sync client behave; it only stops the app reporting a failure that did not happen and tidies up when it can.
- So `delete_save` tries briefly (`remove_dir_all_briefly` — worth it for genuinely transient cross-process holders like a virus scanner), and if the folder survives with `career.json` gone it reports **success** and says so on the console. A 500 would be wrong twice over: the switcher has already stopped listing the career, and there is nothing the user could usefully retry.
- **`saves::sweep_empty_husks` finishes the job at startup**, called once from `main` — the next launch is a different process, which is exactly the condition that succeeds. It removes only **empty** directories in the saves folder, which is what makes it safe to do unasked: a save always has its `career.json`, `track_layouts` has files, and an empty directory holds nothing that can be lost.
- `test_delete_survives_something_holding_the_folder_for_a_moment` covers the transient case by opening a file inside the save **without `FILE_SHARE_DELETE`** — a plain `File::open` shares delete access and blocks nothing, so it would pass without testing anything.
- The active path is `SavePath = Arc<RwLock<PathBuf>>` (`data_store.rs`), cloned into both the HTTP handler and the recorder thread, so `POST /api/saves/activate` repoints both without a restart. Read it via `cur(&data_path)` in the server — never hold the guard across a file write.
- Switching replaces the store's **contents** (`*store.write() = load_data(&new)`), never the `Arc` — the recorder thread holds a clone of the same one.
- `sanitize_name` gates every user-supplied name (rejects separators, `..`, non-`[A-Za-z0-9 _-]`); path segments go through `http::url_decode` first.
- **A save that cannot be read is never written over.** `try_load_data` returns `Result`; `load_data` is the lenient wrapper that reports to the console and yields an empty career, and it is only safe because `persist` independently re-checks the destination before every write. That guard is *stateless* — it re-parses the target with `serde::de::IgnoredAny` (no allocation) rather than trusting a flag set at load time, so it cannot desync and it also catches a file damaged while the server is running. `persist` returns `Result`; routes go through `persisted(...)`, which answers 500 rather than reporting a save that did not happen.
- A leading UTF-8 **BOM is stripped** before parsing, in both the loader and the write guard. Notepad and PowerShell write one by default on Windows, and without this the whole career reads as corrupt — which previously meant a silent empty career and, on the next write, a destroyed save.
- `POST /api/saves/activate` reads the incoming save *before* swapping anything and refuses with 409 if it will not parse. `saves::SaveInfo.error` carries the reason for a broken save so the switcher can list it, disabled, instead of showing it as an empty career.
- `PATCH /api/config` deliberately has **no** `active_career` field — it carries the old value through, like the spotter fields. Changing `saves_dir` is restart-required and clears `active_career`, so `saves::resolve_active` re-picks from the new folder (remembered name if a career of that name is still there → `ams2_career` in either layout → first save → fresh default).

### Standings order: points, then the FIA countback (`data_store.rs`)

- `rank` is the single comparator both tables sort through, so the driver and constructor standings cannot drift apart. It is **points → countback → name**, and position is nothing more than the index of the result (`i + 1` in `careerStandingsHtml`).
- `Countback` is a driver's finishing positions as counts indexed by `position - 1`, and its **derived `Ord` *is* the FIA rule** — lexicographic comparison walks the order and stops at the first position that differs, which is exactly "most wins; if equal, most seconds; if equal, most thirds…". That only works because a trailing zero can never occur: `record` pads up to the position it is about to increment, so the last element is always ≥ 1 and an entry that never finished is the empty vec, which sorts below everyone who finished anything.
- A **retirement is not a place** and never enters the countback — the same rule that stops it scoring. A driver classified P1 who retired has no win and no first place.
- **Name is the final key, and is not an FIA rule.** The regulations hand a genuine dead heat to the stewards, which is not available here. Without *some* total order, tied entries kept whatever order the `HashMap` iterated in — and `RandomState` is seeded per map, not per process, so the order was re-rolled on **every request**. That was not a rare edge case: `points_system` is typically top-ten, so every driver outside the points was on 0/0 and the whole tail of the table reshuffled on each page load. It also reached `compute_career`, which credits `champ_wins` to `driver_standings.first()` on a `Final` season — an all-zero season crowned a random champion.
- `MAX_GRID` (64, the shared memory's participant array) caps what `record` will index. Career files are hand-edited, and the countback indexes by a parsed number.

### Lap charts live beside the career (`src/lap_charts.rs`)

- A lap chart is one row per driver per completed lap, so it is the biggest thing a session carries and the only part **nothing aggregate reads** — standings, the rating, track stats and contracts are all built from `results`. Measured on `career_reference.json` it was **44% of every byte**: 1,139,950 → 638,449 after the split, on the same 72 sessions.
- It therefore lives in `laps/<session id>.json` inside the career's folder, and **the store never holds it**. That matters because `persist` rewrites the whole career on every mutation — on a saves folder inside Google Drive, every added round re-uploads the file.
- **`/api/career` does not carry charts.** It used to clone every one of them into the response (`SessionView.lap_chart`), so opening the Career tab downloaded all of them in order to draw one. `GET /api/sessions/:id/lap-chart` serves one, and answers `[]` rather than 404 — a practice session never has a chart, and that is not an error.
- `career.js` renders each race session's chart as a closed `<details class="lap-chart-wrap">` and fetches it on first open, once (`data-loaded`). The listener is on `document` with `capture: true` because `toggle` does not bubble, and `_lapChartSessions` maps id → session so the arriving chart can still order its drivers by final position. The CSS carries the `:not([open]) > :not(summary)` rule that collapsed `<details>` needs whenever children have author-level `display`.
- **Legacy flat saves keep their charts inline.** A flat `<name>.json` has no folder to put anything beside, and those saves are never migrated — so `externalize` does nothing for them, and both the route and the reader fall back to the copy held in the session.
- `externalize` is both the one-time upgrade (run from `seal_career`, with the other load-time upgrades) and the step `capture` takes for each new session, which is why it works on a slice rather than a whole career. A chart that cannot be written **stays inline**: the career file is about to be saved anyway, so nothing is lost.
- Session ids go through `saves::sanitize_name` before being joined onto a path. They are written as unix timestamps, but a hand-edited career can hold anything.
- Charts are written once and never edited, so unlike `career.json` there is no overwrite guard here — but a leading BOM is still stripped on read, for the same reason it is everywhere else.
- Purging unassigned sessions deletes their charts too; a chart nothing points at is a stray file, and these are the big ones.

### HTTP server notes

- All routes are matched in sequence inside a single `handle()` function — add new routes before the catch-all HTML fallback at the bottom
- `data_path` is `SavePath` (`Arc<RwLock<PathBuf>>`) — the active save, swappable at runtime
- WebSocket (`/ws`) streams `LiveSessionData` JSON at configurable poll interval
- CI runs on `windows-latest` only (shared memory code is Windows-specific)

### Shared memory layout (`ams2_shared_memory.rs`)

- Always read all fields from `ptr` **before** calling `UnmapViewOfFile` — reading after unmap is an access violation.
- `ParticipantInfo` stride is 100 bytes; `mCurrentSector` (i32) at +96 is `-1` when the car is in the pit lane or garage (`in_pits` field).
- AMS2-specific fields (tyre compound, tyre temps, ride height) live at offsets above 19000 and are not in the original PCars2 header.
- **Offsets are derived by counting the header's declaration order forward from a verified anchor, then checked against a live session.** A float read at a wrong offset still returns a plausible-looking number, so neither step alone is enough. The flags/pit/car-state block (`mHighestFlagColour` 6800 → `mFuelCapacity` 6844) was previously placed after `mLastLapTimes` at 9460+ — a guess that compiles, runs, and silently reads noise: flag colour came back as 65537 and fuel as 0.0, which quietly disabled the spotter's flag and fuel calls rather than making them misfire. Counting back from `mSpeed` (6848) puts every field on a value that reads as its own unit, which is the check worth doing: oil ~98 °C, water ~68 °C, a 250 litre tank.
- **`mFuelLevel` is a fraction (0..1), not litres.** The reader multiplies it by `mFuelCapacity` so `fuel_level` matches its name and `fuel_level / fuel_capacity` means what it reads as. Without that conversion the spotter's percentage check divides a fraction by a litre count and calls fuel critical on a full tank.
- Damage lives in two runs: per-corner `mBrakeDamage` (7152) and `mSuspensionDamage` (7168) sit between `mTyreWear` and `mBrakeTempCelsius`; `mCrashState` / `mAeroDamage` / `mEngineDamage` (7280–7288) follow the five tyre-temperature arrays. `mLastOpponentCollisionIndex` (6892) and its magnitude register a tap that causes no damage at all, which none of the 0–1 values do.

### A replay freezes the recorder (`session_recorder.rs`)

- **A replay refills the live participant rows**, stepping backwards through a race that is already over. Watching one after the flag therefore rewrote the snapshot the recorder was holding, and since the usual way out of a replay is to quit the session, the *disconnect* capture filed a lap-20-of-30 picture as the result — with the standings computed off it. The lap chart went too: a falling lap count is how `accumulate_lap_chart` recognises a restart, so a replay wiped the real chart and rebuilt it from the playback.
- `is_replay(game_state)` is the whole test — `mGameState` 6 (from the pause or results screen) or 7 (loaded from the main menu). The `ams2` constants now follow the PCars2 header's declaration order, and the recorder's own observed values confirm that order: 2 while driving, 4 in the garage and on the results screen, both menus with the clock still running.
- **The freeze holds everything, `prev_session_state` included.** A session change that happens while the replay plays is acted on when the game comes back, against the same held snapshot — so watching a qualifying replay from the race lobby still writes qualifying, not the replay's rows.
- `POST /api/record-session` refuses during a replay (409) for the same reason: the frame on screen is not the session.
- The poll body lives in `RecorderState::poll`, which returns a `Taken` rather than writing — the thread owns the store and the save path, and the state machine can then be driven a poll at a time in tests, which cannot run AMS2.
- **A replay is not a disconnect and not a restart**, which is why neither existing reset point caught it: AMS2 stays connected and keeps `session_state` where it was.

### Driver rating (`src/driver_rating.rs`)

- **The grid raced must be the roster: `expected` is a rank within the roster, `field` is the size of the recorded session.** `expected_positions` ranks every seat the roster can field (phantoms already removed) and `positional_score` divides by `s.results.len()`, so the two agree only when AMS2 fielded the whole roster. Race 10 opponents on a 22-seat roster and a back-marker is still expected around P18 in an 11-car result — the score is inflated to the clamp. Nothing here can correct for it: which seats AMS2 left out is not recorded, and rescaling `expected` to the raced field would assume the absent cars were spread evenly through the order, which is exactly what it cannot know. It is therefore a **documented user requirement** (`docs/Getting-Started.md`, "Set the grid size to the roster"), not a code fix. The opposite case is self-correcting: a grid padded with stock AI trips `RosterNotDetected` once they outnumber the roster cars, and the session is skipped.
- No rating is persisted
 — the rating and every team requirement are re-derived from the assigned sessions on each request. What *is* persisted is the **tuning**, stamped onto the career (see below), so the derivation is stable rather than following whatever Config says today.

**The tuning belongs to the career, not to config**

- **`CareerData.rating_params` is copied from `config.rating_params()` when a career is created and then kept**, for the same reason `starting_balance` is. The rating decides which seats a career was ever allowed to take, so retuning it in Config would rewrite backwards whether every contract it has signed could have been signed at all. Only the *tuning* stops moving; the rating itself is still derived from results on every request.
- **`career_rating_params()` in the server is the single way to ask what a career is judged on.** It falls back to config only for a save that predates stamping and has not been loaded since. `champ_eligibility` takes the whole `CareerData` rather than its championships and sessions, because the tuning to judge them on is part of the career too and a caller passing pieces was free to forget the third one.
- `RatingParams` is therefore `Serialize`/`Deserialize` with a **container-level `#[serde(default)]`** — a hand-edited stamp that drops a field falls back to that field's shipped value rather than to zero, which for `starting_rating` would put every driver on the floor. This is the same hazard the per-field config defaults guard against, one level down.
- `seal_career` stamps a career that has none as it is **loaded** — startup and `POST /api/saves/activate` — at the config it has been judged on all along, so no rating moves on the upgrade and none moves afterwards. Same timing rule as the payout seal it sits beside.
- **`POST /api/career/rating/adopt` is the one way to change it afterwards**, and it is the deliberate exception in the shape of `custom_ai::set_baseline` against one-time `ensure_baseline`: retuning difficulty mid-career has to be possible, but not as a side effect of editing a form. It moves every rating and team bar in the career, including those past seasons were judged against, so `config.js` confirms first.
- `GET /api/config` carries `career_rating_matches` and `career_rating` alongside the config fields, and the Config tab shows a notice when they have parted. Without it the tab would quietly imply the numbers on screen are the ones in force.
- The tunable half lives in `RatingParams` (starting rating, requirement offset, which gates apply, form half-life, whether retirements count). **`RatingParams::default()` must keep reproducing the pre-config behaviour**: the public `compute_reputation*` / `team_eligibility` / `team_requirements` functions are thin wrappers that pass it, and `*_with` variants take the user's. `test_reference_career_rating_snapshot` pins real numbers against `src/tests/fixtures/career_reference.json` and is the regression net for the whole module.
- **`offer_margin` is how far below a bar a driver is still offered the seat** (default 10). It is the width of the whole middle tier: inside it a seat is `OfferPossible` and paid for normally, outside it the team is `Locked` and only a back-of-the-grid one will sell it. Zero removes the tier — every bar must be cleared outright. Distinct from `strictness`: that moves the bars, this moves how far short of them a team will look, and `test_the_margin_moves_how_far_short_is_looked_at_not_the_bar` pins the difference.
- `Config::rating_params()` clamps on the way out and `PATCH /api/config` clamps on the way in — config.json is hand-edited often enough that neither side can be trusted alone. New rating fields need a non-zero serde default (`#[serde(default = "…")]`), or a form that omits them silently resets every driver.
- A championship's Custom AI file and player team are both locked once it has its first assigned session. This is a championship integrity rule: `enforce_team_eligibility` does **not** disable it.

### A roster only exists if the liveries do (`src/liveries.rs`)

**Everything singleplayer is downstream of the Custom AI file actually being the grid AMS2 ran.** The rating scores results against the roster's pace scalars, team requirements come from the same place, and contracts are priced off those requirements — so a season raced on stock AI produces no rating movement, no offers and no money, and the live grid falls back to car models. This is one dependency, not four, and the docs say so in those terms.

- A `CustomAIDrivers` file **cannot create a car**. It binds a name, skills and scalars to a livery the game already owns, matched on `livery_name`. A name matching nothing is **silently ignored** — no error, the driver simply never spawns. The 1978 roster shipped 24 entries against 22 real liveries, so two seats looked permanently empty to the seat accounting.
- Livery data is sealed in Oodle-compressed `*_livery.bff` paks, so it cannot be read. What *can* be read is the override manifest a **livery mod** installs at `<install>/Vehicles/Textures/CustomLiveries/Overrides/<model>/<model>.xml`, whose `<LIVERY_OVERRIDE NAME="…">` entries are exactly the strings a `livery_name` must match. `overrides_dir` derives it from `custom_ai_dir` by the same two-level climb `known_class_names` uses.
- That makes the index **partial**: it covers modded models and nothing else. `phantom_liveries` therefore returns `Option`, and `None` means *cannot verify* — callers must read it as **no** phantoms rather than all of them. Two cases look identical from there and are both common: the manifests are unreadable, and not one entry matches because the class has no livery mod at all (an unmodded Formula Renault file is exactly this).
- **`roster_seats_with` is the one way to ask what seats a roster offers**, and it subtracts the phantoms. Both enforcement paths and every rating context go through it: a phantom seat can never be occupied by an AI, so leaving it in makes it look free in every session forever — which corrupts the elimination `infer_player_seat` depends on — and inflates the field size every expected finishing position is derived from. It takes the installed set rather than reading it, so a caller looping over classes scans the manifests once.
- The Car and Driver Performance tabs surface it per row (`no livery`), and `mark_phantom_entries` carries the tri-state through: `true` ignored by AMS2, `false` fine, `null` not checkable — which the tabs must not report as a clean bill of health.
- Separately, `infer_player_seat` gives up with `RosterNotDetected` when **fewer than half** the AI on a recorded grid are named in the file. That is the "this session did not use it" case, and it is a *skip*, never a failure: it means the session was run on stock AI, which is not something to accuse the user of.

### Telling the user the grid was wrong (`GridFit`)

The dependency above is invisible from inside the game, so the app says it out loud. `custom_ai::GridFit` is the whole of the reasoning and **the only place the wording lives** — the live banner, the Manage panel and the Career flag all print the same sentence, and three copies would drift.

- It reports **counts, not a verdict** (`cars`, `seats`, `ai`, `matched`), because the problems are not exclusive: a grid can be short *and* padded with stock AI. `note()` picks what to lead with — "not raced on this roster" first, since a short grid is beside the point when the grid is somebody else's.
- **`seats` counts distinct `seat` values** (team + car number), which is what `driver_rating::expected_positions` ranks. Counting liveries would over-count a season roster that lists two drivers for one car, and a warning derived from a different number than the thing it warns about would contradict it. Phantom seats are already gone — the callers pass a filtered list.
- `not_roster()` reuses **the same majority test** `infer_player_seat` gives up on. Two thresholds could disagree about the same session, and the user would get a warning that the rating did not act on, or the reverse.
- **`seats == 0` is "nothing to check against", not "all clear".** `note()` returns `None` and `is_full()` is false, so a caller cannot report a career with no rosters as perfect. Every surface says *why* instead.
- Three surfaces, one check: `GET /api/live-teams` carries `warning` + `fit` (computed only for a career whose `mode.uses_roster()`, and only when a grid is actually loaded — an empty grid is the menu); `GET /api/championships/:id/grid-check` carries a row per assigned session plus a season summary, for the Manage tab; `SessionView.grid_note` rides `/api/career` for the Career tab's per-race flag.
- **Practice is never flagged**, in either the route or the career view: nothing is derived from it, so a short practice grid is a choice rather than a mistake, and flagging it would put a warning on the one session type the warning cannot apply to.
- Nothing here blocks anything — unlike `check_player_team`, which hides contradicting sessions from the picker. A grid warning is about what a result can *mean*, not about whether it may be recorded, assigned or scored.
- `resolve_live_teams` takes `&[GridEntry]` rather than the shared-memory rows, so the resolver has no use for the other forty fields and a test can hand it a grid without fabricating sentinel values for them.

### Roster baselines (`custom_ai.rs`)

A class's **baseline** is the roster as it was before the app first wrote to it — `<class>.xml.bak`, beside the file it describes. It is the reset source, and it is what any derived "how far has this been developed" figure is measured against.

- `baseline_path` / `has_baseline` / `ensure_baseline` / `reset_from_baseline` / `set_baseline`. `write_with_backup` calls `ensure_baseline` before every write, so nothing has to remember to.
- **`ensure_baseline` is one-time by design.** A file that already has one keeps it, because that copy is the *original*; re-taking it after an edit would quietly redefine what "reset" means and the original would be gone.
- `set_baseline` is the deliberate exception, for a roster hand-tuned *after* the app first recorded one — otherwise the baseline holds the older version forever and a reset throws that tuning away. It overwrites, so the client confirms first.
- It lives in the AMS2 install rather than the app's data because it belongs to the roster. AMS2 ignores it: the game loads a `CustomAIDrivers` file only when its stem is a class in its own registry, and `F-Classic_Gen3.xml.bak` has the stem `F-Classic_Gen3.xml`.
- Routes: `POST /api/car-performance/baseline` (adopt) and `POST /api/car-performance/reset`. Both answer with the whole recomputed table — either one moves every scalar in the class, and with them the pace deltas and the ratings derived from them. `/api/car-performance` carries `has_baseline` per class so the tab knows whether there is anything to reset to.
- `car_performance.js` puts both buttons in the class heading and delegates their click handler from `#carperf-classes`, because `carPerfApplyData` replaces the controls when the first edit creates a baseline.

### Contracts and offers (`src/contracts.rs`)

- The rating decides *whether* a seat is allowed; this module decides *on what terms*. It consumes `driver_rating::TeamEligibility` and never modifies it, so the rating's snapshot test is unaffected by anything here.
- **`Contract` is the only persisted part** — `CareerData.contracts`, a `#[serde(default)]` field, so saves written before it still load. Offers are re-derived on every request exactly like the rating: an offer is only what a team would say *today*, and regenerating it after the rating moves is intended. Accepted terms are history and must never be re-derived.
- A contract joins to everything else by `champ_id` — the championship already records the season, the roster and the team, so a contract stores only what results cannot recover.
- **Every deal runs for exactly one championship.** Teams do not offer multi-year contracts, and there is no `seasons` field on `Offer` or `Contract`. A deal spanning seasons would have to survive the roster changing under it: the next championship may run an entirely different Custom AI file, where the team may not exist at all. Saves written while `seasons` existed still load — serde ignores the unknown field, which is correct because it never had any mechanical effect.
- Salary jitter is seeded by a hand-rolled FNV-1a (`hash64`) rather than `DefaultHasher`, whose algorithm may change between Rust releases. `test_hash_is_pinned` guards it: changing the hash silently re-rolls every team's terms in every existing career.
- Randomness only ever moves money, and only within `SALARY_JITTER`. Which teams offer, for how long, and what they ask for are decided by career state alone, so every offer can be explained.
- Objectives come from `TeamEligibility::expected_position` — what the car should do — not from an invented number. A target past the back of the grid is vacuous, so backmarkers offer `None`.
- **Prize money is credited only when a championship is `Final`.** It pays on a final standings position, and there is no such thing until the season is over — a provisional position is not one, because reassigning a session moves standings retroactively by design.

**Salary is paid one instalment per race (`contracts::salary_earned`)**

- `Championship.planned_rounds` is the season's **declared calendar**, set at creation. It exists because it is the one thing a career could never answer for itself: `rounds` grows as rounds are raced, so after round one there is no way to tell whether a season is an eighth or a fifteenth of the way through. That missing denominator was the *whole* reason a salary could only ever be paid in one lump at `Final`.
- So `POST /api/championships` **requires it wherever `mode.uses_contracts()`** — a singleplayer season is defined by its grid *and* its length. MP needs no calendar: no contracts, so no wage to split across one.
- **Capped, and never topped up.** Racing the full calendar draws the whole salary; stopping short draws only what was raced; racing past it earns nothing extra. The contracted figure is a **ceiling, not a promise** — the driver is paid for the races they turned up to, and the team does not pay twice for a longer season. The asymmetry is deliberate: only the player knows when a season is over, so the Finish button must not be worth money.
- A race is a round holding a session of type `data_store::SESSION_RACE` (5) — the same test standings score on, so a round that pays is a round that counted. A weekend that was only practised is not a race.
- **A season with no calendar keeps the old rule exactly**: nothing until `Final`, then the whole salary. That is every season written before this existed, and it is why the reference-career snapshot is untouched by the change.
- `PATCH /api/championships/:id` may set `planned_rounds`, and it is **not** locked by the first session the way the roster and the seat are. Under a capped, never-topped-up wage, resizing only changes the instalments still to come and re-derives what has been drawn — and it is the one way a season created before calendars existed starts paying per race. Floored at 1: it is a divisor, and career files are hand-edited.

**Sealing: `Final` closes the books**

- **Marking a season `Final` stamps what it paid onto its contract** (`Contract.settled: Option<Settlement>`), and the ledger reads the stamp instead of re-deriving. Salary and buy-in were already frozen at signing and `starting_balance` at career creation, so prize money was the *only* thing left that a Config edit could reach back into — and it reached into every finished season in the career at once. `champion_prize` and `last_place_prize` now move the seasons still to come and nothing else.
- Only the **money** is stamped. Position, field and `objective_met` stay derived, because this module records only what results cannot recover. So reassigning a session to an old championship still moves the standings that season is shown against — the money is settled, the history is not rewritten to match it.
- `settle` is **one-time**, like `custom_ai::ensure_baseline`: the first stamp was taken under the economy the season was raced under, and re-taking it later would quietly redefine what the season paid, which is the exact problem sealing removes.
- **Reopening still takes the payout back.** `unsettle` runs on the transition *out* of `Final`, which is why the server keys off the transition rather than the resulting status. Finishing again takes a fresh stamp at whatever the economy is then. (In singleplayer this never arises — `final_is_terminal()` refuses to reopen at all.)
- `seal_finished` is the **one-time upgrade** for careers finished before this existed, and *when* it runs is the whole of its correctness. It runs where a career is **loaded** — startup, and `POST /api/saves/activate`, both via `seal_career` in the server — against the economy that career has been running on, so every figure it writes is the one the ledger was already showing and the upgrade is invisible. Run it on the request path instead and it would seal those seasons at whatever the economy had since been retuned to, with no way back. Its stamps carry `at: 0`: there is no record of when those seasons were actually finished, and inventing "now" would date a 2023 season to the day the app was upgraded.
- An unsealed `Final` season still derives its prize, so a save that predates this behaves exactly as it did until the upgrade has run.
- There is no `settled` flag on `SeasonLedger`. Loading is what seals, so every `complete` season is a settled one and a second field would only ever repeat the first.
- The balance may go negative. Reassigning a session can move standings that were already paid out, so spending can end up ahead of earnings; show it as debt rather than clamping it. Persisting a ledger to avoid this would turn a derived system into a bookkeeping one.
- **A career is founded with a balance.** `CareerData.starting_balance` is copied from `config.starting_balance` when the save is created and then **kept** — never re-read from config, for the same reason a contract stores the terms that were agreed: a career that started with a million still started with a million after the setting is edited. `Finances.balance` is `starting + earned − spent`, and `starting` stays a separate field because capital is not income.
- The default (5,000,000) is **measured, not guessed**: across the eight rosters in `docs/`, the second-cheapest pay-driver seat for a driver on the starting rating costs 2,099,999–4,949,999, and the default covers the dearest of those. So a new career can buy into **at least two** sponsorship seats on every grid — a choice of way in rather than one take-it-or-leave-it. `test_a_new_career_can_buy_into_two_pay_seats_on_every_shipped_grid` re-measures it against the real rosters and fails if retuning the economy moves the costs out from under it.
- It now holds for **every** shipped class. F-Vintage_Gen2 (one pay seat) and F-Classic_Gen3 (none) used to be named exceptions precisely because their back rows were reachable on merit and so were free rather than for sale; `pay_driver_margin` removed that, and with it the exception list.
- The figure is worth about one season at the *quickest* car, which is rich for someone who has raced nothing. The knob to lower is `contract_buy_in_per_point` — 150,000 a point is what makes any real shortfall cost millions in the first place.
- Routes: `GET /api/championships/:id/offers`, `GET /api/career/finances`, `POST` and `DELETE /api/championships/:id/sign`.
- **`POST .../sign` takes only a team name.** Terms are regenerated server-side from the same eligibility that built the offer list, so a caller cannot dictate its own salary and the recorded deal is what the grid would actually give. It applies the same first-session lock `PATCH /api/championships/:id` does rather than working around it, and refuses outright (409) unless `data.mode.uses_contracts()`.
- `open` means "a seat may still be taken": no contract, and no assigned session. A `player_team` set directly through the picker is **not** a commitment and does not close a season — `POST .../sign` may replace it, exactly as `PATCH` may before the first session.
- `DELETE .../sign` tears up an unraced contract and clears the seat that came with it. Without it a mis-click would be permanent, because a signed season stops showing offers.
- `PATCH /api/championships/:id` is deliberately left permissive: with contracts on, setting a team directly still works and simply leaves that season without a ledger row. The Manage tab now offers signing alongside it, so gating it is possible — but it would strand anyone mid-career who set a team the old way.

**Renewals and pay drivers**

- `offers_for_with` takes a `Standing` — the team of the most recent *completed* contracted season, **the class it was in**, whether its target was met, and consecutive seasons served. `offers_for` is the same call on a `Standing::default()`, mirroring the `*_with` convention in `driver_rating`.
- **An incumbency never leaves its series.** `offers_for_with` takes the current championship's class and only treats a team as held when it matches `Standing::class`; `standing()` breaks tenure on a class change too. Team names are *not* unique across rosters — "Ferrari" appears in seven of the eight shipped classes, spanning thirty years — so matching on the name alone renewed a 1967 drive into a 1990 car, granted regardless of rating because delivering keeps the seat. An empty class matches nothing, so a roster-less or deleted championship holds no seat either way.
- The class is a required *parameter* rather than something the caller is trusted to pre-scope: that is the one thing this rule cannot afford to have a call site forget. `champ_class()` in the server derives it via `custom_ai::class_of_file`.
- Changing series is a fresh start, not a punishment — the rating still earns whatever it earns on the new grid. `F-Classic_Gen1` → `Gen2` counts as a change: different grid, different bar, different cars, and the user chose to switch.
- **Two offer kinds, by which way the money flows:** `Paid` (the team pays the driver) and `Pay` (the driver brings sponsorship). A `Provisional` trial used to sit between them, but it only meant something while deals could run several seasons — as the one-year prove-it deal against a multi-year one. With every deal a single season it was just a paid offer that paid less, and that gradient is better carried by leverage, which already slides to nothing at the team's bar. So clearing the bar and merely coming within reach of it now produce **identical** terms.
- `Renewal` is not a kind either: it is how an offer was come by, not what it is. It survives as `Offer::renewal`, a flag on a paid deal that carries the loyalty rise and is the only reason a team out of reach offers at all.
- **The offers table badges only the exceptions** — a bought seat and a renewal. Most rows are an ordinary paid deal and get no badge: a column reading the same word on every row is decoration, and it was what made the old four-kind table hard to read.
- Each offer carries a second row (`contract-why`) explaining itself: the car's place on pace, where it should finish, the team's bar against the driver's rating, and which rule set the money. `offerWhy()` is assembled **only** from fields already in the payload — it never recomputes a rate or a threshold, because a second copy of those in the browser would drift from the server's. It says *which* rule applied, not what the rule is.
- The "short of their bar but within reach" wording is load-bearing: a team asking 57 offering a seat to a 50 looks like a bug otherwise. That was the one thing the removed `Trial` badge did usefully, and the sentence now carries it.
- The offers payload includes `teams` (the whole grid size) so a client can say "9th-fastest of 21 teams" — `Offer::rank` is a position within the grid, and the teams that make no offer are precisely the ones missing from the list.
- Say **teams**, not cars. `rank` counts teams, so a 9th-placed team expects to finish around P18 on a grid running two cars each; calling it "car 9" makes those two numbers look contradictory.
- **Delivering keeps the seat whatever the rating says.** A met target (or a deal that set none) makes the incumbent's offer a `Renewal`, granted even when the team is `Locked` on merit. Re-earning a seat every winter would make objectives decorative. Missing a target drops the driver back to whatever they can earn on merit — not to something worse. A renewal is the *only* continuity between seasons, and it is re-derived each time rather than being a deal carried forward. A renewal is the *only* continuity between seasons; it is re-derived each time, not a deal carried forward.
- The loyalty rise applies only to a *met* target, scales to `LOYALTY_TENURE` seasons and then stops. Tenure counts an unbroken run: a driver who left and came back is a returning signing.
- **A career is never locked out, and never handed a seat.** `driver_rating` no longer forces the least demanding team open — it judges a rating against a grid and may legitimately return every seat `Locked`, which on most shipped rosters is what an unproven driver gets. The guarantee moved to `contracts::open_a_way_in`, which knows the balance: when nothing at all is obtainable, the **cheapest seat for sale drops its price to exactly what the career holds**. The seat still costs everything, which is the point — a way in, not a gift. The old rule could not do better because it decided blind to money.
- Last resort of the last resort: with `contract_buy_in_per_point: 0` nothing is for sale at all, so there is nothing to discount. The least demanding team then takes the driver free, because the alternative is a career that cannot begin.
- **Only the back of the grid sells a seat** — `sells_seat` gates `PayDriver` to the slowest `OfferParams::pay_driver_share` (default ⅓) of teams. A pay driver is a back-of-the-grid phenomenon because the money is what keeps a skint team running: Osella spent 1986 asking its drivers to bring sponsorship after its state tobacco backing left, as did AGS and Coloni; modern equivalents are Mazepin/Uralkali at Haas and Latifi at Williams. A front-runner has no budget hole to plug and loses more by fielding a slow driver than a cheque covers, so it is shut at **any** price.
- **A back-marker sells to anyone it does not actively *want*.** Clearing its bar is not enough: `OfferParams::pay_driver_margin` (default 10) is how far clear of the bar the driver must be before a selling team pays them instead of charging them. Below that the seat is a `Pay`; at or above it, an ordinary `Paid` deal — so even Osella would have paid Senna, and option 3 ("the back always charges") is deliberately *not* what this is.
- The reason it exists: at the back of the grid the grid gate is exactly zero, so a team's bar is whatever its *incumbent* happens to be. On F-Classic_Gen3, AGS's second driver is Dalmas at `race_skill` 0.49, giving a bar of 49 against a default starting rating of 50 — so an unproven rookie was handed the seat free, on any tuning. It is not fixable by lowering `starting_rating`, because the next roster's last team may run a 0.30 driver.
- The buy-in is priced on the distance to `required + pay_driver_margin`, **not** to the bar, so the price slides continuously to zero exactly where the seat flips to `Paid` rather than stepping off a cliff there. `pay_driver_margin: 0` restores the old behaviour exactly.
- A renewal is never charged: `renewing` is checked before the selling rule, so a team that just had its target met does not turn round and bill the driver.
- Consequence worth knowing: on a 14-team grid the back five all sell, and the grid-gate spacing (~100/total) is smaller than the margin — so a driver good enough to be *paid* by a back-marker has usually already opened a quicker, non-selling seat. Paid offers therefore come from the teams above the selling share, which is the intended shape.
- The price is the rating shortfall alone. An earlier version added a season's wage, which made the *quickest* car the dearest to buy into — exactly backwards, and it priced a Williams seat at 2.3× the best salary on the grid. `contract_buy_in_per_point: 0` (or a zero share) switches pay-driver seats off entirely.
- A bought seat still draws a wage — even Stroll is *paid* by Aston Martin. Without one the sponsorship could never be recovered and a single purchase would end a career.
- The salary spread is 40× (`TOP_SALARY` / `FLOOR_SALARY`), matching the real gap between a front-runner on ~$65m and a rookie on $1–2m. It was 20×, which made the back of the grid a far softer landing than it is.
- The affordability check is server-side against the derived balance; a client naming its own `buy_in` changes nothing.
- Config exposes the money plus the one threshold that decides who pays whom: `contract_top_salary`, `contract_floor_salary`, `contract_buy_in_per_point`, `contract_pay_driver_margin`, `champion_prize`, `last_place_prize` — all as `Option<...>` in `PatchConfig` so a stale form cannot zero the economy. Deal length and objective slack stay on `OfferParams::default()`. An inverted pair holds the floor *below* the top rather than swapping them — silently reordering someone's numbers is worse than ignoring one.
- **`PATCH /api/config` calls `Config::normalize_economy()` before writing**, so what is stored is what the grid runs on. Per-field clamping is not enough for a *pair*: it let a floor above its top be saved and shown by the Config tab while `offer_params()` quietly used something else. `normalize_economy` reads the clamped values back off `offer_params`/`prize_params` rather than restating the rules, so the two cannot drift.

### config.json: a read must never write

- `load_or_create` is **read-only for an existing file**. It used to rewrite the file on every call to pick up newly added fields — but it is called ~17 times across the routes, from more than one thread, and `fs::write` truncates before it writes. A reader landing in that window got `EOF while parsing a value at line 1 column 0`, fell back to `Config::default()`, and the next call persisted those defaults over the user's real settings.
- The upgrade rewrite now happens once, at startup, in `load_and_upgrade` — called only from `main`.
- `config::save` is the only writer: it refuses to overwrite a config that exists but will not parse (same rule as career saves), and writes via a sibling `.json.tmp` plus rename so a concurrent reader never sees a half-written file. `store_active_save`, `PATCH /api/config` and `PATCH /api/spotter` all go through it.
- A leading BOM is stripped here too.

**Frontend (`src/assets/contracts.js`)**

- Two views, both read fresh from the server: the offers table in the Manage tab (inside `#champ-contract-panel`, filled by `loadOffers` from `renderChampDetail`) and the finances page under **Career → Finances**. Both render nothing when `enabled` is false, so the old team picker stays the whole story for anyone not using contracts.
- The sub-tab is keyed `finances` (`#career-sub-finances`, `#career-finances-container`). It was `contracts` until the page grew past the contract table; money is the larger half of what it shows, and a contract is one row in it.
- `renderFinances` is four sections over one payload — `financeSummary` (what the career is worth), `runningSeason` (the season being raced, as a running total), `seasonLedger` (the table), `whereItWent` (wages vs prizes vs sponsorship). `runningSeason` returns `''` when nothing is being raced rather than drawing an empty card, and it only became worth showing once a wage was paid per race: before that every figure in it was zero until the season ended.
- **Nothing here recomputes a rule the server owns** — same discipline as `offerWhy()`. "Still to race for" is `salary_contracted - salary`, a subtraction of two payload fields, because racing the full calendar draws exactly the contracted figure. The per-race instalment is deliberately *not* shown as a rate: stating it would put a second copy of the wage formula in the browser, so the card says what is left and over how many races instead.
- `SeasonLedger.projected_prize` is what today's standings would pay, derived server-side for the same reason. `None` once the season is complete, because then `prize` is the fact rather than a forecast — two numbers claiming to be the payout is one too many. Rendered amber (`.ledger-projected`, `.finance-projected`) and prefixed `~`, so money not yet banked never reads like money that is.

### `ChampionshipStatus::Active` is a singleton

- **At most one championship is `Active`** — `PATCH /api/championships/:id` demotes any other holder to `Progress`. Despite the enum ordering, `Active` means "the season being raced right now", not "not yet started"; `Progress` means "started, but not the current one".
- It is therefore the answer to "which championship does this live session belong to". `resolve_live_teams` (`/api/live-teams`) reads the roster file and player team straight off it, and `loadManage()` opens the Manage tab on it.
- Do **not** reintroduce roster-matching heuristics here. Scoring Custom AI files by how many on-track drivers they name cannot separate two seasons of one series — historic packs reuse a single `.xml` across seasons, so the scores tie and the player's team resolves to the wrong season.
- Every championship mutation goes through `loadManage()`, which is why the live team-name refresh hangs off it — the status, roster file and player team all feed `/api/live-teams`.

### Spotter (`src/spotter.rs`)

- `SpotterState::update()` returns a `Vec<String>` of TTS phrases each poll; the background thread writes them line-by-line to a persistent PowerShell `SpeechSynthesizer` subprocess.
- Position announcements are **debounced** (~2 s): `pending_position` tracks the real-time position; `prev_position` is only updated (and the announcement emitted) once `pos_cooldown` reaches zero. This prevents a queue of stale "Position N" calls after a spin.
- Gap, flag, fuel, and tyre warn events use simple prev-state comparison — no debounce needed there.
- `SpotterConfig` (enabled, voice, name) is shared via `Arc<Mutex<SpotterConfig>>`; PATCH `/api/spotter` updates it and persists it back to `config.json`.

### Telemetry tab freeze behaviour

The tab shows the player car's **damage** (crash state, aero, engine, per-corner brake/suspension, last car-to-car contact) followed by **every tyre field the shared memory carries**, as a card per wheel. Values are rendered raw: no rolling average and no rolling buffer, so a wrong offset shows up as a wrong number instead of being smoothed into something plausible.

- `mTyreSlipSpeed`, `mTyreGrip` and `mTyreLateralStiffness` are marked OBSOLETE in the header ("kept for backward compatibility only"). They are read and shown dimmed at the foot of the panel rather than dropped, so whether AMS2 still fills them is answered on screen instead of being re-investigated each time someone wants slip or grip data.
- The five structure temperatures (`mTyreTreadTemp`, `mTyreLayerTemp`, `mTyreCarcassTemp`, `mTyreRimTemp`, `mTyreInternalAirTemp`) are **Kelvin** in the header while every other temperature in the struct is Celsius. `kelvin4` converts them on read and holds unset (0.0) at zero — a plain subtraction would report an unset wheel as −273 °C, which reads as a bad offset rather than as no data.
- It freezes on the last on-track reading (`dmgLastOnTrack`) whenever the car is not out on track, and resumes when it is. **Two signals are needed**, and neither is sufficient alone: `in_pits` (`mCurrentSector < 0`) flips at the pit entry, but ESC → "Return to pits" teleports the car into the garage without it ever driving down a pit lane — only `mPitMode` catches that. On track is `!in_pits && pit_mode == 0`.
- The held panel is not repainted each poll. `dmgFrozenLabel` stores the reason currently painted rather than a boolean, so the label still updates as the car moves through the pit modes, while an unchanged panel keeps any text selection the user made in it.
- With no on-track reading yet it says so, rather than painting zeroes that look like an undamaged car.

**Ride height is also kept as a record of the run**

- The instantaneous reading says what the car is doing now; the **lowest** a corner ever reached is what says how much floor was left, which is the number a setup is chosen against. `rideSeen` accumulates a per-corner min/max across the run, shown on the damage panel as "Lowest this run / highest" and per wheel as "Seen this run".
- **Per corner, not one overall minimum.** Front and rear ride height are separate setup values, so an overall figure alone would not say which end to lower. `rideExtremes` skips corners at zero — zero is unset, not a car resting on its floor, and it would otherwise always win the minimum.
- `recordRideHeight` is called from the `onTrack` branch **before** the panel's visibility check, on purpose: a lap driven with the Live tab closed is still a lap. It is keyed on track + variation (`rideSeenKey`), so a new track starts a new record rather than answering a question about Monza with a lap of Bathurst; a WebSocket disconnect clears it too, since the next run is a different car on a different setup.
- The **Reset** button is delegated from `document` because the panel is rebuilt on every on-track poll, and it clears `dmgFrozenLabel` to force a repaint — the button is most useful in the box, which is exactly when the panel is held and would not otherwise redraw.
