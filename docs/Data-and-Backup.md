# Data & Backup

## Where data is stored

All career data is stored in a folder called `championships\` next to the server executable by default:

```
ams2_championship_server.exe
config.json                    ← server configuration
championships\
  ams2_career.json             ← all sessions and championships
  track_layouts\
    silverstone.json           ← saved track map layouts (one file per track)
    le_mans.json
    ...
```

The career data path can be changed in `config.json` or the **Config** tab (`data_file` setting). When you change the path in the UI you are prompted whether to move the existing file to the new location.

## config.json

`config.json` is created next to the executable on first run. It stores server settings such as the port, host, poll interval, auto-record flags, and track map options. See [Getting Started — Configuration](Getting-Started.md#configuration) for the full list of keys.

The file is updated whenever you save changes in the **Config** tab. New keys added in future versions are written automatically on the next startup, so you never need to recreate the file from scratch.

## ams2_career.json

This is a plain JSON file containing two arrays:

- **`sessions`** — every recorded session: track, timestamp, session type (practice / qualifying / race), and per-driver results (position, laps completed, fastest lap, last lap, DNF flag, car name)
- **`championships`** — every championship you have created: name, status, points system, constructor scoring flag, and the rounds with their assigned session IDs

The file is updated automatically after every recorded session and after any change made in the Manage tab.

## Backing up your data

Copy `config.json` and the entire `championships\` folder to back up everything — sessions, championships, and track layouts.

To restore, copy them back next to the executable before starting the server.

## Moving to a new PC

1. Copy `ams2_championship_server.exe` to the new PC.
2. Copy `config.json` and the `championships\` folder next to it.
3. Run the server — it will load your existing data automatically.

## Track layouts

Track layout files in `championships\track_layouts\` are built automatically the first time you complete a session at a track. They are used to draw the track map in the Live Session tab.

- If a layout file is missing or has too few points it will be rebuilt during your next session at that track.
- You can delete individual `.json` files from `track_layouts\` to force a rebuild.
- The maximum number of points collected per track is configurable (`track_map_max_points` in the Config tab).

## Resetting everything

To start fresh, delete `championships\ams2_career.json`. Track layouts and `config.json` are unaffected. To also clear track layouts, delete the entire `championships\track_layouts\` folder.
