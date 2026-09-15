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
- Auto-records race/qualify/practice sessions to a JSON file (`ams2_career.json`)
- Serves a single-page HTML app over a hand-rolled TCP HTTP server (no framework, no async runtime)
- The entire UI — HTML, CSS, all JS — is compiled into the binary via `include_str!` in `championship_html.rs`
- usage of javascript shall be minimized

### Key data flow

```
AMS2 shared memory
  └─ ams2_shared_memory.rs   (reads LiveSessionData via Windows MapViewOfFile)
  └─ session_recorder.rs     (background thread: polls every N ms, calls capture() on session end)
       └─ data_store.rs      (CareerData persisted to ams2_career.json)

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
5. `career.js` — career/championships/track-stats tab
6. `manage.js` — championship management tab
7. `config.js` — server config tab
8. `saves.js` — career save switcher (header dropdown + Config tab list)
9. `main.js` — tab switching, sub-tab init

### include_str! caching gotcha

`cargo` does **not** always detect changes to `include_str!` files when only the asset file changes. If CSS/JS edits aren't appearing, touch `championship_html.rs` or run `cargo build` with `--` to force a rebuild.

### Test organisation

Test files live in `src/tests/` and are wired into their parent module with `#[path = "tests/filename.rs"]` — **not** in the top-level `tests/` directory. This gives tests access to `pub(crate)` items.

- `src/tests/data_store.rs` — unit tests for `compute_career`, standings, track stats
- `src/tests/session_recorder.rs` — unit tests for `capture()` and `should_capture()`
- `src/tests/config.rs` — unit tests for config load/create/defaults
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
- The one state decision left to the player is finishing a season, and it has to stay theirs: rounds are added as they are raced and a career never declares how long a season should be, so only the player knows when it is over. The Manage tab shows a single **Finish** button in SP rather than the three-way picker.
- MP refuses both `custom_ai_file` and `player_team`: it races people, so there is no roster and no team.
- **The UI hides what MP cannot use.** `saves.js` holds the active `careerMode` and `applyCareerMode()` hides the Car Performance and Driver Performance tabs plus the Career → Contracts sub-tab, switching away first if one of them is showing. The Manage tab leaves the roster and team pickers out entirely rather than showing them disabled — so their change listeners are `if (el)`-guarded. All three describe a Custom AI roster, which an MP career does not have.
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
- The active path is `SavePath = Arc<RwLock<PathBuf>>` (`data_store.rs`), cloned into both the HTTP handler and the recorder thread, so `POST /api/saves/activate` repoints both without a restart. Read it via `cur(&data_path)` in the server — never hold the guard across a file write.
- Switching replaces the store's **contents** (`*store.write() = load_data(&new)`), never the `Arc` — the recorder thread holds a clone of the same one.
- `sanitize_name` gates every user-supplied name (rejects separators, `..`, non-`[A-Za-z0-9 _-]`); path segments go through `http::url_decode` first.
- **A save that cannot be read is never written over.** `try_load_data` returns `Result`; `load_data` is the lenient wrapper that reports to the console and yields an empty career, and it is only safe because `persist` independently re-checks the destination before every write. That guard is *stateless* — it re-parses the target with `serde::de::IgnoredAny` (no allocation) rather than trusting a flag set at load time, so it cannot desync and it also catches a file damaged while the server is running. `persist` returns `Result`; routes go through `persisted(...)`, which answers 500 rather than reporting a save that did not happen.
- A leading UTF-8 **BOM is stripped** before parsing, in both the loader and the write guard. Notepad and PowerShell write one by default on Windows, and without this the whole career reads as corrupt — which previously meant a silent empty career and, on the next write, a destroyed save.
- `POST /api/saves/activate` reads the incoming save *before* swapping anything and refuses with 409 if it will not parse. `saves::SaveInfo.error` carries the reason for a broken save so the switcher can list it, disabled, instead of showing it as an empty career.
- `PATCH /api/config` deliberately has **no** `active_career` field — it carries the old value through, like the spotter fields. Changing `saves_dir` is restart-required and clears `active_career`, so `saves::resolve_active` re-picks from the new folder (remembered name if a career of that name is still there → `ams2_career` in either layout → first save → fresh default).

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

### Driver rating (`src/driver_rating.rs`)

- Nothing is persisted — the rating and every team requirement are re-derived from the assigned sessions on each request, so config changes take effect immediately and can always be undone.
- The tunable half lives in `RatingParams` (starting rating, requirement offset, which gates apply, form half-life, whether retirements count). **`RatingParams::default()` must keep reproducing the pre-config behaviour**: the public `compute_reputation*` / `team_eligibility` / `team_requirements` functions are thin wrappers that pass it, and `*_with` variants take the user's. `test_reference_career_rating_snapshot` pins real numbers against `src/tests/fixtures/career_reference.json` and is the regression net for the whole module.
- **`offer_margin` is how far below a bar a driver is still offered the seat** (default 10). It is the width of the whole middle tier: inside it a seat is `OfferPossible` and paid for normally, outside it the team is `Locked` and only a back-of-the-grid one will sell it. Zero removes the tier — every bar must be cleared outright. Distinct from `strictness`: that moves the bars, this moves how far short of them a team will look, and `test_the_margin_moves_how_far_short_is_looked_at_not_the_bar` pins the difference.
- `Config::rating_params()` clamps on the way out and `PATCH /api/config` clamps on the way in — config.json is hand-edited often enough that neither side can be trusted alone. New rating fields need a non-zero serde default (`#[serde(default = "…")]`), or a form that omits them silently resets every driver.
- A championship's Custom AI file and player team are both locked once it has its first assigned session. This is a championship integrity rule: `enforce_team_eligibility` does **not** disable it.

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
- **Salary and prize money are credited only when a championship is `Final`.** Rounds are added as they are raced, so a career never declares how long a season is meant to be and there is nothing to pro-rate a part-season against. Reopening a season takes its payout back — that is the derivation working, not a bug.
- The balance may go negative. Reassigning a session can move standings that were already paid out, so spending can end up ahead of earnings; show it as debt rather than clamping it. Persisting a ledger to avoid this would turn a derived system into a bookkeeping one.
- **A career is founded with a balance.** `CareerData.starting_balance` is copied from `config.starting_balance` when the save is created and then **kept** — never re-read from config, for the same reason a contract stores the terms that were agreed: a career that started with a million still started with a million after the setting is edited. `Finances.balance` is `starting + earned − spent`, and `starting` stays a separate field because capital is not income.
- The default (5,000,000) is **measured, not guessed**: across the eight rosters in `docs/`, the second-cheapest pay-driver seat for a driver on the starting rating costs 2,099,999–4,949,999, and the default covers the dearest of those. So a new career can buy into **at least two** sponsorship seats on every grid — a choice of way in rather than one take-it-or-leave-it. `test_a_new_career_can_buy_into_two_pay_seats_on_every_shipped_grid` re-measures it against the real rosters and fails if retuning the economy moves the costs out from under it.
- It now holds for **every** shipped class. F-Vintage_Gen2 (one pay seat) and F-Classic_Gen3 (none) used to be named exceptions precisely because their back rows were reachable on merit and so were free rather than for sale; `pay_driver_margin` removed that, and with it the exception list.
- The figure is worth about one season at the *quickest* car, which is rich for someone who has raced nothing. The knob to lower is `contract_buy_in_per_point` — 150,000 a point is what makes any real shortfall cost millions in the first place.
- `config.contracts_enabled` defaults **off** and is carried through `PATCH /api/config` as an `Option<bool>` (like the spotter fields), so a stale form that omits it cannot switch the feature off behind the user.
- Routes: `GET /api/championships/:id/offers`, `GET /api/career/finances`, `POST` and `DELETE /api/championships/:id/sign`.
- **`POST .../sign` takes only a team name.** Terms are regenerated server-side from the same eligibility that built the offer list, so a caller cannot dictate its own salary and the recorded deal is what the grid would actually give. It applies the same first-session lock `PATCH /api/championships/:id` does rather than working around it, and refuses outright while `contracts_enabled` is off.
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

- Two views, both read fresh from the server: the offers table in the Manage tab (inside `#champ-contract-panel`, filled by `loadOffers` from `renderChampDetail`) and the ledger under Career → Contracts. Both render nothing when `enabled` is false, so the old team picker stays the whole story for anyone not using contracts.

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
