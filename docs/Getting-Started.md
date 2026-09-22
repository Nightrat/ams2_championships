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

## What a singleplayer career needs

> **A singleplayer career is built on your Custom AI Drivers rosters — and a roster only works if the liveries it names are installed.** This is one requirement, not several, but almost everything in a singleplayer career depends on it: historic team names in the live grid, your driver rating, what teams ask of you, the seats you are offered, your salary and the whole Finances page.

You need three things, and they have to agree with each other:

1. **The custom skins (livery mods) installed in AMS2** for the class you are racing.
2. **A Custom AI Drivers file** for that class, naming those liveries — `<driver livery_name="1986 Williams #5 - N. Mansell">` has to match a livery the game actually owns.
3. **The Custom AI Drivers folder set in Config** (`custom_ai_dir`, usually `…\Automobilista 2\UserData\CustomAIDrivers`), and that file chosen when you create the season.

Then race the season **with that roster active in AMS2**, and — just as important — **with the opponent count set so the grid fills the roster**. Everything the app knows about who you were racing comes from the recorded grid matching the file.

### Set the grid size to the roster

> **Opponents = the number of cars the roster can field, minus your own.** One livery is one seat, so a class with 22 installed liveries is a 22-car grid: set 21 opponents and race the field the season was designed around.

The number that matters is **cars**, not entries, and the Driver Performance tab prints both in each class heading — *"29 entries, 26 cars"*. Use the car count.

They differ for two reasons, and a season roster usually has both:

- **A driver the game cannot field.** An entry whose livery AMS2 does not own is silently ignored and never reaches a grid; those rows are tagged **no livery**.
- **More than one entry for the same car.** A roster names everyone who sat in that car across the season — a stand-in for one race, or a second livery — and they all share one seat on track. F-Vintage_Gen2 is 29 entries and 26 cars for exactly this reason: Tino Brambilla replaces Chris Amon in the Ferrari at Monza 1971, and two more entries only retune a driver at certain circuits.

Getting it wrong breaks nothing, but it quietly costs you the things the roster was for:

| Grid size | What it does |
|---|---|
| **Fewer opponents than seats** | AMS2 fields only part of the roster. Your rating is then scored against what your car should do **across the whole grid** — a car that belongs around P18 of 22 — while you actually finished inside a field of, say, ten. Those are not the same scale, so the score flatters you and the rating drifts up on nothing. Most of the roster also looks unraced, which makes it harder for the app to work out which seat was yours |
| **More opponents than seats** | AMS2 makes up the difference with its own stock AI. Those cars are not in the roster, so they show as car models in the live grid — and if they outnumber the roster cars, the session counts as not having used the roster at all and is skipped by the rating |
| **Exactly the roster** | Every team is on track, every expected finishing position means what it says, and your own seat is unambiguous |

This is the same reason a season's roster locks once it has a recorded session: the grid is the yardstick everything is measured with, so it has to stay the same grid all season.

**You will be told.** The app checks the grid against the roster and says so in three places: a banner in the Live Session tab while you are on track and can still fix it, a panel on the season in the Manage tab, and a flag on the affected races in the Career tab. Nothing is ever blocked — the session records, assigns and scores as normal; the message only says what it can and cannot be judged on.

### What happens when a piece is missing

| Missing | What you get |
|---|---|
| The livery a roster entry names | AMS2 **silently ignores that entry** — no error, the driver never appears. The app marks such rows **no livery** in the Car and Driver Performance tabs. A roster can name more drivers than the class has cars |
| The whole livery pack for a class | Nothing in the roster spawns, so the race runs on stock AMS2 AI |
| The Custom AI file (or the folder setting) | The live grid shows car models instead of team names, and the season has no grid to judge you against: no ratings, no team requirements and no offers |
| A race actually run on stock AI | That session **does not count towards your rating** — the app can see that fewer than half the grid is in the roster and skips it rather than guessing. Nothing is broken and nothing is lost; the session is still recorded and still scores championship points |
| A full grid (opponents set below the roster size) | Results are recorded and scored as normal, but the rating is measured against the wrong-sized field — see [Set the grid size to the roster](#set-the-grid-size-to-the-roster) |

A career whose races never match its roster therefore sits on the starting rating for ever: it can still sign for whatever an unproven driver is offered, and still draws that salary, but it never improves its way up the grid. That is the system working as designed rather than a fault — but it is not much of a career, so it is worth getting the three pieces lined up before the first season.

> **A multiplayer career needs none of this**, and cannot use it: it races people, so a season takes no roster and no team. The live grid shows the car models AMS2 reports, and there is nothing to rate or to sign.

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
| `class_years` | `{}` | Season year per class, overriding the built-in years — see [Season years](Rating-and-Performance.md#season-years). Only the classes you have answered for are stored |
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

1. Start AMS2 and enter a race session. In a singleplayer career, race it on the season's Custom AI Drivers roster with the opponent count set to fill the grid — see [Set the grid size to the roster](#set-the-grid-size-to-the-roster).
2. Finish the race (or let it reach the results screen).
3. The server detects the session end automatically and saves the results.
4. Switch to the browser and go to the **Manage** tab to assign the recorded session to a championship.

> Auto-recording is enabled for practice, qualifying, and race sessions by default. You can turn off individual session types in the **Config** tab, or use the **Save Session** button in the **Live Session** tab to save a session manually at any time.

> **Watching a replay is safe.** A replay refills the live timing data with a race that is already over, so the recorder freezes while one is playing and picks up exactly where it was when you leave. Manually saving a session is refused during a replay for the same reason — what is on screen is not the session.
