# Getting Started

## Installation

1. Download `ams2_championship_server.exe` from the [latest release](https://github.com/Nightrat/ams2_championships/releases/latest).
2. Place it in any folder on your PC — for example `C:\AMS2Championships\`.
3. Run it. A `championships\` subfolder and a `config.json` file are created automatically next to the executable on first launch.

> You do not need to install anything else. The server is a single self-contained executable.

## Starting the server

Double-click `ams2_championship_server.exe`, or run it from a terminal:

```
ams2_championship_server.exe
```

You will see output like:

```
Career data:    ...\championships\ams2_career.json (0 championship(s), 0 session(s))
Serving at http://127.0.0.1:8080/  (Ctrl+C to stop)
```

The port, host, and other settings are configured via `config.json` or the **Config** tab in the UI. See [Configuration](#configuration) below.

## Opening the UI

Open your browser and go to:

```
http://127.0.0.1:8080/
```

Keep the server running in the background while you play AMS2. Press **Ctrl+C** in the terminal window to stop it.

## Configuration

On first run `config.json` is created next to the executable with all default values. You can edit it in a text editor or use the **Config** tab in the browser UI.

| Key | Default | Description |
|---|---|---|
| `port` | `8080` | HTTP and WebSocket port |
| `host` | `"127.0.0.1"` | Bind address — use `"0.0.0.0"` to allow LAN access |
| `data_file` | `null` | Full path to the career JSON file; `null` uses `championships\ams2_career.json` next to the executable |
| `poll_ms` | `200` | Shared memory read interval in milliseconds (live view refresh rate) |
| `record_practice` | `true` | Automatically save practice sessions |
| `record_qualify` | `true` | Automatically save qualifying sessions |
| `record_race` | `true` | Automatically save race sessions |
| `show_track_map` | `true` | Show the track radar canvas in the live timing view |
| `track_map_max_points` | `5000` | Maximum unique grid cells accumulated for the track radar |

Settings marked with *restart required* in the Config tab (port, host, data_file, and the record_* flags) take effect after restarting the server.

## First race

1. Start AMS2 and enter a race session.
2. Finish the race (or let it reach the results screen).
3. The server detects the session end automatically and saves the results.
4. Switch to the browser and go to the **Manage** tab to assign the recorded session to a championship.

> Auto-recording is enabled for practice, qualifying, and race sessions by default. You can turn off individual session types in the **Config** tab or use the **Save Session** button in the **Live Session** tab to save a session manually at any time.
