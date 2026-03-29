# Data & Backup

## Where data is stored

All career data is stored in a folder called `championships\` next to the server executable:

```
ams2_championship_server.exe
championships\
  ams2_career.json        ← all sessions and championships
  track_layouts\
    silverstone.json      ← saved track map layouts (one file per track)
    le_mans.json
    ...
```

## ams2_career.json

This is a plain JSON file containing two arrays:

- **`sessions`** — every recorded race: track, timestamp, session type, and per-driver results (position, laps completed, fastest lap, last lap, DNF flag)
- **`championships`** — every championship you have created: name, status, points system, constructor scoring flag, and the rounds with their assigned session IDs

The file is updated automatically after every recorded race and after any change made in the Manage tab.

## Backing up your data

Copy the entire `championships\` folder to back up everything — sessions, championships, and track layouts.

To restore, copy it back next to the executable before starting the server.

## Moving to a new PC

1. Copy `ams2_championship_server.exe` to the new PC.
2. Copy the `championships\` folder next to it.
3. Run the server — it will load your existing data automatically.

## Track layouts

Track layout files in `championships\track_layouts\` are built automatically the first time you complete a session at a track. They are used to draw the track map in the Live Session tab.

- If a layout file is missing or has too few points it will be rebuilt during your next session at that track.
- You can delete individual `.json` files from `track_layouts\` to force a rebuild.

## Resetting everything

To start fresh, delete `championships\ams2_career.json`. Track layouts are unaffected. To also clear track layouts, delete the entire `championships\track_layouts\` folder.
