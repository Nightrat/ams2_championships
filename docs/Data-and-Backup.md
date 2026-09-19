# Data & Backup

## Where data is stored

All career data lives in one folder — `championships\` next to the server executable by default, or
any folder you pick (see [Choosing the save files folder](#choosing-the-save-files-folder)):

```
ams2_championship_server.exe
config.json                      <- server configuration
championships\
  ams2_career\                   <- a career: the folder name is the career name
    career.json                    its sessions, championships and contracts
    laps\
      1728394855.json              one lap chart per recorded race
  gt3_2025\                      <- another career, fully independent
    career.json
  track_layouts\                 <- saved track maps, shared by every career
    silverstone.json
    le_mans.json
```

**A career is a folder holding a `career.json`.** That is also how the app tells a career from
anything else in the folder: `track_layouts\` has no `career.json`, so it is never listed as a
career. Renaming a career renames one folder, and everything inside travels with it.

Lap charts are kept next to the career rather than inside it. They are by far the largest thing a
session records and nothing else reads them, so keeping them separate means recording a race
rewrites one small file instead of the whole career.

> **Saves from an older version** are a single `<name>.json` file sitting directly in the saves
> folder. They still work exactly as they did and are never converted — their lap charts stay
> inside the file. Only new careers are folders.

## Choosing the save files folder

By default saves live in `championships\` next to the executable. To keep them somewhere else —
a data drive, or alongside another install — set **Save files folder** in the **Config** tab
(`saves_dir` in `config.json`). Leave it empty for the default.

The folder is created if it does not exist, and an unusable path is rejected when you save.
Unlike switching careers, this one takes effect **after a restart** — the running server keeps
using its current folder until then.

After restarting, the server opens a career from the new folder: the one you were last on if it is
there, otherwise `ams2_career`, otherwise the first save it finds. If the folder is empty you get
no career at all until you create one — naming a career and choosing its kind are your decisions,
not something to be invented for you.

Your old folder is left untouched. To bring an existing career along, copy its folder into the new
one before restarting (or point the setting back at the old folder).

> **Avoid a folder that Google Drive or OneDrive syncs.** Sync clients hold on to folders they are
> uploading, which can stop a deleted career's now-empty folder from being removed until the app is
> next started. Nothing is lost when this happens — the career is gone from the switcher immediately
> and the empty folder is cleared on the next launch — but a local folder avoids the whole business.

## Multiple careers

Every career is separate: its own championships, sessions, standings and stats, its own kind
(singleplayer or multiplayer), and its own money. Exactly one is active at a time, and recorded
sessions always go into the active one.

Switch careers with the **Career** dropdown in the header. The switch takes effect immediately —
no restart — and the page reloads onto the selected career. The active career is remembered in
`config.json` (`active_career`, by name), so the server comes back to it after a restart.

The **Config** tab lists every career with its championship and session counts and lets you:

- **New career** — create an empty career, choose singleplayer or multiplayer, and switch to it
- **Switch to** — make another career active
- **Duplicate** — copy a career under a new name (useful before a risky change); the active career is unchanged
- **Rename** — rename a career on disk; renaming the active one keeps it active
- **Delete** — remove a career and everything in its folder; the active career cannot be deleted, switch away first

A career whose file cannot be read is listed with the reason and cannot be switched to. The app
never writes over a career it could not read, so a damaged file stays exactly as it is until you
fix or replace it.

Track layouts are shared by all careers — they describe the track, not your results.

## config.json

`config.json` is created next to the executable on first run. It stores server settings such as the
port, host, poll interval, auto-record flags, track map options, voice spotter settings, and the
rating and contract tuning new careers are created with. See
[Getting Started — Configuration](Getting-Started.md#configuration) for the full list of keys.

The file is updated whenever you save changes in the **Config** tab. New keys added in future
versions are written automatically on the next startup, so you never need to recreate the file from
scratch.

## What a career file contains

`career.json` holds:

- **`mode`** — singleplayer or multiplayer, chosen when the career was created
- **`sessions`** — every recorded session: track, timestamp, session type (practice / qualifying / race), and per-driver results (position, laps completed, fastest lap, last lap, DNF flag, car name)
- **`championships`** — every championship you have created: name, status, points system, constructor scoring flag, planned number of races, roster file and team, and the rounds with their assigned session IDs
- **`contracts`** — the deal you signed for each season, and what it paid out once the season was finished
- **`starting_balance`** and the **rating tuning** the career was created with

The file is updated automatically after every recorded session and after any change made in the
Manage tab.

## Backing up your data

Copy `config.json` and the entire save files folder to back up everything — sessions,
championships, contracts, lap charts and track layouts. To restore, copy them back next to the
executable before starting the server.

## Moving to a new PC

1. Copy `ams2_championship_server.exe` to the new PC.
2. Copy `config.json` and the `championships\` folder next to it.
3. Run the server — it will load your existing data automatically.

## Track layouts

Track layout files in `championships\track_layouts\` are built automatically the first time you
complete a session at a track. They are used to draw the track map in the Live Session tab.

- If a layout file is missing or has too few points it will be rebuilt during your next session at that track.
- You can delete individual `.json` files from `track_layouts\` to force a rebuild.
- The maximum number of points collected per track is configurable (`track_map_max_points` in the Config tab).

## Resetting everything

To start fresh, create a new career from the **Config** tab and switch to it — your old career stays
on disk. To wipe one instead, delete it from the same list, or delete its folder. Track layouts and
`config.json` are unaffected either way; to also clear the track maps, delete the whole
`championships\track_layouts\` folder.

There is also **Delete unassigned sessions** at the bottom of the Manage tab, which clears out
recorded sessions that were never assigned to a round, along with their lap charts.
