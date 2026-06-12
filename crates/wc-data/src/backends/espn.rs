//! ESPN hidden-API backend (default).
//!
//! Endpoints (base `https://site.api.espn.com`, league `soccer/fifa.world`):
//! - `/apis/site/v2/sports/soccer/fifa.world/scoreboard` — fixtures + live state
//! - `/apis/site/v2/sports/soccer/fifa.world/summary?event={id}` — match detail
//! - `/apis/v2/sports/soccer/fifa.world/standings` — group tables
//!
//! NOTE: This is currently a stub returning empty data so the workspace
//! compiles; the real mapping is implemented separately.

use time::Date;

use crate::domain::{Bracket, Calendar, Group, Match, MatchDetail};
use crate::error::{DataError, Result};
use crate::provider::ScoreProvider;
use crate::transport::Http;

/// ESPN-backed provider.
#[derive(Debug, Clone)]
pub struct EspnProvider {
    #[allow(dead_code)]
    http: Http,
}

impl EspnProvider {
    /// Build the ESPN provider over a shared HTTP client.
    #[must_use]
    pub fn new(http: Http) -> Self {
        Self { http }
    }
}

impl ScoreProvider for EspnProvider {
    fn name(&self) -> &'static str {
        "ESPN"
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
            provider: "ESPN",
            what: "match detail (not yet implemented)",
        })
    }
}
