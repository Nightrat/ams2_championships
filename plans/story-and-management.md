# Story and management layer — implementation plan

Goal: a season should produce its own story (rivals, teammate battles, title fights, a team
that is pleased or losing patience) and give the player decisions to make between races,
without the app making up anything the career cannot explain.

The race engineer and telemetry feedback are **out of scope** and left for later.

## Ground rules (apply to every phase)

- **Work it out, don't store it.** Headlines, rivalries, head-to-heads, mood and title maths are
  worked out again from `results` on every request, like the rating and offers. Only a
  **player's choice** is stored (so far only the team lineage of Phase 7a), in
  `#[serde(default)]` fields so old saves
  still load.
- **Seeded, never rerolled.** Any variety (which template, which target) is seeded through
  `contracts::hash64` (made `pub(crate)` or moved into a shared helper) from career/champ/session
  ids. Reloading a page never changes the story.
- **No AI writes the story.** No LLM, no network call. Every sentence comes from a handwritten
  template filled with career facts, so the app works offline, every line can be traced to a
  rule, and the same career always reads the same.
- **Rules live in the service.** A new `career/story.rs` (pure logic, no I/O) plus
  `service/story.rs` (the `App` methods). Sentences are built server-side, like `offer_why`. The
  browser prints them and recomputes nothing.
- **Mode-aware.** Anything that needs a team (teammate, mood, team targets) is
  `mode.uses_roster()` / `uses_contracts()` only. Anything built from results alone (news,
  title maths, rival) works in multiplayer too.
- **Practice never counts** for anything here, the same rule as the grid warnings.
- Each phase ends with: tests in `src/tests/career/story.rs` / `src/tests/service.rs`, a route
  test in `src/tests/server.rs`, a CLAUDE.md section, and a user note in `docs/`.
- `test_reference_career_rating_snapshot` and the contracts reference tests must stay green.
  Until Phase 7, no phase may change a rating, an offer or a credit.

## Architecture (applies to every phase)

The goal is that **adding a story element means adding code, not editing existing code**. A new
headline, mood factor or weekend target is a new item in one list. The code
that walks seasons, scores mood or settles money does not change.

### Module layout (follows the existing `service.rs` + `service/` convention, no `mod.rs`)

```
src/career/story.rs            declares the modules below. No logic of its own.
src/career/story/facts.rs      SeasonFacts: the one read-only input every rule sees
src/career/story/teammate.rs   head-to-head
src/career/story/title.rs      title maths, objective tracker
src/career/story/rival.rs      rival selection
src/career/story/headlines.rs  the Headline type, the rule trait, the rule list, the walker
src/career/story/headlines/    one file per group of headline rules (milestones, field, title…)
src/career/story/targets.rs    weekend targets
src/career/story/mood.rs       the mood trait, the factor list, the replay
src/career/story/templates.rs  sentence tables and the seeded picker
src/service/story.rs           App methods: builds SeasonFacts, calls the rules, returns views
src/tests/career/story/…       mirrors the layout, one test file per module
```

### Strong cohesion: one module, one question

- `facts` answers *what happened* (rounds, results, team, roster, expected positions). Nothing
  else reads `CareerData`, the roster or the rating directly.
- `teammate`, `title`, `rival`, `targets` and `mood` each answer one question about those facts.
- `templates` answers *how it is said*. No rule holds a sentence, only a template key and its
  values, so wording can change without touching logic, and a translation would be one file.
- `service/story` answers *where the inputs come from* (roster file, signed copy, rating
  context, config) and turns the result into views. It holds no story rules.

### Loose coupling: depend on narrow inputs, not on the world

- **`SeasonFacts` is the boundary.** The service builds it once per request (reading the roster
  through `champ_roster_file`, the eligibility through `champ_eligibility`), and every story
  function takes `&SeasonFacts` (plus the earlier results it needs). The story modules never
  import `custom_ai`, `liveries`, `config` or the file system. Tests build `SeasonFacts` by hand
  with no XML and no temp folders.
- **Story depends on the rest of the career. The rest of the career does not depend on story.**
  The one exception is planned, and it stays narrow: contracts take a single
  `mood: MoodBand` on `contracts::Standing` (Phase 7). `contracts` never imports `story`. The
  service passes the value in.
- Results are matched to the player through the existing `is_player`, and to teams through the
  existing roster readers (`DriverBlock::team`), never a second way of working them out.
- The browser gets finished views (`SeasonStory`, sentences included) from one route per need.
  `story.js` only renders. No story rule in JavaScript.

### Open-closed: extension points are lists of trait objects or table rows

Use a trait only where the variation is real (several rules of one kind). Plain functions
everywhere else: this codebase prefers functions, and a trait with one implementation is noise.

| Extension point | Shape | Adding one means |
|---|---|---|
| Headlines | `trait HeadlineRule { fn kind(&self) -> HeadlineKind; fn priority(&self) -> u8; fn check(&self, round: &RoundFacts, so_far: &SeasonSoFar) -> Option<HeadlineDraft>; }` plus `const RULES: &[&dyn HeadlineRule]` | one struct + one line in `RULES` |
| Mood factors | `trait MoodFactor { fn name(&self) -> &'static str; fn delta(&self, round: &RoundFacts, so_far: &SeasonSoFar) -> Option<MoodDelta>; }` plus `FACTORS` | one struct + one line |
| Weekend targets | `trait TargetKind { fn fits(&self, …) -> bool; fn judge(&self, …) -> Verdict; }` plus `TARGETS` | one struct + one line |
| Sentences | a table `template key → [variants]` in `templates.rs` | one row |

- Rule walkers (`headlines::for_season`, `mood::replay`, …) iterate the list and never `match`
  on a kind. A `match` over every kind in more than one place is the sign a rule leaked out.
- Every rule is pure: same facts in, same output out. Variety comes only from
  `templates::pick(key, seed)`, seeded through the shared `hash64`.
- **Saved data must survive change.** The only stored choice, `Championship::team_lineage`, is a
  `#[serde(default)]` field, so a save from before it loads unchanged. Template keys are stable
  strings and never renumbered.

### Maintainability

- Every public story type and rule gets a doc comment saying *why* it scores what it does, in
  the style of the rest of the codebase. Weights and thresholds are named constants next to the
  rule that uses them, not magic numbers.
- Each rule has its own tests (fires when it should, stays quiet when it shouldn't, deterministic).
  The walkers are tested once with a fake rule list.
- Each phase updates CLAUDE.md with a section in the same style as the existing ones (what it
  does, why, what not to do), and the `src/` layout table gains the `story` modules.
- `cargo clippy` clean at the end of every phase. Keep phases one PR each.
- Watch the balance: no generic "story engine" or plugin loading. The lists above are enough,
  and an abstraction gets introduced in the phase that first needs a second implementation of it.

---

## Phase 1 — Teammate head-to-head

*Player gets:* the yardstick real drivers are judged by, a running score against the other car
in the same garage.

- [x] Foundation: create `career/story.rs` + `story/facts.rs` and `service/story.rs`. Build
      `SeasonFacts` (rounds in order, race/quali results per round, player team, roster drivers
      per team) in the service.
- [x] *Deferred to Phase 2:* moving `hash64` to a shared place. Phase 1 has no seeded variety, so
      it has no second user yet, and the ground rules say an abstraction arrives with its first
      need.
- [x] `story::teammate::head_to_head(&SeasonFacts)`: the player's team from `player_team`. Teammates are the
      other **regular** drivers of that team (never per-track stand-ins, never the driver
      `player_seat.replaces`; one teammate per name however many liveries).
- [x] Match results to the roster once, in `SeasonFacts`, by `custom_ai::name_key` (the same
      match seat inference uses, so "Allen Berg" finds "Allan Berg"). A session where a teammate
      did not run counts for neither side.
- [x] `HeadToHead { teammate, qualifyings, qualifying, races, race, points, retirements, average_finish }`,
      each score a `Pair { player, teammate }`. Quali is compared on position. A race where both
      retired counts for nobody.
- [x] Added `data_store::race_points` and `SESSION_QUALIFY`. The standings now score through the
      same function, so story points can never disagree with them.
- [x] Read the roster through `champ_roster_seats` (signed copy, phantoms removed).
- [x] `App::season_story(champ_id)` → `SeasonStory { teammates: TeammatesView }`. Each later
      phase adds a field. Route `GET /api/championships/:id/story`.
- [x] New **Career → Paddock** sub-tab (`story.js`) showing the `Active` season only (no picker:
      one season at a time, and finished seasons live on Championships) and the "Teammate" card, headed by its summary line ("Against Riccardo Patrese:
      races 7–3, qualifying 6–4"). Every later story part is a card on this page. (Moved here
      from the Manage tab, which is for setting a season up.)
- [x] Paddock hidden in MP, like Finances. With no team or roster, the card says why.
- [x] Tests: two-car team, three-entry team, stand-in ignored, teammate missing from a session,
      double DNF, finish vs retirement, facts building, service reasons, route + 404.

## Phase 2 — Paddock news feed

*Player gets:* the season told back as it happens. Milestones and surprises turn a list of
results into events.

- [x] `headlines.rs`: the `HeadlineRule` trait, the `RULES` list and the walker
      `for_season(&SeasonFacts)`. It goes through rounds in order, keeps a `SeasonSoFar`
      (career so far, the player's season, standings) and emits
      `Headline { round, session_id, kind, priority, subject, player, key, text }`. Rules get a
      `RoundContext` with the tally before *and* after the round. The walker never matches on a kind.
- [x] Moved `contracts::hash64` to `career::seed` unchanged (`test_hash_is_pinned` moved with
      it and still passes). `templates` is its second user.
- [x] `templates.rs`: the template table, `pick(key, seed)` and `render`. Rules return a key and
      values, never a finished sentence. The Phase 1 teammate summary is worded through it.
- [x] Each kind is its own `HeadlineRule` in `headlines/player.rs` or `headlines/field.rs`,
      with its own tests.
- [x] Player: win (maiden / again), taking the lead, podium (first / again), first points,
      pole (maiden / again), career or team debut, podium and points streaks, best result of
      the season, first retirement of the season. "First" is career-wide where it can be, from
      the player's earlier seasons (`PlayerHistory::before`, filled by the service).
- [x] Field: a lead change, a first win of the season, a winner who still trails the leader,
      and an upset (5+ places above the team's expected position, only with a roster).
- [x] Templates: two or three variants per key, picked by `hash64(champ, session, key, subject)`.
- [x] Caps: 4 headlines per round, 2 per driver per round.
- [x] Added as `SeasonStory.news` (rounds newest first, each with its track), a card on the
      Paddock.
- [x] Works in MP: the Paddock is shown there now, without the teammate card.
- [x] Tests: each rule fires when it should and only then, the walker with fake rules (caps,
      order), the same career gives identical headlines twice, every template placeholder is
      one a rule fills, an earlier season turns a maiden win into a win, and removing a session
      removes its news.
- [x] Checked against the reference career. Sentences use the track's name only ("Azure
      Circuit", not "Azure_Circuit – Azure_Circuit_2021").
- [ ] *Possible follow-up:* a win streak rule. Three wins in a row currently reads as "on the
      podium for the 3rd race running".

## Phase 3 — Title fight and objective tracker

*Player gets:* stakes before each race. "You can clinch at Monza", "you need P6 or better on
average to meet the team's target".

- [x] Races left = `planned_rounds − rounds that held a race`, never below 0 (a season may run
      past its plan). Without a calendar there is no fight, and the card says to set Races.
- [x] The Races box is now offered in **every** mode (Manage and the create form), required only
      in SP. Otherwise MP, which can't set one, would never see a title fight.
- [x] Most points still available = races left × the best score in the points table. Checked:
      there are no pole or fastest-lap points anywhere in the scoring.
- [x] Per driver: `Clinched`, `InContention` or `Eliminated`. Ranked through the standings' own
      comparator (`data_store::standing_order`, with `Countback` now `pub(crate)`). A rival is
      out only when even winning every race left can't put them ahead, the countback included;
      a tie only the name would settle keeps both in.
- [x] "Can clinch at the next round by outscoring X by N" for the leader, built only from the
      gaps and points left. Strict margins; dropped if only the countback would make it work.
- [x] SP with a contract: an objective tracker. Target position vs current position, and the
      points to whoever holds the target place. *Changed from the plan:* no "average finish
      needed", since that would depend on everyone else's results and could not be stated
      honestly. Target read off the contract, never recomputed.
- [x] Headlines: `TitleClinched` (anyone) and `PlayerOutOfTheFight`. Lead changes already existed.
- [x] The running tally moved into its own `story/tally.rs`: the headlines and the title maths
      both read it.
- [x] Paddock: a "Title fight" card, first on the page. Live tab: a one-line title line for the
      active season ("1986: 8 of 12 races run — 14 points behind Nelson Piquet"), refreshed
      when the tab opens.
- [x] Tests: clinch on the last available point through the countback, a tie only the name would
      settle, a season run past its planned rounds, the clinch-next margin, the objective, both
      headlines, the service lines (including "finish the season" never shown for a `Final` one).

## Phase 4 — Rival

*Player gets:* a named opponent. Someone to beat that the season chose, not the menu.

- [x] `story/rival.rs`: a `Rivalry` replayed inside the tally. Each race the player started,
      every other driver's weight decays (×0.75, so recent races weigh more) and gains
      proximity (3/2/1 for 1/2/3 places away) plus standings closeness. Teammates are excluded.
- [x] Hysteresis: nobody before 2 races; a challenger takes over only after out-weighing the
      rival by a margin for 2 races running. Nothing stored. It is replayed with the tally,
      which is also what lets headlines see the hand-over.
- [x] `RivalView { rival: { name, team, since_round, duel }, points_ahead, summary, reason }`.
- [x] Headlines: "new rival" (or "replaces X as closest rival") and the head-to-head with the
      same rival changing hands. *Changed from the plan:* "beat your rival" would fire most
      weekends, so only the lead changing is news.
- [x] A Rival card on the Paddock, second after the title fight. Works in MP.
- [x] Extracted `DriverResult::beats` (finish beats retirement, two retirements beat nobody),
      used by both the teammate and rival head-to-heads.
- [x] Tests: one race is too few, a stable rival, a hand-over that has to hold for two races, a
      teammate never the rival, a missed race changing nothing, both headlines, the service
      sentence. Checked against the reference career.

## Phase 5 — Weekend targets from the team

*Player gets:* a reason to care about every race, not just the season. The team says what it
expects this weekend and judges it afterwards.

- [x] `targets.rs`: the `TargetKind` trait and the `TARGETS` list (`TopN`, `BeatTeammate`,
      `BringItHome`), each kind with `set` (fits + builds) and `judge` (met / missed / void).
- [x] `targets::season(&SeasonFacts)`: each raced round's target set from the tally *before*
      it and judged on its race, plus `next` for the first unraced round after the last race.
      Top N = the car's expected place (rounded up) + 2 places of slack, and only when that
      still leaves cars behind. Beat the teammate whenever there is one (void if they did not
      start). Bring the car home only right after a retirement. Picked by `hash64` among the
      kinds that fit.
- [x] Paddock card (left column, after the title fight): the next target, "Targets met: N of M",
      and each raced round with ✓ / ✗ / –. Live tab: a second line with the next target.
- [x] No money and no rating effect. Phase 6 reads the verdicts.
- [x] SP/unset with a roster and a team only; the view says why otherwise, and the card is left
      out in MP.
- [x] Tests: top N is the expected place plus slack and never asked of a back-marker, the
      teammate target is void when they sat out, bring-it-home only after a retirement, only
      fitting kinds are ever chosen, the choice never rerolls, the next round, the service view
      and its reasons.
- [ ] *Not done:* target met/missed headlines. They would fire every weekend; Phase 6's mood
      headlines are the better place to make news of them.

## Phase 6 — Team mood

*Player gets:* a relationship with the team. A visible "how they see you" that the whole season
feeds into.

- [x] `mood.rs`: the `MoodFactor` trait, the `FACTORS` list and `replay(&SeasonFacts)`. Each
      factor is one struct with its weight as a named constant.
- [x] `mood::replay` gives a 0–100 figure, starting at 60. Each race the player started, it eases
      10% back toward 60, then the factors add up: weekend target (±6), teammate head-to-head
      (±4), objective pace (+2 / −1 / −3), retirement (−2, small on purpose).
- [x] *Added while tuning:* `CarPerformance`, 0.25 per place against the car's expected finish
      (+5 / −4 at most), with the expectation capped at the grid size. The real 1983 season
      (one-car team, back-marker, no objective) otherwise barely moved, though the player won a
      race in the slowest car.
- [x] Bands Delighted / Content / Concerned / Losing patience, each with what the team says, plus
      "why it moved last race" (every factor that touched it, beside the net change).
- [x] Mood-change headlines when the band changes, worded by direction and band.
- [x] Paddock card (left column, after the title fight): the figure, what the team says, why it
      moved, and a CSS bar per race coloured by band.
- [x] Still display-only. Tuned against copies of the real `sp_1983` and `sp_1969` saves
      (scratchpad copies; the originals untouched).
- [x] Tests: each factor moves the mood its way, the figure stays in 0–100, the drift back to
      neutral, a missed race changes nothing, a car expected behind the grid, same career → same
      figure, the band-change headline, the service view and its reasons.

## Phase 7a — Team lineage across seasons (prerequisite for Phase 7)

*Player gets:* renewals that survive a new season's roster. Without this, the 1984 roster's
"Spirit-Hart" is a stranger to the driver who raced 1983 for "Spirit-Honda", so there's no
renewal and no loyalty rise, whatever the objective or the mood. This affects today's
renewals too, not only Phase 7.

**Why it's needed:** a renewal is recognised by the team's *name* in the same class
(`contracts::Standing`). Rosters for consecutive seasons rename teams that change engine. Between
the prepared 1983 and 1984 `F-Retro_Gen3` rosters: Spirit-Honda → Spirit-Hart, Williams-Ford →
Williams-Honda, McLaren-Ford → McLaren-TAG, plus Ligier, Arrows, Osella and Lotus.

**Decided:** match on constructor automatically when the season is created, show the result,
and let the player correct it.

- [x] *Added:* a team still on the grid under the same name links to itself first, so nothing
      that renewed by name before stops renewing.
- [x] Constructor = the team name before the first `-` ("Spirit" for "Spirit-Hart"); a name
      without one is its own constructor. A team of the previous season links to a team of the
      new one when their constructor matches **uniquely on both sides**. Two candidates (1983
      has Lotus-Ford *and* Lotus-Renault), or none, leave it unmatched for the player to settle.
- [x] "The previous season" = the last completed contracted season in the same class: the one
      `contracts::Standing` already reads. Different class → no lineage (a series change is a
      fresh start, as today).
- [x] The confirmed mapping is a **player decision**, so it's stored: on the new
      `Championship` (`team_lineage: Vec<Link { from, to }>`, `#[serde(default)]`). The proposal
      itself is worked out again, never stored. A season without a stored mapping uses the proposal
      as it stands, so a save from before this feature still renews exactly where the names already
      match.
- [x] Where it lives: its own `career/lineage.rs` (proposal + lookup), read by `contracts` through
      the service, which passes the renewal's team already translated. `contracts` stays
      ignorant of how a name was linked, and `story` is not involved at all.
- [x] UI: after creating a singleplayer season (it has its roster from creation), the Manage tab
      shows "Teams carried over from <previous season>": each previous team with its proposed
      successor, a dropdown to change it (any team of the new grid, or "none"), and unmatched
      teams flagged. At minimum the player's own last team must be settled, since that is the
      renewal. Editable until signing: offers are built on it, and signing locks it like the
      roster.
- [x] Routes: `GET /api/championships/:id/lineage` (proposal + stored choices + which are
      unmatched), `PUT /api/championships/:id/lineage` (store), refused (409) once signed.
- [x] Renewal reads the linked name: the standing's team is translated through the lineage before
      `offers_for_with` compares it, so "Spirit-Honda delivered in 1983" makes Spirit-Hart's 1984
      offer a renewal.
- [x] Tests: the 1983→1984 pair from `custom_ai_files_with_perf_scalars/` proposes Spirit-Honda →
      Spirit-Hart and leaves Lotus ambiguous; a stored correction wins over the proposal; no
      lineage across classes; refused after signing; an old save without a mapping renews by
      name as before (reference tests unchanged).

## Phase 7 — Mood changes the next contract

*Player gets:* consequences. A good season with a happy team pays better at renewal. A bad one
draws a warning.

**How it works without knowing the next season:** offers only exist once the next season does,
and a singleplayer season is created with its roster. Finish 1983 → install the 1984 roster →
create the 1984 season → its offers are worked out against the 1984 grid, reading how 1983 ended
(team, class, objective, and now mood). Nothing has to be predicted. Needs Phase 7a, or the
renewal never recognises the renamed team.

- [x] Give the final mood of the last completed contracted season to `contracts::Standing` (as a
      new field, defaulted to neutral so `Standing::default()` and the reference tests do not
      move).
- [x] *As built (`contracts::renews`):* a met target always renews; a missed target renews only
      when the team ended delighted; with no target set, the team renews unless it lost patience.
      A delighted renewal adds 0.10 of the rate on top of leverage and loyalty.
- [x] Renewal: a high mood adds to the loyalty rise (capped, inside `SALARY_JITTER` style
      bounds). A low mood with a missed target drops the renewal, exactly as a missed target
      does today. Never worse than merit: "never locked out" still holds.
- [x] Mid-season warning: mood in the bottom band for 3+ rounds → a headline and a Paddock
      notice that the team is "considering its options". **No sacking.** The contract runs for
      the season.
- [x] `offer_why` mentions mood when it changed the terms.
- [x] Docs: `docs/Contracts-and-Money.md` gains the rule. CLAUDE.md contracts section updated.
- [x] Tests: mood moves only renewal money. `test_hash_is_pinned` and the contracts reference
      tests are unchanged at neutral mood.

## Phase 8 — Next season's market, mid-season

*Player gets:* something to race for after the title is out of reach. Teams watching you.

**Decided:** the current grid, worded honestly. Mid-season the app can't see next season's
grid: the next roster usually isn't even in the game folder until it replaces this one. So this
shows which teams *on this season's grid* would talk to the player today, and says that is what it
is. It assumes the teams carry over, which with Phase 7a's lineage mostly holds.

- [x] From about ⅔ of `planned_rounds`, show "interest": the teams whose offer you would get
      today in the *same class*, from `offers_for_with` on the current rating and the current
      grid. Nothing new is worked out and nothing is promised.
- [x] Wording says it is this grid ("teams on this season's grid that would talk to you"), and
      that next season's offers are made on next season's roster when it is created.
- [ ] *Not done:* headlines when a quicker team first shows or stops showing interest. They would
      need the rating replayed race by race, the dearest computation in the app, for a headline
      the card already conveys.
- [x] Paddock card "Interest from other teams". Clearly worded as interest, not an offer ("would talk to
      you today").
- [x] Tests: appears only past the threshold. A team that moves out of reach disappears.

## Phase 9 — Season review and career biography

*Player gets:* a record worth keeping. The season and the career as a story, not a table.

- [x] On `Final`: a season review card. Champion, the player's position vs objective, teammate
      score, rival, best and worst weekends, mood arc, the top headlines.
- [x] Career page: a timeline across seasons (teams driven for, titles, firsts): "Your career"
      at the foot of the Paddock, from `GET /api/career/story`. Between seasons the Paddock shows
      the last finished season's review, the career and its news.
- [x] `service::export` puts both into the HTML export (rendered in Rust through `format`).
- [x] Tests: review content for a finished reference season. Export contains it.

---

## Not now

- **Race engineer / telemetry feedback.** Parked by decision. The stint log it needs is
  independent of everything above.
- **Car development / R&D spending.** It conflicts with signed roster copies, the per-context
  rating calibration (one set of scalars per context) and the `power_scalar` cap. Revisit only
  once the rest of the plan proves the idea.
- **AI-generated text (LLM).** Ruled out, not just deferred. See the ground rules.
- **Sponsors with conditions, press questions after a race, and a user-written story file per
  season.** Dropped from the plan as not required.
- **Being sacked mid-season.** It conflicts with one-season contracts and "never locked out".
  Phase 7 warns but never ends a contract.
