//! Provider backends.
//!
//! Each submodule translates one upstream API into the normalized
//! [`crate::domain`] model by implementing [`crate::provider::ScoreProvider`].
//! Backends are selected at runtime via [`crate::provider::Provider`].

mod api_football;
mod common;
mod espn;
mod football_data;

pub use api_football::ApiFootballProvider;
pub use espn::EspnProvider;
pub use football_data::FootballDataProvider;
