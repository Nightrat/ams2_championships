//! How the story is said: every sentence it can write, by key.
//!
//! No rule holds a sentence. A rule names a key and hands over the values to fill in, and this
//! table decides the wording — so a headline can be reworded without touching when it fires, and
//! translating the story would be this one file. No AI writes any of it: every line is here.
//!
//! A key with several variants gets one picked by a seed taken from career facts (see
//! [`crate::career::seed`]), so a season does not read as one sentence repeated, while the same
//! career always reads the same.

use crate::career::seed::hash64;

/// Every sentence, by key. Keys are stable strings: renaming one is harmless (nothing stores
/// them), but each must stay unique. Placeholders are `{name}`, filled by [`render`].
const TEMPLATES: &[(&str, &[&str])] = &[
    // ── The player ──────────────────────────────────────────────────────────────────────────
    (
        "win.maiden",
        &[
            "{driver} takes a maiden win at {track}",
            "{track}: {driver} wins for the first time",
            "First win for {driver}, at {track}",
        ],
    ),
    (
        "win",
        &[
            "{driver} wins at {track}",
            "Victory for {driver} at {track}",
            "{driver} first past the flag at {track}",
        ],
    ),
    (
        "podium.maiden",
        &[
            "A first podium for {driver}: {pos} at {track}",
            "{driver} on the podium for the first time, {pos} at {track}",
        ],
    ),
    (
        "podium",
        &[
            "Podium for {driver}: {pos} at {track}",
            "{driver} on the podium again, {pos} at {track}",
        ],
    ),
    (
        "pole.maiden",
        &[
            "Maiden pole for {driver} at {track}",
            "{driver} on pole for the first time, at {track}",
        ],
    ),
    (
        "pole",
        &[
            "Pole position for {driver} at {track}",
            "{driver} quickest in qualifying at {track}",
        ],
    ),
    (
        "points.first",
        &[
            "First points for {driver}: {pos} at {track}",
            "{driver} opens the account with {pos} at {track}",
        ],
    ),
    (
        "debut.career",
        &[
            "{driver} makes a debut at {track}: {result}",
            "Debut race for {driver} at {track}: {result}",
        ],
    ),
    (
        "debut.team",
        &[
            "{driver}'s first race for {team}: {result} at {track}",
            "{team} debut for {driver} at {track}: {result}",
        ],
    ),
    (
        "best",
        &[
            "{driver}'s best result of the season: {pos} at {track}",
            "{pos} at {track}, {driver}'s best yet this season",
        ],
    ),
    (
        "streak.points",
        &[
            "{count} points finishes in a row for {driver}",
            "{driver} scores for the {nth} race running",
        ],
    ),
    (
        "streak.podium",
        &[
            "{count} podiums in a row for {driver}",
            "{driver} on the podium for the {nth} race running",
        ],
    ),
    (
        "dnf.first",
        &[
            "First retirement of the season for {driver}, at {track}",
            "{driver} out at {track}, a first retirement this season",
        ],
    ),
    // ── The field ───────────────────────────────────────────────────────────────────────────
    (
        "lead.taken",
        &[
            "{driver} takes the championship lead from {previous}",
            "New championship leader: {driver}, {gap} clear of the field",
        ],
    ),
    (
        "win.not_leader",
        &[
            "{driver} wins at {track}, but {leader} still leads the championship",
            "{track} goes to {driver}; {leader} keeps the lead by {gap}",
        ],
    ),
    (
        "win.first_of_season",
        &[
            "{driver} wins for the first time this season, at {track}",
            "First win of the season for {driver}, at {track}",
        ],
    ),
    (
        "upset",
        &[
            "{driver} brings the {team} home {pos} at {track}, far above where the car should be",
            "Giant-killing at {track}: {driver} {pos} in the {team}, a car expected around {expected}",
        ],
    ),
    // ── The title fight ─────────────────────────────────────────────────────────────────────
    (
        "title.clinched",
        &[
            "{driver} clinches the championship at {track}",
            "{driver} is champion: the title is settled at {track}",
        ],
    ),
    (
        "title.out",
        &[
            "{driver} is out of the title fight after {track}",
            "The title is out of reach for {driver} after {track}",
        ],
    ),
    ("title.progress", &["{count} of {planned} races run"]),
    ("title.no_races", &["no races run yet"]),
    ("title.champion", &["champion"]),
    ("title.leading", &["leading by {gap}"]),
    ("title.leading_countback", &["level with {rival}, ahead on countback"]),
    ("title.behind", &["{gap} behind {leader}"]),
    ("title.eliminated", &["out of the title fight"]),
    ("title.leader", &["{leader} leads"]),
    ("title.clinch_next", &["{driver} can clinch the title at the next round by {conditions}"]),
    ("title.cond.outscore", &["outscoring {rival} by {gap}"]),
    ("title.cond.match", &["scoring at least as many points as {rival}"]),
    ("title.cond.within", &["losing no more than {gap} to {rival}"]),
    (
        "title.past_plan",
        &["All {planned} planned races are run. Finish the season on the Manage tab to make the standings final."],
    ),
    // ── The rival ───────────────────────────────────────────────────────────────────────────
    (
        "rival.new",
        &[
            "{driver} emerges as {player}'s rival",
            "A rivalry takes shape: {player} and {driver} keep finishing side by side",
        ],
    ),
    (
        "rival.replaced",
        &[
            "{driver} replaces {previous} as {player}'s closest rival",
            "A new rival for {player}: {driver}, ahead of {previous}",
        ],
    ),
    (
        "rival.duel.ahead",
        &[
            "{player} now leads {driver} {won}–{lost} in their head-to-head, after {track}",
            "{track} puts {player} ahead of rival {driver}, {won}–{lost}",
        ],
    ),
    (
        "rival.duel.behind",
        &[
            "{driver} now leads {player} {lost}–{won} in their head-to-head, after {track}",
            "{track} puts rival {driver} ahead of {player}, {lost}–{won}",
        ],
    ),
    // The head-to-head counts every race of the season both started, not only those since they
    // became the rival — "this season" says so, beside a "since round" that could suggest otherwise.
    (
        "rival.summary",
        &["Your rival since round {since}. This season you have finished ahead of them {won_times}, they ahead of you {lost_times}; {standing}."],
    ),
    (
        "rival.summary.behind",
        &["Your rival since round {since}. This season they have finished ahead of you {lost_times}, you ahead of them {won_times}; {standing}."],
    ),
    (
        "rival.summary.level",
        &["Your rival since round {since}. This season you have each finished ahead of the other {won_times}; {standing}."],
    ),
    ("rival.standing.ahead", &["you are {gap} ahead in the championship"]),
    ("rival.standing.behind", &["you are {gap} behind in the championship"]),
    ("rival.standing.level", &["you are level on points"]),
    // ── Weekend targets ─────────────────────────────────────────────────────────────────────
    ("target.top", &["finish {pos} or better"]),
    ("target.teammate", &["finish ahead of {teammate}"]),
    ("target.finish", &["bring the car home"]),
    ("target.next", &["The team's target for round {round}: {target}."]),
    ("target.record", &["Targets met: {met} of {judged}."]),
    // ── The team's mood ─────────────────────────────────────────────────────────────────────
    (
        "mood.up.delighted",
        &[
            "{team} publicly backs {driver} after {track}",
            "Smiles all round at {team}: {driver} is the team's man after {track}",
        ],
    ),
    (
        "mood.up.content",
        &[
            "{team} warms to {driver} after {track}",
            "{driver} wins back the trust of {team} at {track}",
        ],
    ),
    (
        "mood.up.concerned",
        &[
            "{driver} buys some time at {team} with {track}",
            "Less talk of replacing {driver} at {team} after {track}",
        ],
    ),
    (
        "mood.down.content",
        &[
            "The glow fades at {team} after {track}",
            "{team} expects more of {driver} after {track}",
        ],
    ),
    (
        "mood.down.concerned",
        &[
            "{team} is concerned about {driver}'s form after {track}",
            "Questions at {team} over {driver} after {track}",
        ],
    ),
    (
        "mood.down.losing_patience",
        &[
            "{team} is losing patience with {driver} after {track}",
            "{driver} under pressure at {team} after {track}",
        ],
    ),
    (
        "mood.considering",
        &[
            "{team} is said to be considering its options after {track}",
            "Paddock talk after {track}: {team} weighing up life without {driver}",
        ],
    ),
    (
        "mood.warning",
        &["{team} is considering its options. Your contract runs to the end of the season, but the renewal is in doubt unless you meet your target."],
    ),
    ("mood.band.delighted", &["Delighted"]),
    ("mood.band.content", &["Content"]),
    ("mood.band.concerned", &["Concerned"]),
    ("mood.band.losing_patience", &["Losing patience"]),
    ("mood.says.delighted", &["{team} couldn't be happier with you."]),
    ("mood.says.content", &["{team} is happy with how things are going."]),
    ("mood.says.concerned", &["{team} wants to see more from you."]),
    ("mood.says.losing_patience", &["{team} is running out of patience."]),
    ("mood.why.up", &["Up {change} after {track}: {reasons}."]),
    ("mood.why.down", &["Down {change} after {track}: {reasons}."]),
    ("mood.why.flat", &["Unchanged after {track}: {reasons}."]),
    ("mood.why.quiet", &["Easing back toward neutral after {track}, with nothing to move it."]),
    ("mood.f.target_met", &["met the weekend target"]),
    ("mood.f.target_missed", &["missed the weekend target"]),
    ("mood.f.beat_car", &["finished ahead of where the car should"]),
    ("mood.f.below_car", &["finished behind where the car should"]),
    ("mood.f.beat_teammate", &["finished ahead of your teammate"]),
    ("mood.f.lost_teammate", &["finished behind your teammate"]),
    ("mood.f.on_objective", &["on course for the season objective"]),
    ("mood.f.off_objective", &["short of the season objective"]),
    ("mood.f.far_off_objective", &["well short of the season objective"]),
    ("mood.f.retired", &["retired"]),
    // ── The season review ───────────────────────────────────────────────────────────────────
    ("review.champion", &["{driver} won the championship with {points}."]),
    ("review.champion.player", &["You won the championship with {points}!"]),
    ("review.position", &["You finished {pos} of {field} with {points}."]),
    ("review.objective.met", &["The team asked for {target} or better: target met."]),
    ("review.objective.missed", &["The team asked for {target} or better: target missed."]),
    ("review.best", &["Best weekend: {pos} at {track}, round {round}."]),
    ("review.worst", &["Hardest weekend: {result} at {track}, round {round}."]),
    (
        "review.rival",
        &["Your rival was {driver}: you finished ahead of them {won_times}, they ahead of you {lost_times}."],
    ),
    ("review.mood", &["{team} ended the season {band} with you ({end}; at its best {high}, at its lowest {low})."]),
    ("review.pending", &["The review is written once the season is finished."]),
    // ── The career ──────────────────────────────────────────────────────────────────────────
    ("career.line", &["{parts}"]),
    ("career.place", &["{pos} of {field}"]),
    ("career.win", &["1 win"]),
    ("career.wins", &["{count} wins"]),
    ("career.champion", &["Champion"]),
    ("career.first_start", &["First race: {track}, round {round} of {season}."]),
    ("career.first_points", &["First points: {track}, round {round} of {season}."]),
    ("career.first_podium", &["First podium: {track}, round {round} of {season}."]),
    ("career.first_win", &["First win: {track}, round {round} of {season}."]),
    ("career.first_pole", &["First pole: {track}, round {round} of {season}."]),
    ("career.first_title", &["First title: {season}."]),
    // ── The contract's objective ────────────────────────────────────────────────────────────
    ("objective.on", &["The team wants {target} or better. You are {pos}: on target."]),
    ("objective.off", &["The team wants {target} or better. You are {pos}, {gap} behind {holder}."]),
    ("objective.unplaced", &["The team wants {target} or better. {holder} holds it with {gap}."]),
    ("objective.open", &["The team wants {target} or better in the championship."]),
    // ── The teammate summary ────────────────────────────────────────────────────────────────
    ("teammate.clause", &["Against {teammate}: {parts}"]),
    ("teammate.races", &["races {player}–{other}"]),
    ("teammate.qualifying", &["qualifying {player}–{other}"]),
];

/// The variant of `key` that `seed` picks.
///
/// An unknown key is a programming error, caught by the tests; in a release build it comes back
/// as the key itself, which is visibly wrong rather than silently empty.
pub fn pick(key: &str, seed: u64) -> &str {
    match TEMPLATES.iter().find(|(k, _)| *k == key) {
        Some((_, variants)) if !variants.is_empty() => variants[(seed % variants.len() as u64) as usize],
        _ => {
            debug_assert!(false, "no template for {key}");
            key
        }
    }
}

/// `key`'s sentence for `seed`, with each `{name}` replaced by its value.
pub fn render(key: &str, values: &[(&str, String)], seed: u64) -> String {
    let mut text = pick(key, seed).to_string();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// The seed for one sentence about one thing: the same parts always give the same wording.
pub fn seed(parts: &[&str]) -> u64 {
    hash64(parts)
}

#[cfg(test)]
pub(crate) fn keys() -> impl Iterator<Item = &'static str> {
    TEMPLATES.iter().map(|(k, _)| *k)
}

#[cfg(test)]
#[path = "../../tests/career/story/templates.rs"]
mod tests;
