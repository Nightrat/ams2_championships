# Contracts & Money

*Singleplayer careers only. A multiplayer career races people, so it has no seats to be offered and no money; the Finances page and all of this are hidden there.*

> **The grid the terms come from is the Custom AI Drivers roster**, so contracts need the same three things the rating does: the class's **custom liveries installed**, a **Custom AI file naming them**, and the season raced **with that roster active**. Without a roster the season shows *“Assign a Custom AI Drivers file to see what the grid will offer”* and there is nothing to sign; with one that the races were not actually run on, your rating never moves, so the offers never improve. See [What a singleplayer career needs](Getting-Started.md#what-a-singleplayer-career-needs).

Every season in a singleplayer career is raced for a team, on terms the grid decides. You are offered seats your [driver rating](Rating-and-Performance.md) has earned, paid for the races you turn up to, and paid again on where you finish the championship.

## Seats on offer

Open a season on the **Manage** tab and the seats the grid will give you are listed under its settings, with the salary, the season's target, and a sentence explaining how each offer was arrived at.

Offers are **re-derived every time you look**. They are what a team would say today, given your rating, your last season and what the career is worth — so improving and looking again is exactly how they are meant to be used. Once you sign, the terms are fixed: a contract is history and is never recalculated.

Each row shows:

| | |
|---|---|
| **Team** | Who is offering, with a picture of the car — the same preview image its livery mod gives the game's own car picker. Rows show no picture where the mod ships none, which is normal |
| **Badge** | *Renewal* or *Pay driver*. Most rows are an ordinary paid deal and carry no badge |
| **Terms** | Salary for the season, and the finishing position they expect |
| **Price** | Sponsorship you must bring, for a seat you are buying into |
| **Car** | Which of the team's cars you take, and whose seat it is. Every car the team runs is offered, whoever drives it; the choice is fixed for the season once signed |

Teams the roster names but AMS2 has no livery for never reach a grid at all, so they are not among the seats on offer — the Car Performance tab marks them **no livery**. The teams that make no offer are otherwise simply the ones missing from the list. Under the heading you will find your rating, your balance, and the seat you held last season.

### Salary

Pay is scaled by how quick the car is, across a 40× spread from the back of the grid to the front — roughly the real gap between a front-runner and a rookie. Clearing a team's bar comfortably earns you more than scraping in at their rate. The exact figures are tunable in Config (`contract_top_salary` and `contract_floor_salary`).

### The target

Teams ask you to finish around where the car should finish — not an invented number. A team so far back that any target would be meaningless asks for none.

Meeting it matters: see *Renewals* below.

### Pay drivers: buying a seat

Some seats are for sale. **Only the slowest third of the grid sells one**, because sponsorship money is what keeps a skint team running — Osella spent 1986 asking its drivers to bring backing, as did AGS and Coloni. A front-runner has no budget hole to plug and loses more by fielding a slow driver than a cheque covers, so it will not have you at any price.

And a back-marker only charges a driver it does not actively *want*. Clear its bar by a comfortable margin and it pays you like anyone else; fall short, and the seat is for sale at a price set by how far short you are. The price slides to nothing exactly where the seat turns back into a paid one.

A bought seat still pays a salary — even a pay driver is on the payroll — otherwise the sponsorship could never be recovered and one purchase would end a career.

You cannot sign a deal you cannot afford; the **Sign** button is disabled and the row says so.

> **A career is never locked out, and never handed a seat.** If nothing at all is obtainable — nothing earned, nothing affordable — the cheapest seat for sale drops its price to exactly what the career holds. It still costs everything you have. That is the point: a way in, not a gift. Only if sponsorship is switched off entirely (`contract_buy_in_per_point: 0`) does the least demanding team take you for free, because the alternative would be a career that cannot start.

### Renewals

Meet your target and your team re-signs you **whatever your rating says**, with a rise for each season you have served (up to a point). Re-earning your seat every winter would make season objectives decorative.

Miss it and you drop back to whatever you can earn on merit — not to something worse.

A renewal is never charged: a team that just had its target met does not turn round and bill you.

**How the team felt at the end of the season counts too.** This is the team mood from the [Paddock](Career.md#team-mood):

- **Target met:** you are re-signed whatever the mood.
- **Target missed:** you drop back to merit, unless the team ended the season *delighted* with you. Then it keeps you anyway.
- **No target set** (a car too far back for one): you are re-signed unless the team ended the season *losing patience* with you.
- **Delighted team:** a renewal from a delighted team pays a little more, on top of the loyalty rise.

The offer's explanation says when the mood decided it, and the line above the offers says how the team ended the season. If the team loses patience for three races running during a season, the Paddock warns that it is considering its options. It's a warning only: your contract always runs to the end of the season.

**An incumbency does not leave its series.** Team names repeat across rosters — "Ferrari" appears in seven of the eight shipped classes, spanning thirty years — so a seat is only held inside the class it was held in. Switching series is a fresh start, and the offers panel says so rather than leaving the missing renewal looking like a bug. Moving between generations of one series (Gen1 to Gen2, say) counts as a change: different grid, different cars, different bar.

### Teams carried over to the next season

A roster for the next season often renames a team that changed engine. Spirit-Honda in 1983 becomes Spirit-Hart in 1984, for example. Without a link the app would treat you as a stranger to your own team, with no renewal.

When you create the next season in the same series, the Manage tab shows **Teams carried over from** the season you finished, above the offers:

- A team still racing under the same name carries over to itself.
- Otherwise a team carries over when its constructor (the part of the name before the hyphen, "Spirit") is unique on both grids.
- Where that is unclear (1983 ran both a Lotus-Ford and a Lotus-Renault) or a team is gone, it shows **no match**.
- Your own last team is listed first, because its link decides whether its offer is a renewal. Pick a different successor, or "none", from the dropdown and press **Save**; the offers update straight away.
- The line above the offers says which team continues your last seat, or that none does.
- The links lock once you sign or the season has a recorded session, since the offers were made on them.

The usual way through: finish the season, put the next season's roster file in the game's Custom AI Drivers folder, create the new season with it, then check the links before signing. The finished season keeps its own copy of its roster, so replacing the game's file doesn't change how it was judged.

### One season at a time

**Every deal runs for exactly one championship.** There are no multi-year contracts. A deal spanning seasons would have to survive the roster changing under it — the next season may be raced on an entirely different Custom AI file, where the team may not exist at all.

## Being paid

### Salary, one instalment per race

Your salary is paid **race by race**, against the number of races you declared the season would run.

- Race the full calendar and you draw the whole salary.
- Stop short and you keep only what you raced for.
- Race past it and you earn nothing extra. The contracted figure is a ceiling, not a promise.

The asymmetry is deliberate: only you decide when a season is over, so pressing **Finish** must never be worth money.

A race means a round holding a recorded race session — the same thing the standings score on. A weekend you only practised is not a race.

You can change a season's race count at any time, including after it has started. Since the wage is capped and never topped up, resizing only changes the instalments still to come. A season created before calendars existed has none, and keeps the old rule exactly: nothing until the season is Final, then the whole salary at once. Setting a race count on it is what starts it paying per race.

### Prize money

Prize money is paid on **final championship position**, scaled by the size of the field, and only when a season is marked **Final** — there is no final position until the season is over. While a season is being raced the Finances page shows what today's standings *would* pay, in amber with a `~`, so money that is not banked never reads like money that is.

**A finished season's payout is settled at the moment you finish it.** Retuning prize money in Config afterwards moves the seasons still to come and nothing else.

## The Finances page

**Career → Finances** shows the balance, the season being raced as a running total, a row per season, and where the money went. See [Career Tab](Career.md#finances-sub-tab).

Your balance is what you were founded with, plus what you have earned, minus what you have spent. The founding balance is copied from Config when the career is created and then kept — a career that started with five million still started with five million after you edit the setting.

The balance can run negative: reassigning a session moves standings that may already have paid out, and that is shown as debt rather than hidden.

## Tuning the economy

All in the **Config** tab (see [Getting Started](Getting-Started.md#career-rules-rating-and-money)):

| Setting | What it moves |
|---|---|
| **Salary range** | Pay for the quickest and the slowest car; everything between is interpolated by pace |
| **Prize money** | What winning the championship pays, and what finishing last among the scorers pays |
| **Starting balance** | What a *newly created* career begins with. By default enough to buy into at least two sponsored seats on any shipped grid, so there is a choice of way in |
| **Sponsorship required** | Credits per rating point short of a selling team's bar. This is the knob that makes a real shortfall cost millions; `0` switches bought seats off entirely |

Editing any of these changes the seasons still to come. Deals already signed keep their terms, seasons already finished keep their payouts, and a career keeps the balance it was founded with.
