# Live Session

The **Live Session** tab shows real-time timing data pulled from AMS2 shared memory while you are in a session. It updates automatically via a WebSocket connection at the configured poll interval — no manual refresh needed.

## Status indicator

The coloured dot in the top-left shows the connection state:

| Indicator | Meaning |
|---|---|
| Red dot — *Not connected* | The server is running but AMS2 is not open, or you are on the main menu |
| Green dot — *Connected* | AMS2 is in an active session and data is streaming |

The session type, race state, and track name are shown next to the status dot.

## Save Session button

The **Save Session** button (next to the status bar) lets you manually capture the current session at any time, regardless of your auto-record settings. It is enabled whenever you are in a practice, qualifying, or race session with active participants.

Use this if you have auto-recording turned off for a session type (e.g. practice) but want to save a particular session.

> **Watching a replay?** The recorder freezes while one plays, and so does this button (saving is refused with a message). A replay refills the live timing data with a race that is already over, so what is on screen is not the session — recording it would file a mid-race picture as the result. Everything picks up where it was when you leave the replay.

## Timing table

| Column | Description |
|---|---|
| **Pos** | Current race/session position |
| **Driver** | Participant name; your car is marked with a **YOU** badge |
| **Laps** | Laps completed |
| **Interval** | Gap to the car directly ahead (race sessions only) — shown in seconds or whole laps |
| **Gap** | Time delta to the overall fastest lap set in the session |
| **S1 / S2 / S3** | Sector times. Shows the current lap's sector when available, otherwise the driver's personal best. **Purple** = overall fastest sector; **green** = driver's personal best |
| **Best Lap** | Driver's fastest lap of the session |
| **Last Lap** | Most recently completed lap time |
| **Car / Team** | Historic team name where one is known, otherwise the car model AMS2 reports — see [Team names](#team-names) |
| **Tyre** | Player's current tyre compound (e.g. Soft / Medium / Hard) — other drivers show — |

Click any column header to sort by that column.

## Team names

AMS2's shared memory has no team or livery field — every car reports only its model name (e.g. *Formula Retro Gen3*). To show real team names instead, the **Car / Team** column is resolved against the **active championship**:

1. The championship whose status is **Active** in the Manage tab is the one you are racing. There is only ever one.
2. Its assigned **Custom AI Drivers** file maps each AI driver's name to their livery, so *Niki Lauda* shows as *Brabham-Alfa Romeo*.
3. Your own row uses the championship's **My Team** setting, because your profile name is not in the roster file.

Anything the roster does not name — and your row when **My Team** is unset — falls back to the AMS2 car model.

> **Two seasons sharing one roster file?** Historic packs often reuse a single `.xml` across several seasons, so the file alone cannot say which season you are in. Only the **Active** flag can. If the live grid shows the wrong team, check that the season you are racing is the one marked Active.

The names refresh when the session moves to a new track, and immediately whenever you change anything in the Manage tab — so switching which championship is Active updates the live grid without a page reload.

If no championship is Active, or the Active one has no Custom AI file assigned, the whole column falls back to car models.

## Track map

The canvas in the top-left of the timing panel draws the track layout and live car positions:

- **Yellow dot** — your car
- **Red dots** — other participants
- The layout is built from position data collected during the session and saved automatically once enough coverage is accumulated. On subsequent sessions at the same track it loads instantly.

The track map can be shown or hidden in the **Config** tab. You can also configure the maximum number of points accumulated before collection stops (`track_map_max_points`).

## Telemetry panel

Click the **Telemetry** sub-tab to switch from the timing table to the telemetry panel. It shows player-only data:

- **Damage** — crash state, aero, engine, per-corner brake and suspension damage, and the last car you made contact with (by grid slot) with the force of it. A tap that does no damage at all still registers here
- **Every tyre field the shared memory carries**, as a card per wheel: temperatures (inner / mid / outer, plus the five structure temperatures), wear, pressure, brake temperature, suspension travel and velocity, ride height and more
- Three fields the underlying API marks as obsolete are shown dimmed at the foot of the panel rather than dropped, so you can see for yourself whether AMS2 still fills them

Values are shown exactly as they are read — no smoothing and no rolling average — so an unset or wrong reading looks wrong rather than being averaged into something plausible. A wheel with no data shows a dash, not a zero.

### Ride height over a run

The reading on a tyre card is what that corner is doing right now. Alongside it, the panel keeps the **lowest and highest each corner has been during the run** — the damage panel shows the extremes across the car, and each tyre card shows its own corner's range.

The lowest figure is the one a setup is chosen against: it says how much floor you had left. Keeping it per corner is what says *which end* to lower, since front and rear ride height are separate setup values.

The record accumulates while you are driving, whether or not the Telemetry tab is open. It starts fresh at a new track, and on a disconnect — the next run is a different car on a different setup. **Reset** clears it by hand.

### Freezing in the pits

The panel **freezes on the last on-track reading** whenever you are not out on track, so you can read your tyre, brake and damage data during a pit stop, and resumes the moment you rejoin. The label says why it is held, and keeps up as you move through the pit modes.

Two things are watched, because neither catches everything: driving into the pit lane, and being teleported to the garage by ESC → *Return to pits*, which never touches a pit lane at all.

Before you have been on track at all it says so, rather than painting zeroes that would look like an undamaged car.

## Voice spotter

The navigation bar contains controls for a server-side voice spotter that reads race events aloud using Windows text-to-speech (SAPI / .NET `SpeechSynthesizer` — no extra software required):

| Control | Description |
|---|---|
| **🎙 Spotter** button | Toggles the spotter on/off. Highlighted when active. |
| **Voice** dropdown | Selects from voices installed on your Windows PC. Blank = system default. |
| **Focus** dropdown | In multiplayer, selects which driver to track. Defaults to the viewed player. |

Settings are saved to `config.json` automatically when you change them.

### Events announced

| Event | Condition |
|---|---|
| *Position N* | Race position changes — debounced ~2 s so a pile-up collapses into one announcement |
| *Lap N* | Lap completed (race only) |
| *N.N seconds to [name]* | Gap to car ahead drops below 1.5 s |
| *Clear ahead* | Gap to car ahead rises above 5 s |
| *N.N seconds to [name] behind* | Gap to car behind drops below 1.5 s |
| *Clear behind* | Gap to car behind rises above 2 s |
| *Personal best, M:SS.s* / *Fastest lap, M:SS.s* | New personal best, or the session's overall fastest |
| *Yellow flag / Safety Car / Red flag / Green flag* | Flag state transitions |
| *Low fuel / Fuel critical, N laps remaining* | Fuel estimate falls below 5 / 2 laps |
| *[corner] tyre worn / critical* | Per-corner tyre wear exceeds 70 % / 90 % |
