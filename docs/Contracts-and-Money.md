# Contracts & Money

*Singleplayer careers only. A multiplayer career races people, so it has no seats to be offered and no money; the Finances page and all of this are hidden there.*

Every season in a singleplayer career is raced for a team, on terms the grid decides. You are offered seats your [driver rating](Rating-and-Performance.md) has earned, paid for the races you turn up to, and paid again on where you finish the championship.

## Seats on offer

Open a season on the **Manage** tab and the seats the grid will give you are listed under its settings, with the salary, the season's target, and a sentence explaining how each offer was arrived at.

Offers are **re-derived every time you look**. They are what a team would say today, given your rating, your last season and what the career is worth — so improving and looking again is exactly how they are meant to be used. Once you sign, the terms are fixed: a contract is history and is never recalculated.

Each row shows:

| | |
|---|---|
| **Team** | Who is offering |
| **Badge** | *Renewal* or *Pay driver*. Most rows are an ordinary paid deal and carry no badge |
| **Terms** | Salary for the season, and the finishing position they expect |
| **Price** | Sponsorship you must bring, for a seat you are buying into |

The teams that make no offer are the ones missing from the list. Under the heading you will find your rating, your balance, and the seat you held last season.

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

**An incumbency does not leave its series.** Team names repeat across rosters — "Ferrari" appears in seven of the eight shipped classes, spanning thirty years — so a seat is only held inside the class it was held in. Switching series is a fresh start, and the offers panel says so rather than leaving the missing renewal looking like a bug. Moving between generations of one series (Gen1 to Gen2, say) counts as a change: different grid, different cars, different bar.

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
