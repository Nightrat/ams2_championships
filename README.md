# AMS2 Championships

[![Release](https://img.shields.io/github/v/release/Nightrat/ams2_championships)](https://github.com/Nightrat/ams2_championships/releases/latest)

> **Download the latest release:** [ams2_championship.exe](https://github.com/Nightrat/ams2_championships/releases/latest/download/ams2_championship.exe) · [ams2_championship_server.exe](https://github.com/Nightrat/ams2_championships/releases/latest/download/ams2_championship_server.exe)

A motorsport career tracker for Automobilista 2. It records race results directly from the AMS2 shared memory API, lets you organise them into championships, and displays everything in a browser-based UI with a real-time live timing overlay. An optional import tab supports legacy data from [Second Monitor](https://gitlab.com/raceengineer1/second-monitor).

## Features

- **Session recorder** — automatically captures race results at session end from the AMS2 shared memory API; no external tool required
- **Championship management** — create championships, assign recorded sessions to them, set points systems (F1 modern/classic or custom), and track status (Pending / Active / Finished)
- **Championship standings** — per-championship driver standings computed from race results, with collapsible round-by-round detail
- **Live session overlay** — real-time timing table powered by the `$pcars2$` shared memory API: position, lap count, gap to fastest, sector times (S1/S2/S3), best lap, and last lap for all active participants
- **SecondMonitor import** — optional import of `Championships.xml` produced by Second Monitor, with full standings, constructor standings, round-by-round results grid, expandable event details, and driver portraits fetched from Wikipedia
- **Driver statistics** — aggregated stats across all imported championships: races, wins, top-3/10 finishes, DNFs, championship podiums, and average finishing position
- **Dark theme UI** — tab-based layout, collapsible sections, sortable stats table, progress bars, and badge indicators
- **Download button** — save the page as a static self-contained HTML file directly from the browser

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable, 2021 edition)
- Windows (the session recorder and live overlay read the `$pcars2$` named shared memory, which is Windows-only)
- Second Monitor is **no longer required** — career data is recorded automatically by the server

## Build

```bash
cargo build --release
```

## Usage

### Web server (recommended)

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

### Generate a static HTML file (SecondMonitor import only)

```bash
cargo run --release --bin ams2_championship -- <path/to/Championships.xml>
```

Generates `championships.html` in the current working directory. The static file includes the SecondMonitor Import tab but does not support live data or the career API.

**Example** (default Second Monitor path on Windows):

```bash
cargo run --release --bin ams2_championship -- "%USERPROFILE%\OneDrive\Documents\SecondMonitor\Championships.xml"
```

### Server with SecondMonitor import

Pass the XML path as the first argument to also populate the SecondMonitor Import tab:

```bash
cargo run --release --bin ams2_championship_server -- "%USERPROFILE%\OneDrive\Documents\SecondMonitor\Championships.xml" 8080
```

## UI tabs

| Tab | Content |
|---|---|
| **Live Session** | Real-time timing table, updated every 2 seconds from AMS2 shared memory (server mode only) |
| **Championships** | Driver standings and round-by-round results for each championship created in the Manage tab |
| **Manage** | Create championships, assign recorded sessions, edit points systems and status |
| **SecondMonitor Import** | Data imported from a `Championships.xml` file (sub-tabs: Championships, Driver Stats) |

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

### Driver Stats columns (SecondMonitor Import)

| Column | Description |
|---|---|
| Driver | Name with Wikipedia portrait (if found) |
| Seasons | Number of finished championships |
| Races | Total race starts |
| Wins | First-place finishes |
| Top 3 | Podium finishes |
| Top 10 | Points-zone finishes |
| DNF | Races where the session was not completed (player only) |
| Champ Wins | Championship titles |
| Champ Top 3 | Top-3 championship finishes |
| Champ Top 10 | Top-10 championship finishes |
| Avg Pos | Average finishing position |

## Career data

Career data is stored as JSON in `championships/ams2_career.json` next to the server executable. The file is created automatically on first run and updated after every recorded race. It contains two top-level arrays:

- **`sessions`** — each recorded race: track, timestamp, session type, and per-driver results (position, laps, fastest lap, last lap, DNF flag)
- **`championships`** — each user-created championship: name, status, points system, and the ordered list of session IDs assigned to it

## REST API

The server exposes a JSON API used by the Manage and Championships tabs:

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/sessions` | List all recorded sessions |
| `GET` | `/api/championships` | List all championships |
| `POST` | `/api/championships` | Create a championship (`{"name": "...", "points_system": [...]}`) |
| `PATCH` | `/api/championships/:id` | Update name, status, or points system |
| `DELETE` | `/api/championships/:id` | Delete a championship |
| `POST` | `/api/championships/:id/sessions/:sid` | Assign a session to a championship |
| `DELETE` | `/api/championships/:id/sessions/:sid` | Remove a session from a championship |
| `GET` | `/live` | Current AMS2 session state (real-time telemetry) |

## Development

### VS Code

A `.vscode/launch.json` is included with two launch configurations selectable from the Run & Debug panel (Ctrl+Shift+D):

- **ams2_championship (generate HTML)** — builds and runs the static file generator
- **ams2_championship_server (serve on :8080)** — builds and starts the HTTP server

Press **Ctrl+Shift+B** to pick a build task (build / test / clippy / fmt).

### Running tests

```bash
cargo test
```

**Unit tests** (77 total) cover:
- XML parsing helpers and session deserialization
- Stat computation logic (DNF attribution, AI name merging, session filtering)
- HTML generation functions (standings table, championship section, constructor standings, escaping)
- Data store: JSON persistence round-trips, error recovery from invalid/missing files
- HTTP request parsing: method, path, body extraction for all supported routes

**Integration tests** run the full `convert` pipeline against a minimal fixture XML and assert on the generated HTML.

### Project structure

```
src/
  lib.rs                         # Library entry point, re-exports public functions
  main.rs                        # Binary: generate static HTML file
  championship_html.rs           # SecondMonitor XML parsing, stat computation, HTML generation
  ams2_shared_memory.rs          # AMS2 shared memory reader (Windows, $pcars2$ API)
  data_store.rs                  # Career data model (sessions + championships), JSON persistence
  session_recorder.rs            # Background thread: detects race end, captures results
  assets/
    style.css                    # Embedded at compile time via include_str!
    script.js                    # Embedded at compile time via include_str!
  bin/
    ams2_championship_server.rs  # Binary: HTTP server, REST API, session recorder startup
tests/
  integration_test.rs            # End-to-end tests against a fixture XML
  fixtures/
    minimal.xml                  # Minimal two-round championship fixture
```

## Dependencies

| Crate | Purpose |
|---|---|
| [`quick-xml`](https://crates.io/crates/quick-xml) | XML deserialisation via serde (SecondMonitor import) |
| [`serde`](https://crates.io/crates/serde) | Derive macros for XML and JSON serialisation |
| [`serde_json`](https://crates.io/crates/serde_json) | JSON serialisation for the career API and `/live` endpoint |
| [`ureq`](https://crates.io/crates/ureq) | HTTP requests to the Wikipedia REST API (driver portraits) |
| [`windows-sys`](https://crates.io/crates/windows-sys) | Windows shared memory API (`OpenFileMappingW`, `MapViewOfFile`) — Windows target only |
