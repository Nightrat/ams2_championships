# AMS2 Championships

A tool that converts your Automobilista 2 career championship save data into a self-contained HTML report, with a built-in web server that also streams a live session overlay directly from the AMS2 shared memory API.

## Features

- **Championship overview** — collapsible sections per championship with standings, constructor standings, round-by-round results grid, and expandable event details
- **Constructor standings** — per-championship points table grouped by car model, shown when manufacturer scoring is enabled in the save
- **Driver statistics** — aggregated stats across all championships: races, wins, top-3/10 finishes, DNFs, championship podiums, and average finishing position
- **DNF tracking** — races where the session was not completed (player retired early) are counted and shown in a dedicated column
- **Driver portraits** — automatically fetched from Wikipedia for real-world driver names
- **Live session overlay** — real-time timing table powered by the AMS2 shared memory API (`$pcars2$`): position, lap count, gap to fastest, sector times (S1/S2/S3), best lap, and last lap for all active participants
- **Dark theme UI** — sortable stats table, tab switching, collapsible championship sections, progress bars, and badge indicators
- **Download button** — save the currently rendered page as a static self-contained HTML file directly from the browser

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable, 2021 edition)
- AMS2 monitored by [Second Monitor](https://gitlab.com/raceengineer1/second-monitor), which produces the `Championships.xml` save file
- For the live session overlay: the server binary must be running while AMS2 is open (Windows only — reads the `$pcars2$` named shared memory)

## Build

```bash
cargo build --release
```

## Usage

### Generate a static HTML file

```bash
cargo run --release --bin ams2_championship -- <path/to/Championships.xml>
```

This generates `championships.html` in the current working directory. Open it in any browser.

**Example** (default Second Monitor path on Windows):

```bash
cargo run --release --bin ams2_championship -- "%USERPROFILE%\OneDrive\Documents\SecondMonitor\Championships.xml"
```

### Serve over HTTP (with live session overlay)

```bash
cargo run --release --bin ams2_championship_server -- <path/to/Championships.xml> [port]
```

Generates the HTML once at startup (including the Wikipedia portrait fetch) and serves it at `http://127.0.0.1:<port>/`. Default port is `8080`. While the server is running, the **Live Session** tab polls `/live` every 2 seconds to read current session data directly from AMS2.

```bash
cargo run --release --bin ams2_championship_server -- "%USERPROFILE%\OneDrive\Documents\SecondMonitor\Championships.xml" 8080
```

Then open `http://127.0.0.1:8080/` in a browser. Press **Ctrl+C** to stop.

## Output

The generated HTML file contains three tabs:

| Tab | Content |
|---|---|
| **Championships** | One collapsible section per championship (pending/active open by default, finished collapsed) with driver standings, optional constructor standings, a round-by-round results grid, and expandable event details |
| **Driver Stats** | A sortable table aggregating stats for every driver across all championships |
| **Live Session** | Real-time timing table, updated every 2 seconds from the AMS2 shared memory API (server mode only) |

### Driver Stats columns

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

## Development

### VS Code

A `.vscode/launch.json` is included with two launch configurations selectable from the Run & Debug panel (Ctrl+Shift+D):

- **ams2_championship (generate HTML)** — builds and runs the file generator (F5)
- **ams2_championship_server (serve on :8080)** — builds and starts the HTTP server

Press **Ctrl+Shift+B** to pick a build task (build / test / clippy / fmt).

### Running tests

```bash
cargo test
```

**Unit tests** cover the parsing helpers, stat computation logic (including DNF attribution, AI name merging, session filtering), and HTML generation.
**Integration tests** run the full `convert` pipeline against a minimal fixture XML and assert on the generated HTML.

### Project structure

```
src/
  lib.rs                       # Library entry point, re-exports public functions
  main.rs                      # Binary: generate HTML file
  championship_html.rs         # XML parsing, stat computation, and HTML generation
  ams2_shared_memory.rs        # AMS2 shared memory reader (Windows, $pcars2$ API)
  assets/
    style.css                  # Embedded at compile time via include_str!
    script.js                  # Embedded at compile time via include_str!
  bin/
    ams2_championship_server.rs  # Binary: HTTP server + /live JSON endpoint
tests/
  integration_test.rs          # End-to-end tests against a fixture XML
  fixtures/
    minimal.xml                # Minimal two-round championship fixture
```

## Dependencies

| Crate | Purpose |
|---|---|
| [`quick-xml`](https://crates.io/crates/quick-xml) | XML deserialisation via serde |
| [`serde`](https://crates.io/crates/serde) | Derive macros for XML and JSON serialisation |
| [`serde_json`](https://crates.io/crates/serde_json) | JSON serialisation for the `/live` endpoint |
| [`ureq`](https://crates.io/crates/ureq) | HTTP requests to the Wikipedia REST API |
| [`windows-sys`](https://crates.io/crates/windows-sys) | Windows shared memory API (`OpenFileMappingW`, `MapViewOfFile`) — Windows target only |
