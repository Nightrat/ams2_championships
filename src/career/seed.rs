//! The one source of seeded variety: a hash that never changes.
//!
//! Anything the app varies — a team's salary within its jitter, which wording a headline gets —
//! is seeded from career facts through here, so the same career always comes out the same and
//! reloading a page never rerolls anything.

/// FNV-1a over the given parts, with a separator so `("ab", "c")` and `("a", "bc")` differ.
///
/// Hand-rolled rather than taken from `DefaultHasher`, whose algorithm is explicitly allowed to
/// change between Rust releases. A team's terms must not move because the toolchain did.
pub(crate) fn hash64(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
#[path = "../tests/career/seed.rs"]
mod tests;
