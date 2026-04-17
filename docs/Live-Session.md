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
| **Top km/h** | Highest recorded speed this session (capped at 450 km/h to filter teleport spikes) |
| **Tyre** | Player's current tyre compound (e.g. Soft / Medium / Hard) — other drivers show — |

Click any column header to sort by that column.

## Track map

The canvas in the top-left of the timing panel draws the track layout and live car positions:

- **Yellow dot** — your car
- **Red dots** — other participants
- The layout is built from position data collected during the session and saved automatically once enough coverage is accumulated. On subsequent sessions at the same track it loads instantly.

The track map can be shown or hidden in the **Config** tab. You can also configure the maximum number of points accumulated before collection stops (`track_map_max_points`).

## Telemetry panel

Click the **Telemetry** sub-tab to switch from the timing table to the telemetry panel. It shows player-only data:

- Tyre temperatures (inner / mid / outer per corner), wear, and pressure
- Brake temperatures
- Suspension travel
- Automatic setup recommendations based on a rolling 20-sample average
