# AMS2 Championships

A command-line tool that converts your Automobilista 2 career championship save data into a single self-contained HTML report.

## Features

- **Championship overview** — standings table, round-by-round results grid, and event detail for every championship in your save
- **Driver statistics** — aggregated stats across all championships: races, wins, top-3/10 finishes, DNFs, championship podiums, and average finishing position
- **DNF tracking** — races where the session was not completed (player retired early) are counted and shown in a dedicated column
- **Driver portraits** — automatically fetched from Wikipedia for real-world driver names
- **Dark theme UI** — sortable stats table, tab switching between Championships and Driver Stats views, progress bars, and badge indicators

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable, 2021 edition)
- AMS2 monitored by [Second Monitor](https://gitlab.com/raceengineer1/second-monitor), which produces the `Championships.xml` save file

## Build

```bash
cargo build --release
```

## Usage

```bash
cargo run --release -- <path/to/Championships.xml>
```

This generates `championships.html` in the current working directory. Open it in any browser.

**Example** (default Second Monitor path on Windows):

```bash
cargo run --release -- "%USERPROFILE%\OneDrive\Documents\SecondMonitor\Championships\Championships.xml"
```

## Output

The generated HTML file is fully self-contained (no external assets). It contains two tabs:

| Tab | Content |
|---|---|
| **Championships** | One section per championship with standings, a round-by-round results grid, and expandable event details |
| **Driver Stats** | A sortable table aggregating stats for every driver across all championships |

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

## Development

### VS Code

A `.vscode/launch.json` is included. Press **F5** to build and run with the default save path. Press **Ctrl+Shift+B** to pick a build task (build / test / clippy / fmt).

### Running tests

```bash
cargo test
```

**Unit tests** cover the parsing helpers, stat computation logic (including DNF attribution, AI name merging, session filtering), and HTML generation.
**Integration tests** run the full `convert` pipeline against a minimal fixture XML and assert on the generated HTML.

### Project structure

```
src/
  lib.rs                  # Library entry point, re-exports `convert`
  main.rs                 # Binary entry point
  championship_html.rs    # All parsing, stat computation, and HTML generation
tests/
  integration_test.rs     # End-to-end tests against a fixture XML
  fixtures/
    minimal.xml           # Minimal two-round championship fixture
```

## Dependencies

| Crate | Purpose |
|---|---|
| [`roxmltree`](https://crates.io/crates/roxmltree) | Read-only XML parsing |
| [`ureq`](https://crates.io/crates/ureq) | HTTP requests to the Wikipedia REST API |
| [`serde_json`](https://crates.io/crates/serde_json) | Parsing Wikipedia API JSON responses |
