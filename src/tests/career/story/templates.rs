use super::*;

/// Every placeholder a rule ever fills. A template naming anything else has a typo, and would
/// print the brace literally.
const KNOWN: &[&str] = &[
    "driver", "track", "pos", "team", "result", "count", "nth", "previous", "gap", "leader",
    "expected", "teammate", "parts", "player", "other", "planned", "rival", "conditions",
    "target", "holder", "won", "lost", "since", "standing", "won_times", "lost_times", "round", "met", "judged", "change", "reasons", "field", "result", "high", "low", "end", "band", "season", "points",
];

#[test]
fn test_every_key_is_unique() {
    let mut keys: Vec<&str> = keys().collect();
    let all = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), all);
}

#[test]
fn test_every_placeholder_is_one_a_rule_fills() {
    let values: Vec<(&str, String)> = KNOWN.iter().map(|k| (*k, "x".to_string())).collect();
    for key in keys() {
        for seed in 0..8 {
            let text = render(key, &values, seed);
            assert!(!text.contains('{') && !text.contains('}'), "{key}: {text}");
        }
    }
}

#[test]
fn test_the_same_seed_always_picks_the_same_variant_and_seeds_spread() {
    assert_eq!(pick("win", 7), pick("win", 7));
    let variants: std::collections::HashSet<&str> = (0..30).map(|s| pick("win", seed(&[&s.to_string()]))).collect();
    assert!(variants.len() > 1, "a three-variant key never varied");
}

#[test]
fn test_render_fills_values() {
    let text = render("teammate.races", &[("player", "7".into()), ("other", "3".into())], 0);
    assert_eq!(text, "races 7–3");
}
