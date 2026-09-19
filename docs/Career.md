# Career Tab

The **Career** tab shows your championship standings and aggregated statistics across all recorded sessions. It has four sub-tabs: **Championships**, **Driver Stats**, **Track Stats** and **Finances**.

Finances is a singleplayer feature and is not shown in a multiplayer career.

## Championships sub-tab

The left panel lists all championships. Selecting one opens the detail panel on the right, which shows:

- **Status badge** — Active, Progress, or Final
- **Points system** — the first few values of the configured points system
- **Driver Standings** — drivers ranked by points, ties broken by countback (see below)
- **Constructor Standings** — shown when constructor scoring is enabled for the championship
- **Rounds** — each round is expandable and shows the sessions it contains, with full result tables

### How ties are broken

Drivers level on points are separated the way the FIA does it: **most wins first, then most second places, then most thirds**, and so on down the order until one of them is ahead. A retirement is not a finishing place, so it never counts towards this — a driver classified first who retired has no win.

If two entries are still exactly level after all of that, they are ordered by name. That is not a sporting rule; the regulations would send a genuine dead heat to the stewards, which is not something the app can do. Ordering by name at least keeps the table from reshuffling itself every time you load the page.

### Round results

Each session within a round shows:
- Session type (Race, Qualify, Practice)
- Track name and date recorded
- Number of drivers
- Full finishing order with fastest lap times
- Points awarded per driver (race sessions only)
- DNF indicators (race sessions only)

### Lap charts

Each recorded race carries a **Lap Chart** — every driver's position at the end of every lap. It sits collapsed under the round's results; click it to open, and the chart is fetched at that moment rather than downloaded with the rest of the page. Practice sessions have none, which is not an error.

## Driver Stats sub-tab

An aggregated stats table across all championships. Click any column header to sort.

| Column | Description |
|---|---|
| **Driver** | Driver name |
| **Races** | Total race starts |
| **1st / 2nd / 3rd** | Race podium finishes by position |
| **Top 10** | Top-10 race finishes |
| **Avg Pos** | Average finishing position across all races |
| **DNF** | Did-not-finish races |
| **Q Pole / Q 2nd / Q 3rd** | Qualifying pole and front-row positions |
| **Q Top 10** | Qualifying top-10 results |
| **C 1st / C 2nd / C 3rd** | Championship final standings finishes (Final championships only) |

## Track Stats sub-tab

A per-track summary across all recorded sessions. Click any column header to sort.

| Column | Description |
|---|---|
| **Track** | Track name and layout variant |
| **Races** | Number of recorded race sessions at this track |
| **Qualifyings** | Number of recorded qualifying sessions at this track |
| **Best Lap / 2nd Lap / 3rd Lap** | The three fastest laps recorded at this track, each with the driver who set it and the car they set it in |
| **Last Visited** | Date of the most recent session at this track |

## Finances sub-tab

What the career is worth and how it got there. Everything on this page is derived from your results and your contracts on each visit — there is no separate set of books to go out of date. See [Contracts & Money](Contracts-and-Money.md) for how the terms are decided.

Four sections:

1. **Summary** — balance, what the career was founded with, total earned and total spent.
2. **The season being raced** — salary drawn so far against the contracted figure, how much is still to race for and over how many races, the prize today's standings would pay, any sponsorship paid on signing, and what is still to come if you race the calendar out and the standings hold. Not shown when no season is under way.
3. **Season by season** — one row per contracted season: team, state, final result, target, races run, salary, prize and sponsorship.
4. **Where the money went** — wages, prizes and sponsorship totalled across the career.

Money that has not been banked yet is shown in amber and prefixed with `~`. A season still being raced has no final position, so its prize is only what today's standings *would* pay.

> The balance can go negative. Reassigning a session moves standings retroactively, which can move a payout that has already been made, and showing that as debt is more honest than hiding it.

## HTML export

Click **Download HTML** in the Career tab to save a standalone copy of your career: every championship with its standings and rounds expanded, the driver statistics table, and the track statistics table. Styling is embedded in the file, so it opens anywhere and needs neither the server nor a network connection — and prints from the browser if you want it on paper.
