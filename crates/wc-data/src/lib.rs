//! Data layer for the World Cup 2026 TUI.
//!
//! This crate is provider-agnostic: a normalized [`domain`] model describes
//! competitions, matches, standings, brackets, and live match detail, and a set
//! of backends ([`backends`]) translate a specific upstream API into that
//! model. Callers select a backend at runtime through the [`Provider`] enum
//! (see [`provider`]), so the TUI never depends on any single data source.
//!
//! Backends currently implemented:
//! - **ESPN** (default): free, no API key, live data.
//! - **API-Football** (`api-sports.io`): richer stats; requires an API key.
//! - **football-data.org**: simple; requires an API key; limited live detail.

pub mod backends;
pub mod domain;
pub mod error;
pub mod provider;
pub mod transport;

pub use error::{DataError, Result};
pub use provider::{Provider, ProviderConfig, ProviderKind, ScoreProvider};
pub use transport::Http;
