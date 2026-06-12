//! football-data.org backend.
//!
//! Auth: header `X-Auth-Token: <key>`. Base `https://api.football-data.org/v4`.
//! World Cup competition code `WC`. Endpoints: `/competitions/WC/matches`,
//! `/competitions/WC/standings`. The free tier has a low rate limit and limited
//! live-event granularity (no minute-by-minute timeline).
//!
//! NOTE: This is currently a stub returning empty data so the workspace
//! compiles; the real mapping is implemented separately.

use time::Date;

use crate::domain::{Bracket, Calendar, Group, Match, MatchDetail};
use crate::error::{DataError, Result};
use crate::provider::ScoreProvider;
use crate::transport::Http;

/// football-data.org-backed provider.
#[derive(Debug, Clone)]
pub struct FootballDataProvider {
    #[allow(dead_code)]
    http: Http,
    #[allow(dead_code)]
    key: String,
}

impl FootballDataProvider {
    /// Build the provider over a shared HTTP client with the given API token.
    #[must_use]
    pub fn new(http: Http, key: String) -> Self {
        Self { http, key }
    }
}

impl ScoreProvider for FootballDataProvider {
    fn name(&self) -> &'static str {
        "football-data.org"
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
            provider: "football-data.org",
            what: "match detail (limited on this provider)",
        })
    }
}
