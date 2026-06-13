//! The provider abstraction: a uniform [`ScoreProvider`] interface plus a
//! runtime-selectable [`Provider`] enum.
//!
//! The TUI builds one [`Provider`] from a [`ProviderConfig`] and calls its
//! inherent async methods; it never names a concrete backend. New backends are
//! added by implementing [`ScoreProvider`] and adding a variant here.

use time::Date;

use crate::backends::{ApiFootballProvider, EspnProvider, FootballDataProvider};
use crate::domain::{Bracket, Calendar, Group, Match, MatchDetail};
use crate::error::{DataError, Result};
use crate::transport::Http;

/// A uniform interface every backend implements.
///
/// `async fn` in trait is intentional: backends are only ever used through the
/// concrete [`Provider`] enum (not as `dyn ScoreProvider`), so the absence of an
/// auto-`Send` bound is not a problem — the concrete futures are `Send`.
#[allow(async_fn_in_trait)]
pub trait ScoreProvider {
    /// A short, stable name for diagnostics and UI ("ESPN", "API-Football", …).
    fn name(&self) -> &'static str;

    /// The competition calendar (stage windows).
    async fn calendar(&self) -> Result<Calendar>;

    /// Matches for the tournament. `None` returns the full schedule (every
    /// fixture, group stage through the final); `Some(day)` filters to a single
    /// UTC day.
    async fn scoreboard(&self, day: Option<Date>) -> Result<Vec<Match>>;

    /// All group tables.
    async fn standings(&self) -> Result<Vec<Group>>;

    /// The knockout bracket.
    async fn bracket(&self) -> Result<Bracket>;

    /// Full detail for a single match by its provider id.
    async fn match_detail(&self, id: &str) -> Result<MatchDetail>;
}

/// Which backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderKind {
    /// ESPN hidden API (free, no key). The default.
    #[default]
    Espn,
    /// API-Football (`api-sports.io`); requires an API key.
    ApiFootball,
    /// football-data.org; requires an API key.
    FootballData,
}

impl ProviderKind {
    /// All variants, in UI order.
    #[must_use]
    pub fn all() -> [ProviderKind; 3] {
        [
            ProviderKind::Espn,
            ProviderKind::ApiFootball,
            ProviderKind::FootballData,
        ]
    }

    /// The lowercase config token, e.g. `"espn"`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::Espn => "espn",
            ProviderKind::ApiFootball => "api-football",
            ProviderKind::FootballData => "football-data",
        }
    }

    /// Parse a config token (case-insensitive; accepts a few aliases).
    #[must_use]
    pub fn parse(s: &str) -> Option<ProviderKind> {
        match s.trim().to_ascii_lowercase().as_str() {
            "espn" => Some(ProviderKind::Espn),
            "api-football" | "apifootball" | "api_football" => Some(ProviderKind::ApiFootball),
            "football-data" | "footballdata" | "football_data" => Some(ProviderKind::FootballData),
            _ => None,
        }
    }
}

/// Everything needed to build a [`Provider`] at runtime.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfig {
    /// Which backend to use.
    pub kind: ProviderKind,
    /// API key for API-Football, if that backend is selected.
    pub api_football_key: Option<String>,
    /// API key for football-data.org, if that backend is selected.
    pub football_data_key: Option<String>,
}

/// A runtime-selected backend. Dispatches to the concrete provider.
pub enum Provider {
    /// ESPN backend.
    Espn(EspnProvider),
    /// API-Football backend.
    ApiFootball(ApiFootballProvider),
    /// football-data.org backend.
    FootballData(FootballDataProvider),
}

impl Provider {
    /// Build the configured provider, validating that any required API key is
    /// present.
    ///
    /// # Errors
    /// Returns [`DataError::MissingKey`] when the selected backend needs an API
    /// key that was not supplied.
    pub fn from_config(config: &ProviderConfig, http: Http) -> Result<Self> {
        match config.kind {
            ProviderKind::Espn => Ok(Provider::Espn(EspnProvider::new(http))),
            ProviderKind::ApiFootball => {
                let key = config
                    .api_football_key
                    .clone()
                    .filter(|k| !k.trim().is_empty())
                    .ok_or(DataError::MissingKey("API-Football"))?;
                Ok(Provider::ApiFootball(ApiFootballProvider::new(http, key)))
            }
            ProviderKind::FootballData => {
                let key = config
                    .football_data_key
                    .clone()
                    .filter(|k| !k.trim().is_empty())
                    .ok_or(DataError::MissingKey("football-data.org"))?;
                Ok(Provider::FootballData(FootballDataProvider::new(http, key)))
            }
        }
    }

    /// The active backend's display name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Espn(p) => p.name(),
            Provider::ApiFootball(p) => p.name(),
            Provider::FootballData(p) => p.name(),
        }
    }

    /// See [`ScoreProvider::calendar`].
    ///
    /// # Errors
    /// Propagates the active backend's error.
    pub async fn calendar(&self) -> Result<Calendar> {
        match self {
            Provider::Espn(p) => p.calendar().await,
            Provider::ApiFootball(p) => p.calendar().await,
            Provider::FootballData(p) => p.calendar().await,
        }
    }

    /// See [`ScoreProvider::scoreboard`].
    ///
    /// # Errors
    /// Propagates the active backend's error.
    pub async fn scoreboard(&self, day: Option<Date>) -> Result<Vec<Match>> {
        match self {
            Provider::Espn(p) => p.scoreboard(day).await,
            Provider::ApiFootball(p) => p.scoreboard(day).await,
            Provider::FootballData(p) => p.scoreboard(day).await,
        }
    }

    /// See [`ScoreProvider::standings`].
    ///
    /// # Errors
    /// Propagates the active backend's error.
    pub async fn standings(&self) -> Result<Vec<Group>> {
        match self {
            Provider::Espn(p) => p.standings().await,
            Provider::ApiFootball(p) => p.standings().await,
            Provider::FootballData(p) => p.standings().await,
        }
    }

    /// See [`ScoreProvider::bracket`].
    ///
    /// # Errors
    /// Propagates the active backend's error.
    pub async fn bracket(&self) -> Result<Bracket> {
        match self {
            Provider::Espn(p) => p.bracket().await,
            Provider::ApiFootball(p) => p.bracket().await,
            Provider::FootballData(p) => p.bracket().await,
        }
    }

    /// See [`ScoreProvider::match_detail`].
    ///
    /// # Errors
    /// Propagates the active backend's error.
    pub async fn match_detail(&self, id: &str) -> Result<MatchDetail> {
        match self {
            Provider::Espn(p) => p.match_detail(id).await,
            Provider::ApiFootball(p) => p.match_detail(id).await,
            Provider::FootballData(p) => p.match_detail(id).await,
        }
    }
}
