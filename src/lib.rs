//! Four subjects, plus the layer that strings them together:
//!
//! - [`game`] — the running AMS2 process: shared memory, the session recorder, the spotter.
//!   The only Windows-specific and `unsafe` code lives here.
//! - [`roster`] — the AI grid AMS2 can field: Custom AI files, the liveries they bind to, the
//!   pictures of those cars, and the season year of each class.
//! - [`career`] — the save and everything judged on it: sessions and standings, lap charts,
//!   save folders, the roster copy a signed season keeps, the driver rating, contracts and money.
//! - [`config`] — `config.json`, read by all of the above.
//! - [`format`] — how dates, lap times and money are written, for every front end rendering in Rust.
//! - [`service`] — every operation a user can perform, as typed calls on `service::App`.
//! - [`web`] — transport for the HTTP front end: request parsing, the WebSocket, the page.
pub mod career;
pub mod config;
pub mod format;
pub mod game;
pub mod roster;
pub mod service;
pub mod web;
