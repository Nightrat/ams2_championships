# Managing Championships

The **Manage** tab is where you create championships (seasons) and organise your recorded sessions into them.

What it offers depends on the kind of career you are in. A **singleplayer** season is raced on a Custom AI Drivers roster, for a team you sign with; a **multiplayer** season has neither, so those controls are simply not there. See [Getting Started](Getting-Started.md#creating-your-first-career).

## Creating a championship

1. Click **+ New** in the Championships panel on the left.
2. Enter a name.
3. Choose a points system from the dropdown:
   - **F1 Modern** — 25-18-15-12-10-8-6-4-2-1
   - **F1 1991–2002** — 10-6-4-3-2-1
   - **F1 Classic** — 9-6-4-3-2-1
   - **Custom** — type your own comma-separated values (e.g. `15,12,10,8,6,4,2,1`)
4. **Singleplayer only:** choose the **Custom AI Drivers** roster the season is raced on, and how many **Races** the season runs.
5. Optionally tick **Constructor Scoring** to enable a constructor standings table.
6. Click **Create**.

### Why singleplayer asks for a roster and a race count up front

A season is defined by the grid it is raced on, so the roster is picked when the season is created rather than attached afterwards — everything that judges you, from team requirements to the terms you are offered, is measured against that grid. It locks once the season has its first recorded session.

The race count is the season's **calendar**. Rounds are added as you race them, so without a declared length there is no way to tell whether you are an eighth or a fifteenth of the way through a season — and that is exactly what a salary has to be paid out against. It is a plan, not a commitment: you can stop short of it or run past it, and you can change it later.

A multiplayer career has no roster, no team and no contracts, so it is asked for none of this.

## Adding rounds and sessions

1. Select a championship from the left panel.
2. Click **Add Round** to create a new round.
3. Click **Add Session** on a round to open the session picker.
4. Available recorded sessions are listed with track, date, and session type. Click one to assign it to the round.

A round can contain multiple sessions (e.g. a qualifying session and a race). Only race sessions contribute points to the standings — and only race sessions draw a salary instalment.

In a singleplayer season that has both a roster and a team, a session that could not have been yours is listed with the reason — for example *"you declared Brabham but the only free seat was Osella"*. The check reads the recorded grid against the roster: if every seat in the roster was filled by an AI, or the only free one was not your team's, you were not driving for that team in that session.

## Editing a championship

With a championship selected:

- **Rename** — hover over the championship name in the left panel; a pencil icon (✎) appears. Click it to edit the name inline. Press **Enter** to save or **Escape** to cancel.
- **Change points system** — type new comma-separated values in the detail panel on the right
- **Change the race count** — the **Races** field. Unlike the roster and the seat this is *not* locked once the season has started: the salary is capped and never topped up, so resizing only changes the instalments still to come
- **Change status** — see [Status](#status) below
- **Custom AI Drivers** and **My Team** — both lock once the season has its first recorded session. In a singleplayer career the seat comes from signing a contract, not from this picker

## Status

| Status | Meaning |
|---|---|
| **Active** | The season you are racing right now. **Only one championship can be Active** — marking another one Active demotes the previous holder to Progress automatically. |
| **Progress** | Started, but not the one currently being raced. |
| **Final** | Finished. Only Final championships count toward championship standings finishes in the Driver Stats, and prize money is paid when a season reaches this. |

**Active** is not just a label — it is how the app knows which season a live session belongs to. The [Live Session](Live-Session.md#team-names) tab reads its team names from the Active championship's Custom AI Drivers file and **My Team** setting. If you run several seasons of the same series off one roster file, the Active flag is the only thing that tells them apart, so set it before you go on track.

The Manage tab also opens on the Active championship when you load the page.

### Status in a singleplayer career

A singleplayer career races one season at a time, so "started, but not the current one" has nothing to describe. Instead of the three-way picker each season shows a single **Finish** button, and a finished one reads *Finished*.

- A new season is **Active** the moment it is created.
- You cannot create a new season while an existing one is unfinished.
- **Finishing is permanent.** It pays the season out, and the next season is created on the strength of it being over, so it cannot be reopened.

Only you know when a season is over — you may stop a calendar short or run past it — so this is the one state decision the app leaves entirely to you. It is also why finishing early is not worth money: the salary is paid per race, capped at the contracted figure, so pressing Finish never pays for races you did not run.

## Signing for a team (singleplayer)

Below the season's settings, a singleplayer career shows the seats the grid is offering you for that season, with the terms and the reason for them. Sign from there; the team you sign with becomes the season's **My Team**.

Offers are re-derived every time you look, from your rating and the state of the career — they are never stored, so improving and looking again is the intended way to use them. A contract, once signed, is history and never changes.

**Tear up** undoes a mis-click: it deletes the contract and clears the seat that came with it, and is only possible before the season has a recorded session.

See [Contracts & Money](Contracts-and-Money.md) for what the terms mean.

## Removing sessions and rounds

- Click the **×** next to a session to remove it from the round (the session is not deleted, just unassigned).
- Click **Remove Round** to remove an entire round and all its session assignments.

## Deleting a championship

Click **Delete** at the top of the championship detail panel. This removes the championship and all its round assignments. Recorded sessions are not deleted.

## Deleting unassigned sessions

At the bottom of the Manage tab, the **Delete unassigned sessions** button removes all recorded sessions that have not been assigned to any championship round, along with their lap charts. Use this to clean up test sessions or accidental recordings.

> This action is permanent. Sessions deleted this way cannot be recovered.
