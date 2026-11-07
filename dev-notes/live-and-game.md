# The running game and the Live tab

Developer notes moved out of CLAUDE.md. Read when touching shared memory, the recorder, replays, the spotter, the telemetry panel, ride height, the live frame or which championship is Active.

## Shared memory layout (`game/shared_memory.rs`)

- Always read all fields from `ptr` **before** calling `UnmapViewOfFile` — reading after unmap is an access violation.
- `ParticipantInfo` stride is 100 bytes; `mCurrentSector` (i32) at +96 is `-1` when the car is in the pit lane or garage (`in_pits` field).
- AMS2-specific fields (tyre compound, tyre temps, ride height) live at offsets above 19000 and are not in the original PCars2 header.
- **Offsets are derived by counting the header's declaration order forward from a verified anchor, then checked against a live session.** A float read at a wrong offset still returns a plausible-looking number, so neither step alone is enough. The flags/pit/car-state block (`mHighestFlagColour` 6800 → `mFuelCapacity` 6844) was previously placed after `mLastLapTimes` at 9460+ — a guess that compiles, runs, and silently reads noise: flag colour came back as 65537 and fuel as 0.0, which quietly disabled the spotter's flag and fuel calls rather than making them misfire. Counting back from `mSpeed` (6848) puts every field on a value that reads as its own unit, which is the check worth doing: oil ~98 °C, water ~68 °C, a 250 litre tank.
- **`mFuelLevel` is a fraction (0..1), not litres.** The reader multiplies it by `mFuelCapacity` so `fuel_level` matches its name and `fuel_level / fuel_capacity` means what it reads as. Without that conversion the spotter's percentage check divides a fraction by a litre count and calls fuel critical on a full tank.
- Damage lives in two runs: per-corner `mBrakeDamage` (7152) and `mSuspensionDamage` (7168) sit between `mTyreWear` and `mBrakeTempCelsius`; `mCrashState` / `mAeroDamage` / `mEngineDamage` (7280–7288) follow the five tyre-temperature arrays. `mLastOpponentCollisionIndex` (6892) and its magnitude register a tap that causes no damage at all, which none of the 0–1 values do.

## A replay freezes the recorder (`game/recorder.rs`)

- **A replay refills the live participant rows**, stepping backwards through a race that is already over. Watching one after the flag therefore rewrote the snapshot the recorder was holding, and since the usual way out of a replay is to quit the session, the *disconnect* capture filed a lap-20-of-30 picture as the result — with the standings computed off it. The lap chart went too: a falling lap count is how `accumulate_lap_chart` recognises a restart, so a replay wiped the real chart and rebuilt it from the playback.
- `is_replay(game_state)` is the whole test — `mGameState` 6 (from the pause or results screen) or 7 (loaded from the main menu). The `ams2` constants now follow the PCars2 header's declaration order, and the recorder's own observed values confirm that order: 2 while driving, 4 in the garage and on the results screen, both menus with the clock still running.
- **The freeze holds everything, `prev_session_state` included.** A session change that happens while the replay plays is acted on when the game comes back, against the same held snapshot — so watching a qualifying replay from the race lobby still writes qualifying, not the replay's rows.
- `POST /api/record-session` refuses during a replay (409) for the same reason: the frame on screen is not the session.
- The poll body lives in `RecorderState::poll`, which returns a `Taken` rather than writing — the thread owns the store and the save path, and the state machine can then be driven a poll at a time in tests, which cannot run AMS2.
- **A replay is not a disconnect and not a restart**, which is why neither existing reset point caught it: AMS2 stays connected and keeps `session_state` where it was.


## `ChampionshipStatus::Active` is a singleton

- **At most one championship is `Active`** — `PATCH /api/championships/:id` demotes any other holder to `Progress`. Despite the enum ordering, `Active` means "the season being raced right now", not "not yet started"; `Progress` means "started, but not the current one".
- It is therefore the answer to "which championship does this live session belong to". `resolve_live_teams` (`/api/live-teams`) reads the roster file and player team straight off it, and `loadManage()` opens the Manage tab on it.
- Do **not** reintroduce roster-matching heuristics here. Scoring Custom AI files by how many on-track drivers they name cannot separate two seasons of one series — historic packs reuse a single `.xml` across seasons, so the scores tie and the player's team resolves to the wrong season.
- Every championship mutation goes through `loadManage()`, which is why the live team-name refresh hangs off it — the status, roster file and player team all feed `/api/live-teams`.

## Spotter (`src/game/spotter.rs`)

- `SpotterState::update()` returns a `Vec<String>` of TTS phrases each poll; the background thread writes them line-by-line to a persistent PowerShell `SpeechSynthesizer` subprocess.
- Position announcements are **debounced** (~2 s): `pending_position` tracks the real-time position; `prev_position` is only updated (and the announcement emitted) once `pos_cooldown` reaches zero. This prevents a queue of stale "Position N" calls after a spin.
- Gap, flag, fuel, and tyre warn events use simple prev-state comparison — no debounce needed there.
- `SpotterConfig` (enabled, voice, name) is shared via `Arc<Mutex<SpotterConfig>>`; PATCH `/api/spotter` updates it and persists it back to `config.json`.

## Telemetry tab freeze behaviour

The tab shows the player car's **damage** (crash state, aero, engine, per-corner brake/suspension, last car-to-car contact) followed by **every tyre field the shared memory carries**, as a card per wheel. Values are rendered raw: no rolling average and no rolling buffer, so a wrong offset shows up as a wrong number instead of being smoothed into something plausible.

- `mTyreSlipSpeed`, `mTyreGrip` and `mTyreLateralStiffness` are marked OBSOLETE in the header ("kept for backward compatibility only"). They are read and shown dimmed at the foot of the panel rather than dropped, so whether AMS2 still fills them is answered on screen instead of being re-investigated each time someone wants slip or grip data.
- The five structure temperatures (`mTyreTreadTemp`, `mTyreLayerTemp`, `mTyreCarcassTemp`, `mTyreRimTemp`, `mTyreInternalAirTemp`) are **Kelvin** in the header while every other temperature in the struct is Celsius. `kelvin4` converts them on read and holds unset (0.0) at zero — a plain subtraction would report an unset wheel as −273 °C, which reads as a bad offset rather than as no data.
- It freezes on the last on-track reading (`dmgLastOnTrack`) whenever the car is not out on track, and resumes when it is. **Two signals are needed**, and neither is sufficient alone: `in_pits` (`mCurrentSector < 0`) flips at the pit entry, but ESC → "Return to pits" teleports the car into the garage without it ever driving down a pit lane — only `mPitMode` catches that. On track is `!in_pits && pit_mode == 0` — decided server-side by `service::on_track` and sent as the frame's `on_track`, since the ride-height record needs the same test.
- The held panel is not repainted each poll. `dmgFrozenLabel` stores the reason currently painted rather than a boolean, so the label still updates as the car moves through the pit modes, while an unchanged panel keeps any text selection the user made in it.
- With no on-track reading yet it says so, rather than painting zeroes that look like an undamaged car.

**Ride height is also kept as a record of the run**

- The instantaneous reading says what the car is doing now; the **lowest** a corner ever reached is what says how much floor was left, which is the number a setup is chosen against. The record is a per-corner min/max across the run, shown on the damage panel as "Lowest this run / highest" and per wheel as "Seen this run".
- **It is kept server-side** (`RideRecord` in `service/live_frame.rs`, one per `App`) and arrives on every frame as `ride_seen`; `telemetry.js` only displays it. One record for the app, not one per viewer: two browsers watching the same run are watching the same car, and a page reloaded mid-stint keeps the stint. It accumulates while any viewer's WebSocket is polling — `App::live_frame` is what folds a reading in.
- **Per corner, not one overall minimum.** Front and rear ride height are separate setup values, so an overall figure alone would not say which end to lower. Corners at zero are skipped — zero is unset, not a car resting on its floor, and it would otherwise always win the minimum (`rideExtremes` in the JS applies the same rule to the instantaneous reading).
- Only on-track readings count, and a new **run** starts a new record: keyed on track + variation + car, so neither Monza nor last car's setup answers for this one. Losing the game (`connected: false`) clears it; a browser reconnecting does not.
- The **Reset** button posts `POST /api/live/ride-height/reset`. It is delegated from `document` because the panel is rebuilt on every on-track poll, and it clears `dmgFrozenLabel` to force a repaint — **after** the reset lands, so the repaint cannot pick up a frame still carrying the old record. The button is most useful in the box, which is exactly when the panel is held and would not otherwise redraw.

## The live frame (`service/live_frame.rs`)

`/ws` streams a `LiveFrame` per poll: the `LiveSessionData` snapshot (flattened, so every old field is where it was) plus what is derived from it, so no front end re-derives it:

- `session_best` — fastest lap and best of each sector across the field (0 = nobody yet): the gap-to-fastest column and the purple sectors.
- `sectors` — each participant's three sector cells (`SectorCell { time, mark }`, `mark` = `plain` / `best` / `personal`), parallel to `participants`. **AMS2's current-lap sector times include partials** — `01.737` for a car that left the pits mid-sector, `9:09.152` for one that sat in the garage — which it leaves out of the driver's best. A counted sector enters the best the moment it completes, so `counted_sector` shows a current time only when it is no faster than the driver's best and no more than twice it; otherwise the best stands in. The page printed them raw before, and the short ones came out purple.
- `ride_seen`, `on_track` — above.
- `can_record` — `recorder::recordable`, the **same** test `App::record_current_session` refuses with, so the Record button is never enabled for a click the route would turn down. An active career is deliberately not part of it: the refusal explains itself, a greyed-out button would not.
- `save_error` — why the career could not be saved, while that is still so (see *Saving a career* in `dev-notes/saves-and-persistence.md`). It rides every frame rather than a route of its own, because the recorder fails with nobody asking.
- `handle_websocket` takes the frame from its caller (`FnMut() -> Option<Vec<u8>>`) and stays transport. `/live` still answers the raw snapshot.

Other things the browser used to derive and now receives: `track_stats_all` on `/api/career` (the Tracks tab's "All cars" view, built by the same `track_stats_by` as the per-car rows), `winner` on every session in `/api/sessions` and in the career view (`RecordedSession::winner`), and both championship lists already in display order (`data_store::display_order` — status, then name ignoring case).

