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
Career data:    ...\championships\ams2_career\career.json (3 championship(s), 41 session(s))
Serving at http://127.0.0.1:8080/  (Ctrl+C to stop)
```

Before you have made a career it says so instead:

```
Career data:    none yet — create a career in the app before racing
Serving at http://127.0.0.1:8080/  (Ctrl+C to stop)
```

The port, host, and other settings are configured via `config.json` or the **Config** tab in the UI. See [Configuration](#configuration) below.

## Opening the UI

Open your browser and go to:

```
http://127.0.0.1:8080/
```

Keep the server running in the background while you play AMS2. Press **Ctrl+C** in the terminal window to stop it.

## Creating your first career

The server does not invent a career for you — naming it, and choosing what kind it is, are your decisions. If the career dropdown in the header says **No careers yet**, open the **Config** tab, type a name under *Career Save Files*, pick the kind, and click **New career**.

| Kind | What it does |
|---|---|
| **Singleplayer** | Race the AI. A season is raced on a Custom AI Drivers roster, you sign for a team on the terms your driver rating has earned, and you are paid for it. One season at a time, and finishing one is final. |
| **Multiplayer** | Race people. No roster, no team, no contracts and no money — just sessions, championships and standings. As many seasons running at once as you like. |

**The kind is chosen once and kept.** The two allow different things, so switching would leave a career holding seasons it could never have created. To race the other way, make a second career — you can have as many as you like and switch between them from the header without restarting.

> A career created by an older version has no kind recorded. The switcher asks you to pick one the first time you use it; that choice is also permanent.

## Configuration

On first run `config.json` is created next to the executable with all default values. You can edit it in a text editor or use the **Config** tab in the browser UI.

### Server and recording

| Key | Default | Description |
|---|---|---|
| `port` | `8080` | HTTP and WebSocket port |
| `host` | `"127.0.0.1"` | Bind address — use `"0.0.0.0"` to allow LAN access |
| `saves_dir` | `null` | Folder holding the career saves and their track layouts; `null` uses `championships\` next to the executable — see [Data & Backup](Data-and-Backup.md#choosing-the-save-files-folder) |
| `active_career` | `null` | **Name** of the active career — set by the Career dropdown in the header, not by hand. When unset, or naming a career that is no longer there, the server picks one from `saves_dir` at startup |
| `poll_ms` | `200` | Shared memory read interval in milliseconds (live view refresh rate) |
| `record_practice` | `true` | Automatically save practice sessions |
| `record_qualify` | `true` | Automatically save qualifying sessions |
| `record_race` | `true` | Automatically save race sessions |
| `show_track_map` | `false` | Show the track radar canvas in the live timing view |
| `track_map_max_points` | `5000` | Maximum unique grid cells accumulated for the track radar |
| `custom_ai_dir` | `null` | Your AMS2 `UserData\CustomAIDrivers` folder — the rosters seasons are raced on |
| `spotter_enabled` | `false` | Enable the server-side voice spotter |
| `spotter_voice` | `null` | TTS voice name to use (`null` = system default) |
| `spotter_name` | `null` | Driver name to track in multiplayer (`null` = viewed player) |

### Career rules, rating and money

These only do anything in a singleplayer career. See [Driver Rating & Performance](Rating-and-Performance.md) and [Contracts & Money](Contracts-and-Money.md).

| Key | Default | Description |
|---|---|---|
| `enforce_team_eligibility` | `true` | Only let a championship's My Team be a team your rating has earned |
| `hide_locked_teams` | `false` | Hide teams you cannot claim instead of listing them greyed out with what they ask for |
| `starting_balance` | `5000000` | Credits a **newly created** career begins with |
| `contract_top_salary` | `4000000` | Per-season pay for the quickest car on the grid |
| `contract_floor_salary` | `100000` | Per-season pay for the slowest car |
| `contract_buy_in_per_point` | `150000` | Sponsorship asked per rating point short of a selling team's bar; `0` switches pay-driver seats off |
| `contract_pay_driver_margin` | `10` | How far clear of a back-marker's bar you must be before it pays you instead of charging you (file only) |
| `champion_prize` | `2000000` | Credits for winning a championship |
| `last_place_prize` | `50000` | Credits for finishing last among the drivers who scored |
| `starting_rating` | `50` | Where a driver with no results sits, 0–100 |
| `rating_strictness` | `0` | Rating points added to every team's requirement; negative opens the grid up |
| `offer_margin` | `10` | How far below a team's requirement you may sit and still be offered the seat on merit |
| `eligibility_gates` | `both` | Build a team's requirement from car pace, the incumbent's skill, or both |
| `rating_half_life` | `10` | Results this far back count half; `0` weighs a whole career equally |
| `count_retirements` | `true` | Whether a DNF costs a point of finish rate |
| `retirement_min_laps_down` | `3` | Laps behind the leader before a car counts as retired rather than lapped |
| `retirement_distance_pct` | `90` | Share of the leader's distance a car must fall short of to count as retired |

Settings marked with *restart required* in the Config tab — port, host, save files folder, and the auto-record flags — take effect after restarting the server. Switching careers does not need a restart.

> **The rating tuning and the starting balance belong to the career, not to the file.** Both are copied onto a career when it is created and then kept, so editing them here changes *new* careers only. Everything your rating decided — which seats were open, what a team asked, whether a contract could have been signed — would otherwise be rewritten backwards through seasons you have already raced. The Config tab has a button to move the career you are playing onto the current rating settings deliberately.

## First race

1. Start AMS2 and enter a race session.
2. Finish the race (or let it reach the results screen).
3. The server detects the session end automatically and saves the results.
4. Switch to the browser and go to the **Manage** tab to assign the recorded session to a championship.

> Auto-recording is enabled for practice, qualifying, and race sessions by default. You can turn off individual session types in the **Config** tab, or use the **Save Session** button in the **Live Session** tab to save a session manually at any time.

> **Watching a replay is safe.** A replay refills the live timing data with a race that is already over, so the recorder freezes while one is playing and picks up exactly where it was when you leave. Manually saving a session is refused during a replay for the same reason — what is on screen is not the session.
