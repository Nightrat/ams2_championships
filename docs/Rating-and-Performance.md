# Driver Rating & Performance

*Singleplayer careers only. A multiplayer career hides all of this — it has no roster to be judged against.*

> **None of this works without the rosters, and the rosters do not work without the skins.** Your rating is measured against the pace scalars in the season's Custom AI Drivers file, and the app can only tell which car you were in by matching the recorded grid against that file. So you need the class's **custom liveries installed**, a **Custom AI Drivers file that names them**, and the season raced **with that roster active**. See [What a singleplayer career needs](Getting-Started.md#what-a-singleplayer-career-needs).

## The driver rating

Your driver rating is a number from 0 to 100 that says how good a drive you are giving, relative to the car you are giving it in. It is what decides which seats the grid will offer you.

The problem it exists to solve: P3 in an Osella is a far better drive than P3 in a Williams. A rating built on finishing positions alone would reward you for already having a quick car, so **every score is measured against what your car was expected to do**, using the performance scalars in the season's Custom AI Drivers roster.

It is built from your recorded results:

| Signal | Weight | What it measures |
|---|---|---|
| **Race pace** | 55% | Where you finished against where the car should have finished |
| **Qualifying** | 30% | The same question on Saturday. When a career has no qualifying sessions at all, race pace carries its share rather than qualifying dragging the rating toward the middle |
| **Finishing** | 15% | Whether you get to the end |
| **Team-mate** | bonus, capped | Beating the driver in the other half of your garage — same car, same track, same conditions, so the cleanest signal there is |
| **Multiplayer** | bonus, capped | Results against humans, capped because a small lobby is close to a coin flip |

Some things that follow from how it is built:

- **Nothing is stored.** The rating is re-derived from your sessions every time it is asked for, so correcting a mis-assigned session corrects the rating with it.
- **Recent results count for more.** By default a result ten sessions back counts half. That is also what lets the rating recover after you change AI difficulty: the old scores fade out.
- **A thin record is pulled toward the starting rating** so two lucky afternoons cannot unlock a front-running seat. By around twenty races your own results have taken over entirely.
- **Only races count as evidence.** A career of qualifying sessions with no race, or one where every race ended in a retirement, stays exactly on the starting rating. Races are what a seat is earned in.
- **AI difficulty and field size are never assumed.** They are read from, or cancelled out of, each session.
- **A session not raced on the roster contributes nothing.** If fewer than half the AI on the recorded grid appear in the Custom AI file, the app concludes the session was run on stock AI and skips it rather than guessing — so it moves neither your rating nor the seats you are offered. The session is still recorded and still scores championship points; it simply says nothing about how good a drive it was. A career that never races on its roster stays on the starting rating for ever.

### Race the full grid

**Set the AMS2 opponent count so the grid fills the roster** — one livery is one car, so a class with 22 installed liveries wants 21 opponents.

This matters because of how the rating is worked out. What your car *should* do is its rank among **every team in the roster**: a back-marker belongs around P18 of 22. What you *did* is your finishing position in the field that actually raced. Those two numbers are only comparable when the field is the roster.

Race ten opponents on a 22-car roster and your car is still expected to finish around P18, but P18 does not exist in an eleven-car race — so merely finishing looks like a heroic drive and the rating climbs on nothing. Race more opponents than the roster has cars and AMS2 fills the rest with its own AI; once those outnumber the roster cars the session is skipped entirely.

Count the cars the roster can really field, not the drivers it lists: entries whose livery AMS2 does not own never appear, and are tagged **no livery** in the Driver Performance tab.

## What a team asks for

Every team on the grid has a **requirement** — the rating you need to be offered its seat on merit. It is built from two bars, and by default the stricter of the two applies:

- **Car pace** — the quicker the car, the higher the bar.
- **Incumbent skill** — you are asked to be about as good as the weaker of the two drivers already in the seat.

You can pick which bars apply with **Requirement built from** in the Config tab. Note that *incumbent only* leaves every seat free on a roster that declares no driver skills at all.

Two settings move things around:

- **Requirement offset** shifts every bar on the grid up or down.
- **Offer margin** is how far *below* a bar you may sit and still be offered the seat at the ordinary rate. Inside the margin the seat is yours for the asking; outside it the team is locked, and only a back-of-the-grid team will then sell you a seat for sponsorship.

**Enforce team eligibility** (on by default) is what makes the bars binding: with it off you can set any team as My Team and the ratings become advisory.

## Moving a career onto new settings

The rating tuning is **recorded on a career when it is created and then kept**. Editing it in Config changes new careers only.

That is deliberate. Your rating decided which seats were ever open to you, what each team asked, and whether the contracts you signed could have been signed at all. If Config could reach backwards, retuning the difficulty would rewrite the history of seasons you have already raced.

When you do want to retune the career you are playing, save your changes and then use **Apply these settings to the current career** in the Config tab. The tab warns you when the career you are on has parted from the settings on screen, so the numbers you are reading are never quietly the wrong ones. Applying moves every rating and every team bar in the career, including the ones past seasons were judged against.

## Car Performance tab

The per-class performance scalars from your AMS2 Custom AI Drivers rosters, in a table you can edit. Changes are written straight back to the roster XML, so AMS2 uses them the next time it loads.

These scalars are what the rating measures your results against, and they are also what decides which teams are quick — so editing them moves both the pace order of the grid and the bars every team asks for.

### Entries marked “no livery”

A Custom AI Drivers file cannot create a car. Each entry binds a name, skills and scalars to a livery the game **already owns**, matched on `livery_name` — and an entry naming a livery AMS2 does not have is **silently ignored**. No error, no warning: that driver simply never reaches a grid, and a roster can happily name more drivers than the class has cars.

Rows in that state are tagged **no livery** in the Car and Driver Performance tabs, and a team whose every entry is a phantom has no seat to earn. Editing such a row changes nothing in game.

The check reads the livery manifests your **livery mods** install, which is the only list of livery names that can be read — the game's own livery data is sealed inside its pak files. That makes the check partial by nature: a class no livery mod covers reads as *liveries not verifiable* rather than being guessed at, so an unmodded class is never wrongly condemned.

The scalars affect **your** car too, not only the AI: you occupy a seat in a livery like everyone else.

Reiza documents the scalars as 0.900–1.100, where 1.000 means no change, and edits outside that range are refused. AMS2 reads the file when a session loads, so restart the session for an edit to take effect.

### Baselines and reset

The first time the app writes to a class it saves a copy of the roster as it was, beside the file. That copy is the class's **baseline**, and it is what **Reset** restores.

- It is taken **once**. A class that already has one keeps it, because that copy is the original — re-taking it after an edit would quietly redefine what "reset" means.
- If you hand-tuned a roster yourself *after* the app first recorded one, **Adopt as baseline** overwrites it so a reset comes back to your version rather than a much older one. It is confirmed first, because the old baseline is gone afterwards.
- The baseline lives with the roster in the AMS2 install rather than with the app's data, because it belongs to the roster. AMS2 ignores it.

## Driver Performance tab

The same idea for the drivers: the skill values each roster declares, editable in place and written back to the XML. These feed the *incumbent skill* bar, so raising a driver's skill raises what their team asks of you.
