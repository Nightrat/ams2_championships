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
8. `main.js` — tab switching, sub-tab init

### include_str! caching gotcha

`cargo` does **not** always detect changes to `include_str!` files when only the asset file changes. If CSS/JS edits aren't appearing, touch `championship_html.rs` or run `cargo build` with `--` to force a rebuild.

### Test organisation

Test files live in `src/tests/` and are wired into their parent module with `#[path = "tests/filename.rs"]` — **not** in the top-level `tests/` directory. This gives tests access to `pub(crate)` items.

- `src/tests/data_store.rs` — unit tests for `compute_career`, standings, track stats
- `src/tests/session_recorder.rs` — unit tests for `capture()` and `should_capture()`
- `src/tests/config.rs` — unit tests for config load/create/defaults
- `src/tests/server.rs` — integration tests for HTTP routes via real TCP loopback (`TcpListener::bind("127.0.0.1:0")`)

### Data files (same directory as ams2_career.json)

| File | Purpose |
|---|---|
| `ams2_career.json` | All sessions and championships |
| `track_layouts/` | Per-track radar point arrays (`{slug}.json`) |

### HTTP server notes

- All routes are matched in sequence inside a single `handle()` function — add new routes before the catch-all HTML fallback at the bottom
- `data_path` is `Arc<PathBuf>`
- WebSocket (`/ws`) streams `LiveSessionData` JSON at configurable poll interval
- CI runs on `windows-latest` only (shared memory code is Windows-specific)

### Shared memory layout (`ams2_shared_memory.rs`)

- Always read all fields from `ptr` **before** calling `UnmapViewOfFile` — reading after unmap is an access violation.
- `ParticipantInfo` stride is 100 bytes; `mCurrentSector` (i32) at +96 is `-1` when the car is in the pit lane or garage (`in_pits` field).
- AMS2-specific fields (tyre compound, tyre temps, ride height) live at offsets above 19000 and are not in the original PCars2 header.

### Spotter (`src/spotter.rs`)

- `SpotterState::update()` returns a `Vec<String>` of TTS phrases each poll; the background thread writes them line-by-line to a persistent PowerShell `SpeechSynthesizer` subprocess.
- Position announcements are **debounced** (~2 s): `pending_position` tracks the real-time position; `prev_position` is only updated (and the announcement emitted) once `pos_cooldown` reaches zero. This prevents a queue of stale "Position N" calls after a spin.
- Gap, flag, fuel, and tyre warn events use simple prev-state comparison — no debounce needed there.
- `SpotterConfig` (enabled, voice, name) is shared via `Arc<Mutex<SpotterConfig>>`; PATCH `/api/spotter` updates it and persists it back to `config.json`.

### Telemetry tab freeze behaviour

`telemetry.js` only pushes samples into `telBuf` when `viewed.in_pits === false`. When the player enters the pit lane/garage the panel freezes on the last on-track data. The buffer clears only on WebSocket disconnect.
