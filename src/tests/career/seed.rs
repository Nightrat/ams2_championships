use super::*;

#[test]
fn test_hash_separates_its_parts() {
    assert_ne!(hash64(&["ab", "c"]), hash64(&["a", "bc"]));
}

#[test]
fn test_hash_is_pinned() {
    // Salaries hang off this, and so does every headline's wording. Changing the hash silently
    // re-rolls every team's terms in every existing career, so it is pinned rather than left to
    // whatever the implementation becomes.
    assert_eq!(hash64(&["c1", "Brabham"]), 0x4e86_46be_3ed8_0686);
}
