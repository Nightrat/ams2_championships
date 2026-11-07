//! The story a season tells: who the player is measured against, and how that is going.
//!
//! Every part is **worked out from results on each request**, like the rating and the offers —
//! nothing here is stored. Each module answers one question about one input, [`facts::SeasonFacts`],
//! which the service builds; no story module reads the career file, a roster or the config
//! itself. The rest of the career never depends on this module.

pub mod biography;
pub mod facts;
pub mod headlines;
pub mod mood;
pub mod review;
pub mod rival;
pub mod tally;
pub mod targets;
pub mod teammate;
pub mod templates;
pub mod title;

#[cfg(test)]
#[path = "../tests/career/story/support.rs"]
pub(crate) mod test_support;
