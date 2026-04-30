# AMS2 Championships

[![Release](https://img.shields.io/github/v/release/Nightrat/ams2_championships)](https://github.com/Nightrat/ams2_championships/releases/latest)
[![Tests](https://github.com/Nightrat/ams2_championships/actions/workflows/rust.yml/badge.svg)](https://github.com/Nightrat/ams2_championships/actions/workflows/rust.yml)

> **Download the latest release:** [ams2_championship_server.exe](https://github.com/Nightrat/ams2_championships/releases/latest/download/ams2_championship_server.exe)

A motorsport career tracker for Automobilista 2. It records race results directly from the AMS2 shared memory API, lets you organise them into championships, and displays everything in a browser-based UI with a real-time live timing overlay.

> **Note:** The majority of the code in this repository was written with the assistance of [Claude](https://claude.ai) (Anthropic AI).

## Documentation

- [Getting Started](docs/Getting-Started.md)
- [Live Session](docs/Live-Session.md)
- [Career Tab](docs/Career.md)
- [Managing Championships](docs/Managing-Championships.md)
- [Data & Backup](docs/Data-and-Backup.md)

## Features

- **Session recorder** — automatically captures race and qualifying results at session end from the AMS2 shared memory API; no external tool required
- **Championship management** — create championships, assign recorded sessions to rounds, set points systems (F1 modern/classic or custom), toggle constructor scoring, and track status (Active / Progress / Final)
- **Championship standings** — master-detail view with per-championship driver and constructor standings, collapsible round-by-round results (qualifying and race)
- **Career statistics** — aggregated stats across all championships: race starts, podium splits (1st/2nd/3rd), top-10 finishes, average finishing position, DNFs, qualifying results (pole/2nd/3rd/top-10), and championship standings finishes (1st/2nd/3rd)
- **Track statistics** — per-track summary across all recorded sessions: race and qualifying counts, best lap time with record holder name and car, last visited date
- **Live session overlay** — real-time timing table pushed over WebSocket at 5 Hz from AMS2 shared memory: position, laps, race interval, gap to fastest lap, sector times, best/last lap, top speed, and tyre compound for the player
- **Track radar** — canvas overlay on the live timing view that builds a map of the track from car positions and renders all participants as dots; map is saved to disk per track and loaded on the next visit
- **Telemetry panel** — player tyre temperatures (inner/mid/outer per corner), tyre wear/pressure, brake temperatures, suspension travel, and automatic setup recommendations based on a rolling 20-sample average
- **PDF export** — download a print-ready PDF of all championships with all round details expanded
- **Configuration** — browser-based Config tab writes a `config.json` next to the executable; all settings have documented defaults and the file is created automatically on first run

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable, 2021 edition)
- Windows (the session recorder and live overlay read the `$pcars2$` named shared memory, which is Windows-only)

## Build

```bash
cargo build --release
```

## Usage

```bash
cargo run --release --bin ams2_championship_server
```

On first run the server creates a `championships/` folder and a `config.json` file next to the executable, then:

1. Loads existing career data from `championships/ams2_career.json` (path is configurable)
2. Starts a background session recorder that saves results automatically when a race or qualifying session ends in AMS2
3. Serves the UI at `http://127.0.0.1:8080/` (host and port are configurable)

Open the URL in a browser. Press **Ctrl+C** to stop.

## Configuration

On first run `config.json` is created next to the executable with all defaults. Edit it directly or use the **Config** tab in the UI.

| Key | Default | Restart required | Description |
|---|---|---|---|
| `port` | `8080` | Yes | HTTP and WebSocket port |
| `host` | `"127.0.0.1"` | Yes | Bind address — use `"0.0.0.0"` to allow LAN access |
| `data_file` | `null` | Yes | Full path to the career JSON file; `null` uses `championships/ams2_career.json` next to the executable |
| `poll_ms` | `200` | No | Shared memory read interval in milliseconds (live view refresh rate) |
| `record_practice` | `true` | Yes | Automatically save practice sessions when AMS2 is running |
| `record_qualify` | `true` | Yes | Automatically save qualifying sessions when AMS2 is running |
| `record_race` | `true` | Yes | Automatically save race sessions when AMS2 is running |
| `show_track_map` | `true` | No | Show the track radar canvas in the live timing view |
| `track_map_max_points` | `5000` | No | Maximum unique grid cells accumulated for the track radar before collection stops |

Settings marked *restart required* are written to disk immediately but only take effect after restarting the server. All other settings apply on save without a restart.

When a new config key is added in a future version the existing file is updated with the default value automatically on the next startup.

## UI tabs

| Tab | Content |
|---|---|
| **Live Session** | Real-time timing table and telemetry panel, updated via WebSocket from AMS2 shared memory |
| **Career** | Championships sub-tab (master-detail view), Driver Stats sub-tab, and Track Stats sub-tab |
| **Manage** | Create championships, assign recorded sessions to rounds, edit points systems and status |
| **Config** | Edit server configuration; changes are written to `config.json` immediately |

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
| Top km/h | Highest recorded speed (capped at 450 km/h to filter teleport spikes) |
| Tyre | Player's current tyre compound (e.g. Soft / Medium / Hard) |

### Career sub-tabs

| Sub-tab | Content |
|---|---|
| **Championships** | Master-detail list of championships; sidebar shows status badge; detail panel shows standings, constructor standings, and round results |
| **Driver Stats** | Aggregated stats per driver across all championships |
| **Track Stats** | Per-track summary across all recorded sessions: races, qualifyings, best lap with record holder and car, last visited |

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
| Best Lap | Fastest lap recorded at this track across all sessions |
| Record Holder | Driver who set the best lap |
| Car | Car used to set the best lap (falls back to car class if name unavailable) |
| Last Visited | Date of the most recent session at this track |

## Career data

Career data is stored as JSON in `championships/ams2_career.json` next to the server executable (or at the path set in `config.json`). The file is created automatically on first run and updated after every recorded session. It contains two top-level arrays:

- **`sessions`** — each recorded session: track, timestamp, session type, and per-driver results (position, laps, fastest lap, last lap, DNF flag, car name)
- **`championships`** — each user-created championship: name, status (`Active` / `Progress` / `Final`), points system, constructor scoring flag, and the ordered list of rounds (each round contains one or more session IDs)

Track layout data is stored as JSON files in `championships/track_layouts/` — one file per track, named by a slug of the track name. These are built automatically from car positions during live sessions and loaded on subsequent visits.

## REST API

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/sessions` | List all recorded sessions |
| `GET` | `/api/championships` | List all championships |
| `GET` | `/api/career` | Pre-computed career view: standings, constructor standings, rounds, driver stats, and track stats |
| `POST` | `/api/championships` | Create a championship |
| `PATCH` | `/api/championships/:id` | Update name, status, or points system |
| `DELETE` | `/api/championships/:id` | Delete a championship |
| `POST` | `/api/championships/:id/rounds` | Add a round to a championship |
| `POST` | `/api/championships/:id/rounds/:r/sessions/:sid` | Assign a session to a round |
| `DELETE` | `/api/championships/:id/sessions/:sid` | Remove a session assignment |
| `POST` | `/api/record-session` | Manually capture the current live session regardless of auto-record settings |
| `GET` | `/api/config` | Read current server configuration |
| `PATCH` | `/api/config` | Write server configuration (optionally moves the data file) |
| `GET` | `/api/track-layout/:track` | Load saved track radar points for a track |
| `POST` | `/api/track-layout/:track` | Save track radar points for a track |
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

**117 unit tests** across four test files in `src/tests/`:

- `src/tests/data_store.rs` — JSON persistence round-trips, standings computation, constructor scoring, `compute_career` aggregation, track stats, points-earned in result views, qualifying position stats, championship standings finishes
- `src/tests/session_recorder.rs` — session capture and `should_capture` logic
- `src/tests/config.rs` — config load/create/defaults
- `src/tests/server.rs` — HTTP request parsing, SHA-1, base64, WebSocket accept-key (RFC 6455), track slug generation, HTTP route integration tests

### Project structure

```
src/
  lib.rs                         # Library crate entry point
  championship_html.rs           # HTML template and embedded asset constants
  ams2_shared_memory.rs          # AMS2 shared memory reader (Windows, $pcars2$ API)
  config.rs                      # Config struct, JSON load/create with per-field serde defaults
  data_store.rs                  # Career data model, JSON persistence, standings/career computation
  http.rs                        # HTTP primitives: Request, send_response, json_ok/err, read_full_request, track_slug
  session_recorder.rs            # Background thread: detects race end, captures results
  websocket.rs                   # WebSocket handshake (SHA-1, base64, RFC 6455) and live push loop
  tests/
    data_store.rs                # Unit tests for data_store
    session_recorder.rs          # Unit tests for session_recorder
    config.rs                    # Unit tests for config
    server.rs                    # Unit tests for the server binary
  assets/
    style.css                    # Embedded at compile time via include_str!
    utils.js                     # Shared helpers: formatting, sorting, tab switching
    track_map.js                 # Track radar: point accumulation, disk save/load, canvas rendering
    live.js                      # Live timing table rendering and WebSocket connection
    career.js                    # Career championships (master-detail), driver stats, and track stats
    manage.js                    # Manage tab CRUD
    config.js                    # Config tab: load/save config, apply track map settings
    telemetry.js                 # Telemetry panel: tyre/brake temps, setup recommendations
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
