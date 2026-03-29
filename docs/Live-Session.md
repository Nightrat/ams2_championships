# Live Session

The **Live Session** tab shows real-time timing data pulled from AMS2 shared memory while you are in a session. It updates automatically every 100 ms via a WebSocket connection — no manual refresh needed.

## Status indicator

The coloured dot in the top-left shows the connection state:

| Indicator | Meaning |
|---|---|
| Red dot — *Not connected* | The server is running but AMS2 is not open, or you are on the main menu |
| Green dot — *Connected* | AMS2 is in an active session and data is streaming |

The session type, race state, and track name are shown next to the status dot.

## Timing table

| Column | Description |
|---|---|
| **Pos** | Current race/session position |
| **Driver** | Participant name |
| **Lap** | Current lap number |
| **Gap** | Time delta to the overall session fastest lap |
| **S1 / S2 / S3** | Sector times. Shows the current lap's sector when the driver is in that sector, otherwise their personal best. **Purple** = overall fastest; **green** = driver's personal best |
| **Best Lap** | Driver's fastest lap of the session |
| **Last Lap** | Most recently completed lap time |
| **Top km/h** | Highest recorded speed this session |

Click any column header to sort by that column.

## Track map

The canvas in the top-left of the timing panel draws the track layout and live car positions:

- **Yellow dot** — your car
- **Red dots** — other participants
- The layout is built from position data collected during the session and saved automatically once enough coverage is accumulated. On subsequent sessions at the same track it loads instantly.

## Telemetry panel

Click the **Telemetry** sub-tab to switch from the timing table to the per-driver telemetry panel. It shows:

- Tyre temperatures and wear per corner
- Brake temperatures
- Setup recommendations based on the current data
