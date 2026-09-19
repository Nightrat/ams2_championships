# AMS2 Championships

[![Release](https://img.shields.io/github/v/release/Nightrat/ams2_championships)](https://github.com/Nightrat/ams2_championships/releases/latest)
[![Tests](https://github.com/Nightrat/ams2_championships/actions/workflows/rust.yml/badge.svg)](https://github.com/Nightrat/ams2_championships/actions/workflows/rust.yml)

> **Download the latest release:** [ams2_championship_server.exe](https://github.com/Nightrat/ams2_championships/releases/latest/download/ams2_championship_server.exe)

A motorsport career tracker for Automobilista 2. It records race results directly from the AMS2 shared memory API, lets you organise them into championships, and displays everything in a browser-based UI with a real-time live timing overlay. A singleplayer career goes further: it rates your driving against the grid, and teams offer you seats on terms you have earned.

> **Note:** The majority of the code in this repository was written with the assistance of [Claude](https://claude.ai) (Anthropic AI).

## Documentation

- [Getting Started](docs/Getting-Started.md)
- [Live Session](docs/Live-Session.md)
- [Career Tab](docs/Career.md)
- [Managing Championships](docs/Managing-Championships.md)
- [Contracts & Money](docs/Contracts-and-Money.md)
- [Driver Rating & Performance](docs/Rating-and-Performance.md)
- [Data & Backup](docs/Data-and-Backup.md)

## Features

- **Session recorder** — automatically captures race, qualifying and practice results at session end from the AMS2 shared memory API; no external tool required. Watching a replay freezes the recorder rather than filing the replay as the result
- **Championship management** — create championships, assign recorded sessions to rounds, set points systems (F1 modern/classic or custom), toggle constructor scoring, declare the season calendar, and track status (Active / Progress / Final — exactly one championship is Active, marking the season you are currently racing)
- **Championship standings** — master-detail view with per-championship driver and constructor standings, collapsible round-by-round results, and a per-race lap chart loaded on demand. Ties are broken by the FIA countback: most wins, then most seconds, then most thirds
- **Career statistics** — aggregated stats across all championships: race starts, podium splits (1st/2nd/3rd), top-10 finishes, average finishing position, DNFs, qualifying results (pole/2nd/3rd/top-10), and championship standings finishes (1st/2nd/3rd)
- **Track statistics** — per-track summary across all recorded sessions: race and qualifying counts, best lap time with record holder name and car, last visited date
- **Multiple careers** — every career is a separate save folder with its own championships, sessions and stats; switch between them from the header without restarting. Each career is either **singleplayer** or **multiplayer**, chosen when it is created
- **Driver rating** — a 0–100 rating derived from your recorded results, measured against what the car should have done so a slow car is no handicap. Re-derived on every request and never stored. Each team on the grid has a requirement built from car pace and the skill of the driver already in the seat — all of it read from the season's Custom AI Drivers roster, so a session not raced on that roster is skipped rather than guessed at
- **Contracts and money** (singleplayer) — teams offer seats on terms your rating has earned: a salary scaled by car pace, a season objective, renewals for delivering, and back-of-the-grid seats that ask for sponsorship instead. Salary is paid one instalment per race; prize money is paid when a season is marked Final
- **Grid checks** — warns when the grid raced is not the one a season is judged against: a roster that was not used, a short grid, or one padded with stock AI. Shown live while it can still be fixed, on the season in Manage, and flagged on the affected races in Career. It never blocks anything
- **Car & Driver Performance tabs** — edit the performance scalars and driver skills in your AMS2 Custom AI Drivers rosters in place, with a one-time backup per class to reset to
- **Live session overlay** — real-time timing table pushed over WebSocket from AMS2 shared memory: position, laps, race interval, gap to fastest lap, sector times, best/last lap, car/team, and tyre compound for the player
- **Historic team names** — AMS2 exposes no livery field, so the live grid resolves each driver to their real team from the **active** championship Custom AI Drivers file, and your own row from that championship **My Team** setting; anything unmatched falls back to the AMS2 car model
- **Track radar** — canvas overlay on the live timing view that builds a map of the track from car positions and renders all participants as dots; the map is saved to disk per track and loaded on the next visit
- **Telemetry panel** — player damage (crash state, aero, engine, per-corner brake and suspension, last contact) and every tyre field the shared memory carries, per wheel; ride height is also kept as a min/max record of the run. Freezes on the last on-track reading while you are in the pits
- **Voice spotter** — server-side Windows TTS calls position, gaps, flags, fuel and tyre wear
- **HTML export** — download a standalone HTML copy of the career: every championship expanded, plus the driver and track statistics tables
- **Configuration** — browser-based Config tab writes a `config.json` next to the executable; all settings have documented defaults and the file is created automatically on first run

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable, 2021 edition)
- Windows (the session recorder and live overlay read the `$pcars2$` named shared memory, which is Windows-only)

**For a singleplayer career** you also need, for each class you race: the **custom liveries (skin mod) installed in AMS2**, a **Custom AI Drivers file that names those liveries**, and `custom_ai_dir` pointed at your `UserData/CustomAIDrivers` folder — then the season raced with that roster active **and with the opponent count set so the grid fills the roster** — one installed livery is one car, and the rating compares your finish against where your car ranks across the whole roster, so a short grid measures you on the wrong scale while an over-long one pads the grid with stock AI. Historic team names, the driver rating, team requirements, contracts and the whole Finances page are all derived from the recorded grid matching that file, so a season raced on stock AI records its results and scores its points but tells the career nothing: no rating movement, no offers, no money. A Custom AI entry whose `livery_name` AMS2 does not own is silently ignored by the game, and the Car and Driver Performance tabs mark those rows **no livery**. A multiplayer career needs none of this and cannot use it. See [What a singleplayer career needs](docs/Getting-Started.md#what-a-singleplayer-career-needs).

## Build

```bash
cargo build --release
```

## Usage

```bash
cargo run --release --bin ams2_championship_server
```

On first run the server creates a `championships/` folder and a `config.json` file next to the executable, then:

1. Opens a career from the saves folder — none is invented, so create the first one from the Config tab
2. Starts a background session recorder that saves results automatically when a session ends in AMS2
3. Serves the UI at `http://127.0.0.1:8080/` (host and port are configurable)

Open the URL in a browser. Press **Ctrl+C** to stop.

## Configuration

On first run `config.json` is created next to the executable with all defaults. Edit it directly or use the **Config** tab in the UI.

| Key | Default | Restart required | Description |
|---|---|---|---|
| `port` | `8080` | Yes | HTTP and WebSocket port |
| `host` | `"127.0.0.1"` | Yes | Bind address — use `"0.0.0.0"` to allow LAN access |
| `saves_dir` | `null` | Yes | Folder holding the career saves and their track layouts; `null` uses `championships/` next to the executable |
| `active_career` | `null` | No | **Name** of the active career, not a path. Set by the career switcher, not by hand |
| `poll_ms` | `200` | No | Shared memory read interval in milliseconds (live view refresh rate) |
| `record_practice` | `true` | Yes | Automatically save practice sessions |
| `record_qualify` | `true` | Yes | Automatically save qualifying sessions |
| `record_race` | `true` | Yes | Automatically save race sessions |
| `show_track_map` | `false` | No | Show the track radar canvas in the live timing view |
| `track_map_max_points` | `5000` | No | Maximum unique grid cells accumulated for the track radar before collection stops |
| `custom_ai_dir` | `null` | No | AMS2 `UserData/CustomAIDrivers` folder — the rosters championships are raced on |
| `spotter_enabled` | `false` | No | Enable the server-side voice spotter |
| `spotter_voice` | `null` | No | TTS voice name (`null` = system default) |
| `spotter_name` | `null` | No | Driver name to track in multiplayer (`null` = viewed player) |
| `enforce_team_eligibility` | `true` | No | Only allow My Team to be set to a team the driver rating has earned |
| `hide_locked_teams` | `false` | No | Hide teams you cannot claim instead of listing them greyed out |
| `starting_balance` | `5000000` | No | Credits a **newly created** career begins with; recorded on the save at creation |
| `contract_top_salary` | `4000000` | No | Per-season pay for the quickest car on the grid |
| `contract_floor_salary` | `100000` | No | Per-season pay for the slowest car; held below the top if a hand-edited file inverts the pair |
| `contract_buy_in_per_point` | `150000` | No | Sponsorship demanded per rating point short of a selling team's bar. `0` switches pay-driver seats off |
| `contract_pay_driver_margin` | `10` | No | Rating points clear of a back-marker's bar before it pays you rather than charging you |
| `champion_prize` | `2000000` | No | Credits for winning a championship |
| `last_place_prize` | `50000` | No | Credits for finishing last among the drivers who scored |
| `starting_rating` | `50` | No | Rating a driver with no results starts at, 0–100 |
| `rating_strictness` | `0` | No | Rating points added to every team requirement; negative opens the grid up |
| `offer_margin` | `10` | No | How far below a team requirement you may sit and still be offered the seat on merit |
| `eligibility_gates` | `both` | No | Build a team requirement from car pace, incumbent skill, or both (stricter wins) |
| `rating_half_life` | `10` | No | Results this far back count half; `0` weighs a whole career equally |
| `count_retirements` | `true` | No | Whether a DNF costs a point of finish rate |
| `retirement_min_laps_down` | `3` | No | Laps behind the leader before a car counts as retired rather than lapped |
| `retirement_distance_pct` | `90` | No | Share of the leader's distance a car must fall short of to count as retired |

Settings marked *restart required* are written to disk immediately but only take effect after restarting the server. All other settings apply on save without a restart.

The **rating tuning** keys and `starting_balance` are copied onto a career when it is created and then kept, so editing them changes *new* careers only. The Config tab has an explicit button to move the career you are playing onto the current rating settings — see [Driver Rating & Performance](docs/Rating-and-Performance.md).

When a new config key is added in a future version the existing file is updated with the default value automatically on the next startup.

## UI tabs

| Tab | Content |
|---|---|
| **Live Session** | Real-time timing table and telemetry panel, updated via WebSocket from AMS2 shared memory |
| **Career** | Championships, Driver Stats, Track Stats and Finances sub-tabs |
| **Manage** | Create championships, assign recorded sessions to rounds, edit points systems and status, sign for a team |
| **Config** | Career saves (create / switch / duplicate / rename / delete) and server configuration |
| **Car Performance** | Per-class performance scalars from the Custom AI Drivers rosters, editable, with baseline and reset |
| **Driver Performance** | Per-driver skill values from the same rosters, editable |

Car Performance, Driver Performance and Career → Finances all describe a Custom AI roster or a contract, so they are hidden in a multiplayer career.

### Live Session columns

| Column | Description |
|---|---|
| Pos | Current race/session position |
| Driver | Participant name |
| Laps | Laps completed |
| Interval | Gap to the car directly ahead (race sessions only); shown as seconds or laps |
| Gap | Delta to the overall fastest lap set in the session |
| S1 / S2 / S3 | Sector times — current lap sector when available, personal best otherwise. **Purple** = overall fastest sector; **green** = driver's personal best |
| Best Lap | Driver's fastest lap of the session |
| Last Lap | Driver's most recently completed lap time |
| Car / Team | Historic team name from the active championship's Custom AI Drivers file, falling back to the car model AMS2 reports |
| Tyre | Player's current tyre compound (e.g. Soft / Medium / Hard) |

### Career sub-tabs

| Sub-tab | Content |
|---|---|
| **Championships** | Master-detail list of championships; sidebar shows status badge; detail panel shows standings, constructor standings, round results and per-race lap charts |
| **Driver Stats** | Aggregated stats per driver across all championships |
| **Track Stats** | Per-track summary across all recorded sessions |
| **Finances** | Balance, the season being raced as a running total, the season-by-season ledger, and where the money went (singleplayer only) |

### Career Driver Stats columns

| Column | Description |
|---|---|
| Driver | Name |
| Races | Total race starts |
| 1st / 2nd / 3rd | Race podium finishes by position |
| Top 10 | Points-zone race finishes |
| Avg Pos | Average finishing position across all races |
| DNF | Did-not-finish races |
| Q Pole / Q 2nd / Q 3rd / Q Top 10 | Qualifying results by position |
| C 1st / C 2nd / C 3rd | Championship final standings finishes (Final championships only) |

### Track Stats columns

| Column | Description |
|---|---|
| Track | Track name and layout variant |
| Races | Number of recorded race sessions at this track |
| Qualifyings | Number of recorded qualifying sessions at this track |
| Best Lap / 2nd Lap / 3rd Lap | The three fastest laps at this track, each with the driver who set it and the car |
| Last Visited | Date of the most recent session at this track |

## Career data

Each career is a **folder** inside the saves folder (`championships/` next to the executable by default). The folder's name is the career's name:

```
championships/
  ams2_career/
    career.json            <- the career: sessions, championships, contracts
    laps/
      1728394855.json      <- one lap chart per race session
  gt3_2025/
    career.json
  track_layouts/           <- shared by every career
    silverstone.json
```

`career.json` holds:

- **`mode`** — `singleplayer` or `multiplayer`, chosen at creation and never changed afterwards
- **`sessions`** — each recorded session: track, timestamp, session type, and per-driver results (position, laps, fastest lap, last lap, DNF flag, car name)
- **`championships`** — each championship: name, status (`Active` / `Progress` / `Final`), points system, constructor scoring flag, planned rounds, assigned Custom AI Drivers file and player team, and the ordered list of rounds (each round holds one or more session IDs)
- **`contracts`** — the deal signed for each season, plus what it settled for once the season was marked Final
- **`starting_balance`** and **`rating_params`** — stamped from config when the career was created, so later config edits never rewrite a career's history

Lap charts live beside the career in `laps/` rather than inside it. They are the biggest thing a session carries and nothing aggregate reads them, so keeping them out means recording a round does not rewrite them all; `GET /api/sessions/:id/lap-chart` serves one on demand.

At most one championship carries the `Active` status: it marks the season currently being raced, and `PATCH /api/championships/:id` demotes any previous holder to `Progress`. The live timing grid reads its team names from that championship, so the flag is load-bearing rather than cosmetic.

A career written before folders existed — a flat `<name>.json` directly in the saves folder — is still read, and is never migrated. Its lap charts stay inside the file.

Track layout data is stored as JSON files in `championships/track_layouts/`, one per track, named by a slug of the track name. They are built automatically from car positions during live sessions and shared by every career.

## REST API

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/sessions` | List all recorded sessions |
| `DELETE` | `/api/sessions/unassigned` | Delete every session not assigned to a round, and its lap chart |
| `GET` | `/api/sessions/:id/lap-chart` | Lap chart for one session; `[]` when there is none |
| `GET` | `/api/career` | Pre-computed career view: standings, constructor standings, rounds, driver stats, track stats |
| `GET` | `/api/career/finances` | Balance, season ledger, and the season being raced |
| `PATCH` | `/api/career/mode` | Set the career mode — allowed once, only from unset |
| `POST` | `/api/career/rating/adopt` | Move the current career onto the rating tuning in config |
| `GET` | `/api/championships` | List all championships |
| `POST` | `/api/championships` | Create a championship; a singleplayer one requires a Custom AI file and a planned round count |
| `PATCH` | `/api/championships/:id` | Update name, status, points system, planned rounds, Custom AI file or player team. Setting status to `Active` demotes any other Active championship to `Progress` |
| `DELETE` | `/api/championships/:id` | Delete a championship |
| `GET` | `/api/championships/:id/teams` | Teams on that championship's roster |
| `GET` | `/api/championships/:id/team-eligibility` | Per-team requirement, and whether the rating has earned it |
| `GET` | `/api/championships/:id/session-eligibility` | Which recorded sessions may be assigned, and why not |
| `GET` | `/api/championships/:id/grid-check` | How each of the season's recorded sessions lined up with its roster |
| `GET` | `/api/championships/:id/offers` | Seat offers for the season, re-derived on every request |
| `POST` | `/api/championships/:id/sign` | Sign for a team; the terms are regenerated server-side |
| `DELETE` | `/api/championships/:id/sign` | Tear up an unraced contract and clear the seat it came with |
| `POST` | `/api/championships/:id/rounds` | Add a round to a championship |
| `DELETE` | `/api/championships/:id/rounds/:r` | Delete a round and its session assignments |
| `POST` | `/api/championships/:id/rounds/:r/sessions/:sid` | Assign a session to a round |
| `DELETE` | `/api/championships/:id/rounds/:r/sessions/:sid` | Unassign a session from a round |
| `POST` | `/api/record-session` | Manually capture the current live session regardless of auto-record settings |
| `GET` | `/api/saves` | List career saves with their counts, mode, and any load error |
| `POST` | `/api/saves` | Create a new empty career with its mode, and switch to it |
| `POST` | `/api/saves/activate` | Switch to an existing career — no restart |
| `POST` | `/api/saves/duplicate` | Copy a career under a new name; the active one is unchanged |
| `PATCH` | `/api/saves/:name` | Rename a career |
| `DELETE` | `/api/saves/:name` | Delete a career and everything in its folder |
| `GET` | `/api/custom-ai-files` | Custom AI Drivers rosters found in `custom_ai_dir` |
| `GET` `PATCH` | `/api/car-performance` | Per-class performance scalars; PATCH writes the roster XML |
| `POST` | `/api/car-performance/baseline` | Adopt the current roster as the class's reset point |
| `POST` | `/api/car-performance/reset` | Restore a class from its baseline |
| `GET` `PATCH` | `/api/driver-performance` | Per-driver skill values; PATCH writes the roster XML |
| `GET` | `/api/config` | Read current server configuration, plus whether the career's rating tuning still matches it |
| `PATCH` | `/api/config` | Write server configuration |
| `GET` `PATCH` | `/api/spotter` | Read or update the voice spotter settings |
| `GET` | `/api/spotter/voices` | TTS voices installed on this PC |
| `GET` | `/api/track-layout/:track` | Load saved track radar points for a track |
| `POST` | `/api/track-layout/:track` | Save track radar points for a track |
| `GET` | `/api/live-teams` | Driver → team names for the live grid, from the active championship's Custom AI Drivers file, plus that championship's player team and any grid warning |
| `GET` | `/live` | Current AMS2 session state snapshot (JSON) |
| `WS` | `/ws` | WebSocket endpoint — pushes live session JSON at the configured poll interval |

## Development

### VS Code

A `.vscode/launch.json` is included with a launch configuration selectable from the Run & Debug panel (Ctrl+Shift+D):

- **ams2_championship_server (serve on :8080)** — builds and starts the HTTP server

Press **Ctrl+Shift+B** to pick a build task (build / test / clippy / fmt).

### Running tests

```bash
cargo test
```

Around 657 tests live in `src/tests/`, wired into their parent modules with `#[path = "tests/…"]` so they can reach `pub(crate)` items:

- `data_store.rs` — JSON persistence round-trips, standings and the countback tiebreak, constructor scoring, `compute_career` aggregation, track stats
- `session_recorder.rs` — session capture, `should_capture`, and the recorder state machine driven one poll at a time (replay freeze, restarts, disconnects)
- `config.rs` — config load/create/defaults and economy clamping
- `saves.rs` — both save layouts, name gating, rename/duplicate/delete
- `lap_charts.rs` — externalising a chart, reading it back, the legacy inline fallback
- `driver_rating.rs` — ratings and team requirements, pinned against a reference career
- `contracts.rs` — offers, renewals, pay-driver seats, salary instalments, sealing
- `custom_ai.rs`, `liveries.rs`, `season_years.rs` — roster parsing and writing, baselines, team-name resolution
- `server.rs` — HTTP request parsing, SHA-1, base64, WebSocket accept-key (RFC 6455), track slug generation, and route integration tests over real TCP loopback

### Project structure

```
src/
  lib.rs                         # Library crate entry point
  championship_html.rs           # HTML template and embedded asset constants
  ams2_shared_memory.rs          # AMS2 shared memory reader (Windows, $pcars2$ API)
  config.rs                      # Config struct, JSON load/create with per-field serde defaults
  data_store.rs                  # Career data model, JSON persistence, standings/career computation
  saves.rs                       # Career saves: folder layout, listing, create/rename/duplicate/delete
  lap_charts.rs                  # Lap charts, stored beside the career in laps/
  driver_rating.rs               # Driver rating and per-team requirements, derived from results
  contracts.rs                   # Seat offers, contracts, salary, prize money, the ledger
  custom_ai.rs                   # Custom AI Drivers roster XML: read, write, baselines
  liveries.rs                    # Livery string to team name resolution
  season_years.rs                # Season year parsing from roster and livery names
  spotter.rs                     # Voice spotter state machine and TTS subprocess
  http.rs                        # HTTP primitives: Request, send_response, json_ok/err, track_slug
  session_recorder.rs            # Background thread: detects session end, captures results
  websocket.rs                   # WebSocket handshake (SHA-1, base64, RFC 6455) and live push loop
  tests/                         # Unit and integration tests, wired in with #[path]
  assets/
    style.css                    # Embedded at compile time via include_str!
    utils.js                     # Shared helpers: formatting, sorting, sortable tables
    telemetry.js                 # Telemetry panel: damage, tyre fields, ride-height record
    track_map.js                 # Track radar: point accumulation, disk save/load, canvas rendering
    live.js                      # Live timing table rendering and WebSocket connection
    career.js                    # Career championships (master-detail), stats, lap charts
    manage.js                    # Manage tab CRUD
    contracts.js                 # Seat offers and the Finances page
    config.js                    # Config tab: load/save config, adopt rating tuning
    saves.js                     # Career switcher and save list; hides what a multiplayer career lacks
    car_performance.js           # Car Performance tab
    driver_performance.js        # Driver Performance tab
    main.js                      # Tab init and sub-tab wiring
  bin/
    ams2_championship_server.rs  # handle() route dispatcher and main(); HTTP/WebSocket via lib modules
```

## Dependencies

| Crate | Purpose |
|---|---|
| [`serde`](https://crates.io/crates/serde) | Derive macros for JSON serialisation |
| [`serde_json`](https://crates.io/crates/serde_json) | JSON serialisation for the career API, config file, and `/live` endpoint |
| [`windows-sys`](https://crates.io/crates/windows-sys) | Windows shared memory API (`OpenFileMappingW`, `MapViewOfFile`) — Windows target only |
