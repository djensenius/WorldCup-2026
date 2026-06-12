//! football-data.org backend.
//!
//! Auth: header `X-Auth-Token: <key>`. Base `https://api.football-data.org/v4`.
//! World Cup competition code `WC`. Endpoints: `/competitions/WC/matches`,
//! `/competitions/WC/standings`. The free tier has a low rate limit and limited
//! live-event granularity (no minute-by-minute timeline).

use serde::Deserialize;
use time::{Date, Month, OffsetDateTime, Time};

use crate::backends::common::{day_bounds, f64_to_i16, parse_time};
use crate::domain::{
    Bracket, BracketRound, Calendar, Group, GroupStanding, Lineup, Match, MatchDetail, MatchStatus,
    Score, Stage, StageWindow, Team,
};
use crate::error::Result;
use crate::provider::ScoreProvider;
use crate::transport::Http;

const BASE_URL: &str = "https://api.football-data.org/v4";

/// football-data.org-backed provider.
#[derive(Debug, Clone)]
pub struct FootballDataProvider {
    http: Http,
    key: String,
}

impl FootballDataProvider {
    /// Build the provider over a shared HTTP client with the given API token.
    #[must_use]
    pub fn new(http: Http, key: String) -> Self {
        Self { http, key }
    }

    fn headers(&self) -> [(&str, &str); 1] {
        [("X-Auth-Token", self.key.as_str())]
    }

    async fn fetch<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.http
            .get_json(&format!("{BASE_URL}{path}"), &self.headers())
            .await
    }
}

impl ScoreProvider for FootballDataProvider {
    fn name(&self) -> &'static str {
        "football-data.org"
    }

    async fn calendar(&self) -> Result<Calendar> {
        Ok(static_calendar())
    }

    async fn scoreboard(&self, day: Option<Date>) -> Result<Vec<Match>> {
        let dto: FdMatches = self.fetch("/competitions/WC/matches").await?;
        let calendar = static_calendar();
        let mut matches = dto
            .matches
            .into_iter()
            .map(|m| m.to_match(&calendar))
            .collect::<Result<Vec<_>>>()?;
        if let Some(day) = day {
            let (start, end) = day_bounds(day);
            matches.retain(|m| m.kickoff >= start && m.kickoff < end);
        }
        Ok(matches)
    }

    async fn standings(&self) -> Result<Vec<Group>> {
        let dto: FdStandingsResponse = self.fetch("/competitions/WC/standings").await?;
        Ok(dto
            .standings
            .into_iter()
            .map(FdStanding::into_group)
            .collect())
    }

    async fn bracket(&self) -> Result<Bracket> {
        let matches = self.scoreboard(None).await?;
        Ok(Bracket {
            rounds: Stage::knockout_order()
                .into_iter()
                .map(|stage| BracketRound {
                    stage,
                    matches: matches
                        .iter()
                        .filter(|m| m.stage == stage)
                        .cloned()
                        .collect(),
                })
                .collect(),
        })
    }

    async fn match_detail(&self, id: &str) -> Result<MatchDetail> {
        let dto: FdMatchEnvelope = self.fetch(&format!("/matches/{id}")).await?;
        // NOTE: football-data free tier lacks fine-grained events; timeline is empty.
        Ok(MatchDetail {
            summary: dto.match_.to_match(&static_calendar())?,
            events: Vec::new(),
            lineups: dto.match_.lineups(),
            stats: Vec::new(),
        })
    }
}

fn static_calendar() -> Calendar {
    Calendar {
        stages: vec![
            window(
                Stage::GroupStage,
                "Group Stage",
                (2026, Month::June, 11),
                (2026, Month::June, 28),
            ),
            window(
                Stage::RoundOf32,
                "Round of 32",
                (2026, Month::June, 28),
                (2026, Month::July, 4),
            ),
            window(
                Stage::RoundOf16,
                "Round of 16",
                (2026, Month::July, 4),
                (2026, Month::July, 9),
            ),
            window(
                Stage::QuarterFinal,
                "Quarter-final",
                (2026, Month::July, 9),
                (2026, Month::July, 14),
            ),
            window(
                Stage::SemiFinal,
                "Semi-final",
                (2026, Month::July, 14),
                (2026, Month::July, 18),
            ),
            window(
                Stage::ThirdPlace,
                "Third place",
                (2026, Month::July, 18),
                (2026, Month::July, 19),
            ),
            window(
                Stage::Final,
                "Final",
                (2026, Month::July, 19),
                (2026, Month::August, 1),
            ),
        ],
    }
}

fn window(
    stage: Stage,
    label: &str,
    start: (i32, Month, u8),
    end: (i32, Month, u8),
) -> StageWindow {
    StageWindow {
        stage,
        label: label.to_owned(),
        start: Date::from_calendar_date(start.0, start.1, start.2)
            .map_or(OffsetDateTime::UNIX_EPOCH, |d| {
                d.with_time(Time::MIDNIGHT).assume_utc()
            }),
        end: Date::from_calendar_date(end.0, end.1, end.2)
            .map_or(OffsetDateTime::UNIX_EPOCH, |d| {
                d.with_time(Time::MIDNIGHT).assume_utc()
            }),
    }
}

#[derive(Debug, Deserialize)]
struct FdMatches {
    matches: Vec<FdMatch>,
}
#[derive(Debug, Deserialize)]
struct FdMatchEnvelope {
    #[serde(rename = "match")]
    match_: FdMatch,
}

#[derive(Debug, Deserialize)]
struct FdMatch {
    id: i64,
    #[serde(rename = "utcDate")]
    utc_date: String,
    status: String,
    stage: Option<String>,
    group: Option<String>,
    #[serde(rename = "homeTeam")]
    home_team: FdTeam,
    #[serde(rename = "awayTeam")]
    away_team: FdTeam,
    score: FdScore,
    #[serde(default)]
    lineups: Vec<FdLineup>,
}

impl FdMatch {
    fn to_match(&self, calendar: &Calendar) -> Result<Match> {
        let kickoff = parse_time(&self.utc_date)?;
        let status = fd_status(&self.status);
        Ok(Match {
            id: self.id.to_string(),
            stage: crate::backends::common::stage_for_date(
                calendar,
                kickoff,
                self.stage.as_deref().map_or(Stage::GroupStage, fd_stage),
            ),
            group: self
                .group
                .as_ref()
                .map(|g| g.trim_start_matches("GROUP_").to_owned()),
            home: self.home_team.to_domain(),
            away: self.away_team.to_domain(),
            score: self.score.to_domain(&status),
            status,
            kickoff,
            venue: None,
        })
    }

    fn lineups(&self) -> Vec<Lineup> {
        self.lineups.iter().map(FdLineup::to_domain).collect()
    }
}

fn fd_status(status: &str) -> MatchStatus {
    match status {
        "SCHEDULED" | "TIMED" => MatchStatus::Scheduled,
        "IN_PLAY" => MatchStatus::Live {
            minute: None,
            detail: None,
        },
        "PAUSED" => MatchStatus::HalfTime,
        "FINISHED" => MatchStatus::FullTime,
        "POSTPONED" => MatchStatus::Postponed,
        "CANCELLED" | "CANCELED" => MatchStatus::Canceled,
        _ => MatchStatus::Unknown,
    }
}

fn fd_stage(stage: &str) -> Stage {
    match stage {
        "LAST_32" => Stage::RoundOf32,
        "LAST_16" => Stage::RoundOf16,
        "QUARTER_FINALS" => Stage::QuarterFinal,
        "SEMI_FINALS" => Stage::SemiFinal,
        "THIRD_PLACE" => Stage::ThirdPlace,
        "FINAL" => Stage::Final,
        _ => Stage::GroupStage,
    }
}

#[derive(Debug, Deserialize)]
struct FdTeam {
    id: Option<i64>,
    name: Option<String>,
    tla: Option<String>,
    crest: Option<String>,
}
impl FdTeam {
    fn to_domain(&self) -> Team {
        let name = self.name.clone().unwrap_or_else(|| "TBD".to_owned());
        Team {
            id: self.id.map_or_else(String::new, |id| id.to_string()),
            abbreviation: self
                .tla
                .clone()
                .unwrap_or_else(|| name.chars().take(3).collect::<String>().to_uppercase()),
            country_code: self.tla.clone(),
            crest_url: self.crest.clone(),
            name,
        }
    }
}

#[derive(Debug, Deserialize)]
struct FdScore {
    full_time: FdGoals,
    regular_time: Option<FdGoals>,
    penalties: Option<FdGoals>,
}
impl FdScore {
    fn to_domain(&self, status: &MatchStatus) -> Option<Score> {
        if matches!(status, MatchStatus::Scheduled) {
            return None;
        }
        let goals = self.regular_time.as_ref().unwrap_or(&self.full_time);
        Some(Score {
            home: goals.home.unwrap_or_default().clamp(0, i64::from(u8::MAX)) as u8,
            away: goals.away.unwrap_or_default().clamp(0, i64::from(u8::MAX)) as u8,
            home_pens: self
                .penalties
                .as_ref()
                .and_then(|p| p.home)
                .map(|v| v.clamp(0, i64::from(u8::MAX)) as u8),
            away_pens: self
                .penalties
                .as_ref()
                .and_then(|p| p.away)
                .map(|v| v.clamp(0, i64::from(u8::MAX)) as u8),
        })
    }
}
#[derive(Debug, Deserialize)]
struct FdGoals {
    home: Option<i64>,
    away: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct FdStandingsResponse {
    standings: Vec<FdStanding>,
}
#[derive(Debug, Deserialize)]
struct FdStanding {
    group: Option<String>,
    table: Vec<FdStandingRow>,
}
impl FdStanding {
    fn into_group(self) -> Group {
        Group {
            name: self
                .group
                .map(|g| g.trim_start_matches("GROUP_").to_owned())
                .unwrap_or_default(),
            standings: self
                .table
                .into_iter()
                .map(FdStandingRow::into_domain)
                .collect(),
        }
    }
}
#[derive(Debug, Deserialize)]
struct FdStandingRow {
    position: u8,
    team: FdTeam,
    played_games: u8,
    won: u8,
    draw: u8,
    lost: u8,
    goals_for: u16,
    goals_against: u16,
    goal_difference: i16,
    points: u16,
}
impl FdStandingRow {
    fn into_domain(self) -> GroupStanding {
        GroupStanding {
            team: self.team.to_domain(),
            rank: self.position,
            played: self.played_games,
            won: self.won,
            drawn: self.draw,
            lost: self.lost,
            goals_for: self.goals_for,
            goals_against: self.goals_against,
            goal_diff: f64_to_i16(Some(f64::from(self.goal_difference))),
            points: self.points,
        }
    }
}

#[derive(Debug, Deserialize)]
struct FdLineup {
    team: FdTeam,
    formation: Option<String>,
    #[serde(default)]
    start_xi: Vec<FdPlayerSlot>,
    #[serde(default)]
    substitutes: Vec<FdPlayerSlot>,
}
impl FdLineup {
    fn to_domain(&self) -> Lineup {
        Lineup {
            team_id: self.team.id.map_or_else(String::new, |id| id.to_string()),
            formation: self.formation.clone(),
            starters: self.start_xi.iter().map(FdPlayerSlot::to_domain).collect(),
            substitutes: self
                .substitutes
                .iter()
                .map(FdPlayerSlot::to_domain)
                .collect(),
        }
    }
}
#[derive(Debug, Deserialize)]
struct FdPlayerSlot {
    name: Option<String>,
    shirt_number: Option<u8>,
    position: Option<String>,
}
impl FdPlayerSlot {
    fn to_domain(&self) -> crate::domain::Player {
        crate::domain::Player {
            name: self.name.clone().unwrap_or_default(),
            number: self.shirt_number,
            position: self.position.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DataError;

    #[test]
    fn maps_football_data_matches() -> Result<()> {
        let dto: FdMatches = serde_json::from_str(include_str!(
            "../../tests/fixtures/football_data_matches.json"
        ))
        .map_err(|e| DataError::Decode(e.to_string()))?;
        let matches = dto
            .matches
            .iter()
            .map(|m| m.to_match(&static_calendar()))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(matches[0].home.name, "Canada");
        assert_eq!(matches[0].status, MatchStatus::Scheduled);
        assert_eq!(matches[1].stage, Stage::RoundOf32);
        Ok(())
    }

    #[test]
    fn maps_football_data_standings() -> Result<()> {
        let dto: FdStandingsResponse = serde_json::from_str(include_str!(
            "../../tests/fixtures/football_data_standings.json"
        ))
        .map_err(|e| DataError::Decode(e.to_string()))?;
        let groups: Vec<_> = dto
            .standings
            .into_iter()
            .map(FdStanding::into_group)
            .collect();
        assert_eq!(groups[0].standings[0].team.name, "Mexico");
        assert_eq!(groups[0].standings[0].points, 3);
        Ok(())
    }
}
