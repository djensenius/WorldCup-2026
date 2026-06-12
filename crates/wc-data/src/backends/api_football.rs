//! API-Football (`api-sports.io`) backend.
//!
//! Auth: header `x-apisports-key: <key>`. Base `https://v3.football.api-sports.io`.
//! Endpoints used: `/fixtures`, `/standings`, `/fixtures/events`,
//! `/fixtures/lineups`, `/fixtures/statistics`.
//!
//! NOTE: This is currently a stub returning empty data so the workspace
//! compiles; the real mapping is implemented separately.

use time::Date;

use crate::domain::{Bracket, Calendar, Group, Match, MatchDetail};
use crate::error::{DataError, Result};
use crate::provider::ScoreProvider;
use crate::transport::Http;

/// API-Football-backed provider.
#[derive(Debug, Clone)]
pub struct ApiFootballProvider {
    #[allow(dead_code)]
    http: Http,
    #[allow(dead_code)]
    key: String,
}

impl ApiFootballProvider {
    /// Build the provider over a shared HTTP client with the given API key.
    #[must_use]
    pub fn new(http: Http, key: String) -> Self {
        Self { http, key }
    }
}

impl ScoreProvider for ApiFootballProvider {
    fn name(&self) -> &'static str {
        "API-Football"
    }

    async fn calendar(&self) -> Result<Calendar> {
        Ok(Calendar::default())
    }

    async fn scoreboard(&self, _day: Option<Date>) -> Result<Vec<Match>> {
        Ok(Vec::new())
    }

    async fn standings(&self) -> Result<Vec<Group>> {
        Ok(Vec::new())
    }

    async fn bracket(&self) -> Result<Bracket> {
        Ok(Bracket::default())
    }

    async fn match_detail(&self, _id: &str) -> Result<MatchDetail> {
        Err(DataError::Unsupported {
            provider: "API-Football",
            what: "match detail (not yet implemented)",
        })
    }
}
