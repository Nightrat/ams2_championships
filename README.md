# AMS2 Championships

[![Release](https://img.shields.io/github/v/release/Nightrat/ams2_championships)](https://github.com/Nightrat/ams2_championships/releases/latest)

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

- **Session recorder** — automatically captures race results at session end from the AMS2 shared memory API; no external tool required
- **Championship management** — create championships, assign recorded sessions to rounds, set points systems (F1 modern/classic or custom), toggle constructor scoring, and track status (Pending / Active / Finished)
- **Championship standings** — per-championship driver and constructor standings computed from race results, with collapsible round-by-round detail
- **Career statistics** — aggregated stats across all championships: races, wins, top-3/10 finishes, DNFs, championship wins, and average finishing position
- **Live session overlay** — real-time timing table pushed over WebSocket from the AMS2 shared memory API: position, lap count, gap to fastest, sector times (S1/S2/S3), best lap, last lap, and top speed for all active participants
- **Telemetry panel** — per-driver tyre temperatures/wear, brake temperatures, and setup recommendations
- **Track map** — canvas-based track layout built from positional data, with live car positions
- **PDF export** — download a print-ready PDF of the career standings via the browser's native print dialog

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable, 2021 edition)
- Windows (the session recorder and live overlay read the `$pcars2$` named shared memory, which is Windows-only)

## Build

```bash
cargo build --release
```

## Usage

```bash
cargo run --release --bin ams2_championship_server -- [port]
```

Default port is `8080`. The server:

1. Creates a `championships/` folder next to the executable on first run
2. Loads existing career data from `championships/ams2_career.json`
3. Starts a background session recorder that saves race results automatically when a race ends in AMS2
4. Serves the UI at `http://127.0.0.1:8080/`

```bash
# with a custom port
cargo run --release --bin ams2_championship_server -- 9000
```

Open `http://127.0.0.1:8080/` in a browser. Press **Ctrl+C** to stop.

## UI tabs

| Tab | Content |
|---|---|
| **Live Session** | Real-time timing table and telemetry panel, updated via WebSocket from AMS2 shared memory |
| **Career** | Driver standings and round-by-round results for each championship; aggregated driver stats |
| **Manage** | Create championships, assign recorded sessions to rounds, edit points systems and status |

### Live Session columns

| Column | Description |
|---|---|
| Pos | Current race/session position |
| Driver | Participant name |
| Lap | Current lap number |
| Gap | Delta to the overall fastest lap set in the session |
| S1 / S2 / S3 | Sector times — current lap sector when available, personal best otherwise. **Purple** = overall fastest sector; **green** = driver's personal best |
| Best Lap | Driver's fastest lap of the session |
| Last Lap | Driver's most recently completed lap time |
| Top km/h | Highest recorded speed (capped at 450 km/h to filter teleport spikes) |

### Career Driver Stats columns

| Column | Description |
|---|---|
| Driver | Name |
| Races | Total race starts |
| Wins | First-place finishes |
| Top 3 | Podium finishes |
| Top 10 | Points-zone finishes |
| DNF | Did-not-finish races |
| Champ Wins | Championship titles |
| Avg Pos | Average finishing position |

## Career data

Career data is stored as JSON in `championships/ams2_career.json` next to the server executable. The file is created automatically on first run and updated after every recorded race. It contains two top-level arrays:

- **`sessions`** — each recorded race: track, timestamp, session type, and per-driver results (position, laps, fastest lap, last lap, DNF flag)
- **`championships`** — each user-created championship: name, status, points system, constructor scoring flag, and the ordered list of rounds (each round contains one or more session IDs)

## REST API

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/sessions` | List all recorded sessions |
| `GET` | `/api/championships` | List all championships |
| `GET` | `/api/career` | Pre-computed career view: standings, constructor standings, rounds, and driver stats |
| `POST` | `/api/championships` | Create a championship |
| `PATCH` | `/api/championships/:id` | Update name, status, or points system |
| `DELETE` | `/api/championships/:id` | Delete a championship |
| `POST` | `/api/championships/:id/rounds` | Add a round to a championship |
| `POST` | `/api/championships/:id/rounds/:r/sessions/:sid` | Assign a session to a round |
| `DELETE` | `/api/championships/:id/sessions/:sid` | Remove a session assignment |
| `GET` | `/live` | Current AMS2 session state snapshot (JSON) |
| `WS` | `/ws` | WebSocket endpoint — pushes live session JSON every 300 ms |

## Development

### VS Code

A `.vscode/launch.json` is included with a launch configuration selectable from the Run & Debug panel (Ctrl+Shift+D):

- **ams2_championship_server (serve on :8080)** — builds and starts the HTTP server

Press **Ctrl+Shift+B** to pick a build task (build / test / clippy / fmt).

### Running tests

```bash
cargo test
```

**55 unit tests** across two test files:

- `src/data_store_tests.rs` — JSON persistence round-trips, standings computation, constructor scoring, `compute_career` aggregation
- `src/server_tests.rs` — HTTP request parsing, SHA-1, base64, WebSocket accept-key (RFC 6455), track slug generation

### Project structure

```
src/
  lib.rs                         # Library crate entry point
  championship_html.rs           # HTML template and embedded asset constants
  ams2_shared_memory.rs          # AMS2 shared memory reader (Windows, $pcars2$ API)
  data_store.rs                  # Career data model, JSON persistence, standings/career computation
  data_store_tests.rs            # Unit tests for data_store (via #[path])
  session_recorder.rs            # Background thread: detects race end, captures results
  server_tests.rs                # Unit tests for the server binary (via #[path])
  assets/
    style.css                    # Embedded at compile time via include_str!
    utils.js                     # Shared helpers: formatting, sorting, tab switching
    live.js                      # Live timing and telemetry rendering
    career.js                    # Career championships and stats rendering
    manage.js                    # Manage tab CRUD
    track_map.js                 # Canvas track map
    telemetry.js                 # Setup/telemetry panel
    main.js                      # Tab init and sub-tab wiring
  bin/
    ams2_championship_server.rs  # HTTP server, REST API, WebSocket, session recorder startup
```

## Dependencies

| Crate | Purpose |
|---|---|
| [`serde`](https://crates.io/crates/serde) | Derive macros for JSON serialisation |
| [`serde_json`](https://crates.io/crates/serde_json) | JSON serialisation for the career API and `/live` endpoint |
| [`windows-sys`](https://crates.io/crates/windows-sys) | Windows shared memory API (`OpenFileMappingW`, `MapViewOfFile`) — Windows target only |
