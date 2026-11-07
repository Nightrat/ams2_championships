# Career Tab

The **Career** tab shows your championship standings and aggregated statistics across all recorded sessions. It has five sub-tabs: **Championships**, **Paddock**, **Driver Stats**, **Track Stats** and **Finances**.

Finances is a singleplayer feature and is not shown in a multiplayer career. The Paddock is shown in both, but a multiplayer career has no teammate card.

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

A session raced on something other than the season's own grid carries a **⚠ grid** badge and a line saying what was different — a short grid, stock AI filling it out, or a roster that was not used at all. The flag stays with the result, so a season's history shows which races were run on the grid they are judged against. See [Set the grid size to the roster](Getting-Started.md#set-the-grid-size-to-the-roster).

### Lap charts

Each recorded race carries a **Lap Chart** — every driver's position at the end of every lap. It sits collapsed under the round's results; click it to open, and the chart is fetched at that moment rather than downloaded with the rest of the page. Practice sessions have none, which is not an error.

## Paddock sub-tab

The story of the season you are racing. Between seasons, until you start the next one, it shows the season you last finished instead: its review, your career and its news.

### Title fight

The **Title fight** card says where you stand: for example "8 of 12 races run — 14 points behind Nelson Piquet". The same line appears on the Live Session tab for the season you are racing.

- **The table** lists every driver who can still win the championship, with their points and how far behind the leader they are. Drivers who can no longer win it are counted below the table instead of listed. You are always shown.
- **Who can still win:** a driver is out once they can't finish ahead of the leader even by winning every race left while the leader scores nothing. A tie on points is settled by the countback (most wins, then most second places, and so on), the same rule as the standings. A tie that the countback can't separate keeps both drivers in the fight. The app never calls a title won while a tie could still go the other way.
- **Clinching at the next round:** when the leader can settle the title at the next round, the card says what it takes against each rival, for example "by outscoring Alain Prost by 5 points".
- **Your contract's target:** with a contract, the card also shows the championship position the team wants and how you stand against it, for example "You are 5th, 6 points behind Riccardo Patrese".
- **Needs a race count:** the title fight works from the number of **Races** set for the season. Without one, the card asks you to set it on the Manage tab. Each race left counts as worth a win's points.
- **Past the plan:** once all the planned races are run, the card reminds you to finish the season. You can also race past the plan.

The news feed announces the round a title is won and the round you can no longer win it.

### Team mood

The **Team mood** card shows how your team rates you, from 0 to 100, with a word for it: Delighted, Content, Concerned or Losing patience. It is a singleplayer feature. The card also says why the mood last moved, and draws a bar for every race so you can see how it has gone.

- **Every new season with a team starts at 60.** Between races the mood eases back toward that a little, so one great or awful month doesn't decide the whole season.
- **What lifts it:** meeting the weekend target, finishing ahead of where your car is expected to, beating your teammate, and being on course for your contract's objective.
- **What lowers it:** the opposite of each of those. A retirement also costs a little, but only a little, since the app can't tell your mistake from a broken engine.
- **The car counts most:** finishing ahead of your car's expected place is what a team wants most. That's why a driver dragging a slow car into the points is rated highly even without a teammate to beat.

The news feed announces when the mood crosses into another band, for example "Osella is concerned about Nightrat's form after Monza".

The mood the team ends a season with decides close calls at renewal and can add to the pay. See [Renewals](Contracts-and-Money.md#renewals). It never affects your rating. If the team loses patience for three races running, the card warns that it is considering its options. That's a warning only: your contract runs to the end of the season.

### Interest from other teams

From two thirds of the way through the season, this card lists the teams **on this season's grid** that would pay to have you today, and marks the ones with a quicker car as "a step up". Your own team isn't listed, nor are teams that would only sell you a seat.

It shows this season's grid because next season's isn't known yet. The real offers for next season are worked out from next season's roster when you create that season.

### Weekend targets

Your team sets a target for every race. The **Weekend targets** card shows the one for the coming round, how many you have met so far, and how each raced round went (✓ met, ✗ missed, – could not be judged). It is a singleplayer feature.

- **Finish in the top N:** N is where your car is expected to finish, plus two places of room, so you meet it by doing the car justice. It is never asked of a car expected near the back, where it would just mean finishing.
- **Finish ahead of your teammate:** asked whenever your team runs another car. If your teammate didn't start, the target can't be judged.
- **Bring the car home:** asked only in the race after a retirement.

The team picks one of these per race. The choice varies, but it is fixed for the career and never changes when you reload. The coming round's target also shows on the Live Session tab.

Targets don't affect money or your rating.

### Rival

The **Rival** card names the driver the season keeps putting next to you. You don't pick a rival; the results do.

- **Who becomes your rival:** after each race, drivers who finished close to you score for it, and closer finishes score more. Being close to you in the championship also counts. Recent races count more than early ones.
- **When it's decided:** nobody is named until two races are run. After that, a new rival takes over only once they have been clearly ahead for two races in a row, so the name doesn't change with every result.
- **Who can't be your rival:** your teammate, who has a card of their own.
- **What the card shows:** since which round they've been your rival, how many races you've finished ahead of each other, and the points between you in the championship.

The news feed announces a new rival, and the moment your head-to-head with them changes hands.

### Your teammate

The **Teammate** card measures you against the other car of your team, the one comparison the car cannot explain away. For each teammate it shows who was ahead in qualifying and in races, the points you each scored, your average finishes and your retirements. The figure that is ahead is shown in green.

- Your teammate is a driver who holds a seat in your team in its roster. The driver you replaced is not your teammate, since you are in their car. A per-track stand-in isn't one either, since they are only there for one weekend. If a seat is shared by two drivers, each is a teammate.
- Only sessions you both drove count. A race where both of you retired counts for nobody, and a classified finish always beats a retirement.
- Without a team or a roster, the card says so instead.

### Paddock news

The season told back as headlines, newest round first. Headlines about you are in bold.

- **About you:** wins, podiums, poles and points, marked as a first when they are the first of your career. Also your debut and your first race for a new team, your best result of the season, streaks of podiums or points finishes, and your first retirement of the season.
- **About the field:** a new championship leader, a driver's first win of the season, a winner who still trails the leader, and a driver finishing far above where their car is expected to. That last one needs the season's roster.
- A round shows at most four headlines, and at most two about the same driver, so the biggest news comes first.
- A tie on points is not a lead. The feed only announces a leader who is ahead on points.
- Every headline is worked out from the results each time you look. Move a session to another round or season and the news changes with it. All wording comes from fixed sentences in the app; nothing is generated by AI.

### Your career

At the foot of the Paddock, one line per season you have raced, in the order you raced them: your team, where you finished (or *Champion*), your points and wins. Below it are your career firsts: first race, first points, first podium, first win, first pole and first title, each with where it happened.

### Season review

Once you mark a season finished, it gets a review. You can see it on the Championships sub-tab above the standings, and on the Paddock until your next season starts. It says who won, where you finished, whether you met the team's target, your best and hardest weekends, how you did against your teammate and your rival, how the team felt at the end, and the season's biggest headlines.

The **Download HTML** export includes your career at the top, and the review of each finished season.

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

There is nothing to show until you have signed for a team, and you can only be offered a seat on a season raced on a Custom AI Drivers roster whose liveries AMS2 actually has — see [What a singleplayer career needs](Getting-Started.md#what-a-singleplayer-career-needs).

Four sections:

1. **Summary** — balance, what the career was founded with, total earned and total spent.
2. **The season being raced** — salary drawn so far against the contracted figure, how much is still to race for and over how many races, the prize today's standings would pay, any sponsorship paid on signing, and what is still to come if you race the calendar out and the standings hold. Not shown when no season is under way.
3. **Season by season** — one row per contracted season: team, state, final result, target, races run, salary, prize and sponsorship.
4. **Where the money went** — wages, prizes and sponsorship totalled across the career.

Money that has not been banked yet is shown in amber and prefixed with `~`. A season still being raced has no final position, so its prize is only what today's standings *would* pay.

> The balance can go negative. Reassigning a session moves standings retroactively, which can move a payout that has already been made, and showing that as debt is more honest than hiding it.

## HTML export

Click **Download HTML** in the Career tab to save a standalone copy of your career: every championship with its standings and rounds expanded, the driver statistics table, and the track statistics table. Styling is embedded in the file, so it opens anywhere and needs neither the server nor a network connection — and prints from the browser if you want it on paper.
