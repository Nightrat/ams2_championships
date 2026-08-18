# Data & Backup

## Where data is stored

All career data lives in one folder — `championships\` next to the server executable by default, or
any folder you pick (see [Choosing the save files folder](#choosing-the-save-files-folder)):

```
ams2_championship_server.exe
config.json                    ← server configuration
championships\
  ams2_career.json             ← a career save: its sessions and championships
  gt3_2025.json                ← another career, fully independent
  track_layouts\
    silverstone.json           ← saved track map layouts (one file per track)
    le_mans.json
    ...
```

## Choosing the save files folder

By default saves live in `championships\` next to the executable. To keep them somewhere else —
a synced folder, a data drive, or alongside another install — set **Save files folder** in the
**Config** tab (`saves_dir` in `config.json`). Leave it empty for the default.

The folder is created if it does not exist, and an unusable path is rejected when you save.
Unlike switching careers, this one takes effect **after a restart** — the running server keeps
using its current folder until then.

After restarting, the server opens a career from the new folder: `ams2_career.json` if present,
otherwise the first save it finds, otherwise a fresh empty `ams2_career.json`. Track layouts live
in a `track_layouts\` subfolder of whichever folder is in use, so moving the folder starts a new
layout cache — they rebuild automatically as you drive.

Your old folder is left untouched. To bring an existing career along, copy its `.json` file into
the new folder before restarting (or point the setting back at the old folder).

## Multiple careers

Every `*.json` file directly inside the save files folder is a **career save** — its own championships,
sessions, standings and stats. Exactly one is active at a time; recorded sessions always go into
the active save.

Switch careers with the **Career** dropdown in the header. The switch takes effect immediately —
no restart — and the page reloads onto the selected career. The active save is remembered in
`config.json` (`data_file`), so the server comes back to it after a restart.

The **Config** tab lists every save with its championship and session counts and lets you:

- **New career** — create an empty save and switch to it
- **Switch to** — make another save active
- **Duplicate** — copy a save under a new name (useful before a risky change); the active save is unchanged
- **Rename** — rename a save on disk; renaming the active one keeps it active
- **Delete** — remove a save; the active save cannot be deleted, switch away first

Track layouts are shared by all careers — they describe the track, not your results.

## config.json

`config.json` is created next to the executable on first run. It stores server settings such as the port, host, poll interval, auto-record flags, track map options, and voice spotter settings. See [Getting Started — Configuration](Getting-Started.md#configuration) for the full list of keys.

The file is updated whenever you save changes in the **Config** tab. New keys added in future versions are written automatically on the next startup, so you never need to recreate the file from scratch.

## Career save files

Each career save is a plain JSON file containing two arrays:

- **`sessions`** — every recorded session: track, timestamp, session type (practice / qualifying / race), and per-driver results (position, laps completed, fastest lap, last lap, DNF flag, car name)
- **`championships`** — every championship you have created: name, status, points system, constructor scoring flag, and the rounds with their assigned session IDs

The file is updated automatically after every recorded session and after any change made in the Manage tab.

## Backing up your data

Copy `config.json` and the entire save files folder to back up everything — sessions, championships, and track layouts.

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

To start fresh, create a new career from the **Config** tab and switch to it — your old career stays on
disk. To wipe one instead, delete its save file (e.g. `championships\ams2_career.json`); track layouts
and `config.json` are unaffected. To also clear track layouts, delete the entire
`championships\track_layouts\` folder.
