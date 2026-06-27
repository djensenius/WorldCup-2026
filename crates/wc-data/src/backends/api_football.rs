//! API-Football (`api-sports.io`) backend.
//!
//! Auth: header `x-apisports-key: <key>`. Base `https://v3.football.api-sports.io`.
//! Endpoints used: `/fixtures`, `/standings`, `/fixtures/events`,
//! `/fixtures/lineups`, `/fixtures/statistics`.

use serde::Deserialize;
use time::{Date, Month, OffsetDateTime, Time};

use crate::backends::common::{
    api_status, day_bounds, f64_to_i16, f64_to_u16, group_from_text, parse_time,
};
use crate::domain::{
    Bracket, BracketRound, Calendar, Group, GroupStanding, Lineup, Match, MatchDetail, MatchEvent,
    MatchEventKind, MatchStatus, Player, Score, Stage, StageWindow, Team, TeamStat,
};
use crate::error::{DataError, Result};
use crate::provider::ScoreProvider;
use crate::transport::Http;

const BASE_URL: &str = "https://v3.football.api-sports.io";
const WORLD_CUP_LEAGUE: u16 = 1;
const WORLD_CUP_SEASON: u16 = 2026;

/// API-Football-backed provider.
#[derive(Debug, Clone)]
pub struct ApiFootballProvider {
    http: Http,
    key: String,
}

impl ApiFootballProvider {
    /// Build the provider over a shared HTTP client with the given API key.
    #[must_use]
    pub fn new(http: Http, key: String) -> Self {
        Self { http, key }
    }

    fn headers(&self) -> [(&str, &str); 1] {
        [("x-apisports-key", self.key.as_str())]
    }

    async fn fetch<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.http
            .get_json(&format!("{BASE_URL}{path}"), &self.headers())
            .await
    }
}

impl ScoreProvider for ApiFootballProvider {
    fn name(&self) -> &'static str {
        "API-Football"
    }

    async fn calendar(&self) -> Result<Calendar> {
        Ok(static_calendar())
    }

    async fn scoreboard(&self, day: Option<Date>) -> Result<Vec<Match>> {
        // NOTE: API-Football documents FIFA World Cup as league=1; the 2026 tournament uses season=2026.
        let dto: ApiResponse<ApiFixture> = self
            .fetch(&format!(
                "/fixtures?league={WORLD_CUP_LEAGUE}&season={WORLD_CUP_SEASON}"
            ))
            .await?;
        let calendar = static_calendar();
        let mut matches = dto
            .response
            .into_iter()
            .map(|fixture| fixture.into_match(&calendar))
            .collect::<Result<Vec<_>>>()?;
        if let Some(day) = day {
            let (start, end) = day_bounds(day);
            matches.retain(|m| m.kickoff >= start && m.kickoff < end);
        }
        Ok(matches)
    }

    async fn standings(&self) -> Result<Vec<Group>> {
        let dto: ApiResponse<ApiStandingLeagueEnvelope> = self
            .fetch(&format!(
                "/standings?league={WORLD_CUP_LEAGUE}&season={WORLD_CUP_SEASON}"
            ))
            .await?;
        Ok(dto
            .response
            .into_iter()
            .flat_map(|r| {
                r.league.standings.into_iter().map(|table| Group {
                    name: table
                        .first()
                        .and_then(|row| group_from_text(row.group.as_deref()))
                        .unwrap_or_default(),
                    standings: table.into_iter().map(ApiStanding::into_domain).collect(),
                })
            })
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
        let fixtures: ApiResponse<ApiFixture> = self.fetch(&format!("/fixtures?id={id}")).await?;
        let summary = fixtures
            .response
            .into_iter()
            .next()
            .ok_or_else(|| DataError::Decode(format!("API-Football fixture {id} not found")))?
            .into_match(&static_calendar())?;
        let events: ApiResponse<ApiEvent> = self
            .fetch(&format!("/fixtures/events?fixture={id}"))
            .await?;
        let lineups: ApiResponse<ApiLineup> = self
            .fetch(&format!("/fixtures/lineups?fixture={id}"))
            .await?;
        let stats: ApiResponse<ApiStatisticTeam> = self
            .fetch(&format!("/fixtures/statistics?fixture={id}"))
            .await?;
        Ok(MatchDetail {
            summary,
            events: events
                .response
                .into_iter()
                .map(ApiEvent::into_domain)
                .collect(),
            lineups: lineups
                .response
                .into_iter()
                .map(ApiLineup::into_domain)
                .collect(),
            stats: api_team_stats(&stats.response),
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
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct ApiResponse<T> {
    #[serde(default)]
    response: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct ApiFixture {
    fixture: FixtureInfo,
    league: ApiLeague,
    teams: ApiTeams,
    goals: ApiGoals,
    score: ApiScore,
}

impl ApiFixture {
    fn into_match(self, calendar: &Calendar) -> Result<Match> {
        let kickoff = parse_time(&self.fixture.date)?;
        let (venue, location) = match self.fixture.venue {
            Some(v) => (v.name, v.city.filter(|s| !s.is_empty())),
            None => (None, None),
        };
        let status = api_status(
            &self.fixture.status.short,
            self.fixture.status.elapsed,
            self.fixture.status.long.clone(),
        );
        let score = if matches!(status, MatchStatus::Scheduled) {
            None
        } else {
            Some(Score {
                home: self
                    .goals
                    .home
                    .unwrap_or_default()
                    .clamp(0, i64::from(u8::MAX)) as u8,
                away: self
                    .goals
                    .away
                    .unwrap_or_default()
                    .clamp(0, i64::from(u8::MAX)) as u8,
                home_pens: self
                    .score
                    .penalty
                    .home
                    .map(|v| v.clamp(0, i64::from(u8::MAX)) as u8),
                away_pens: self
                    .score
                    .penalty
                    .away
                    .map(|v| v.clamp(0, i64::from(u8::MAX)) as u8),
            })
        };
        Ok(Match {
            id: self.fixture.id.to_string(),
            stage: crate::backends::common::stage_for_date(
                calendar,
                kickoff,
                stage_from_round(self.league.round.as_deref()),
            ),
            group: group_from_text(self.league.round.as_deref()),
            home: self.teams.home.to_domain(),
            away: self.teams.away.to_domain(),
            score,
            status,
            kickoff,
            venue,
            location,
        })
    }
}

fn stage_from_round(round: Option<&str>) -> Stage {
    let text = round.unwrap_or_default().to_ascii_lowercase();
    if text.contains("round of 32") {
        Stage::RoundOf32
    } else if text.contains("round of 16") {
        Stage::RoundOf16
    } else if text.contains("quarter") {
        Stage::QuarterFinal
    } else if text.contains("semi") {
        Stage::SemiFinal
    } else if text.contains("third") {
        Stage::ThirdPlace
    } else if text.contains("final") {
        Stage::Final
    } else {
        Stage::GroupStage
    }
}

#[derive(Debug, Deserialize)]
struct FixtureInfo {
    id: i64,
    date: String,
    status: ApiStatus,
    venue: Option<ApiVenue>,
}
#[derive(Debug, Deserialize)]
struct ApiStatus {
    long: Option<String>,
    short: String,
    elapsed: Option<u16>,
}
#[derive(Debug, Deserialize)]
struct ApiVenue {
    name: Option<String>,
    city: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ApiLeague {
    round: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ApiTeams {
    home: ApiTeam,
    away: ApiTeam,
}
#[derive(Debug, Deserialize)]
struct ApiTeam {
    id: Option<i64>,
    name: Option<String>,
    code: Option<String>,
    logo: Option<String>,
}
impl ApiTeam {
    fn to_domain(&self) -> Team {
        let name = self.name.clone().unwrap_or_else(|| "TBD".to_owned());
        Team {
            id: self.id.map_or_else(String::new, |id| id.to_string()),
            abbreviation: self
                .code
                .clone()
                .unwrap_or_else(|| name.chars().take(3).collect::<String>().to_uppercase()),
            country_code: self.code.clone(),
            crest_url: self.logo.clone(),
            name,
        }
    }
}
#[derive(Debug, Deserialize)]
struct ApiGoals {
    home: Option<i64>,
    away: Option<i64>,
}
#[derive(Debug, Deserialize)]
struct ApiScore {
    penalty: ApiGoals,
}

#[derive(Debug, Deserialize)]
struct ApiStandingLeagueEnvelope {
    league: ApiStandingLeague,
}
#[derive(Debug, Deserialize)]
struct ApiStandingLeague {
    standings: Vec<Vec<ApiStanding>>,
}
#[derive(Debug, Deserialize)]
struct ApiStanding {
    rank: u8,
    team: ApiTeam,
    group: Option<String>,
    all: ApiStandingAll,
    goals_diff: i16,
    points: Option<u16>,
}
impl ApiStanding {
    fn into_domain(self) -> GroupStanding {
        GroupStanding {
            team: self.team.to_domain(),
            rank: self.rank,
            played: self.all.played,
            won: self.all.win,
            drawn: self.all.draw,
            lost: self.all.lose,
            goals_for: f64_to_u16(Some(f64::from(self.all.goals.for_))),
            goals_against: f64_to_u16(Some(f64::from(self.all.goals.against))),
            goal_diff: f64_to_i16(Some(f64::from(self.goals_diff))),
            points: self.points.unwrap_or_default(),
        }
    }
}
#[derive(Debug, Deserialize)]
struct ApiStandingAll {
    played: u8,
    win: u8,
    draw: u8,
    lose: u8,
    goals: ApiStandingGoals,
}
#[derive(Debug, Deserialize)]
struct ApiStandingGoals {
    #[serde(rename = "for")]
    for_: u16,
    against: u16,
}

#[derive(Debug, Deserialize)]
struct ApiEvent {
    time: ApiEventTime,
    team: ApiTeam,
    player: Option<ApiPerson>,
    assist: Option<ApiPerson>,
    #[serde(rename = "type")]
    type_: String,
    detail: Option<String>,
    comments: Option<String>,
}
impl ApiEvent {
    fn into_domain(self) -> MatchEvent {
        MatchEvent {
            minute: self.time.elapsed,
            stoppage: self.time.extra,
            kind: api_event_kind(&self.type_, self.detail.as_deref()),
            team_id: self.team.id.map(|id| id.to_string()),
            player: self.player.and_then(|p| p.name),
            detail: self
                .comments
                .or_else(|| self.assist.and_then(|p| p.name))
                .or(self.detail),
        }
    }
}
#[derive(Debug, Deserialize)]
struct ApiEventTime {
    elapsed: Option<u16>,
    extra: Option<u16>,
}
#[derive(Debug, Deserialize)]
struct ApiPerson {
    name: Option<String>,
}
fn api_event_kind(type_: &str, detail: Option<&str>) -> MatchEventKind {
    let text = format!("{} {}", type_, detail.unwrap_or_default()).to_ascii_lowercase();
    if text.contains("own") {
        MatchEventKind::OwnGoal
    } else if text.contains("missed") {
        MatchEventKind::PenaltyMiss
    } else if text.contains("penalty") {
        MatchEventKind::PenaltyGoal
    } else if text.contains("goal") {
        MatchEventKind::Goal
    } else if text.contains("yellow") {
        MatchEventKind::YellowCard
    } else if text.contains("red") {
        MatchEventKind::RedCard
    } else if text.contains("sub") {
        MatchEventKind::Substitution
    } else if text.contains("var") {
        MatchEventKind::Var
    } else {
        MatchEventKind::Other
    }
}

#[derive(Debug, Deserialize)]
struct ApiLineup {
    team: ApiTeam,
    formation: Option<String>,
    #[serde(default)]
    start_xi: Vec<ApiLineupSlot>,
    #[serde(default)]
    substitutes: Vec<ApiLineupSlot>,
}
impl ApiLineup {
    fn into_domain(self) -> Lineup {
        Lineup {
            team_id: self.team.id.map_or_else(String::new, |id| id.to_string()),
            formation: self.formation,
            starters: self
                .start_xi
                .into_iter()
                .map(ApiLineupSlot::into_player)
                .collect(),
            substitutes: self
                .substitutes
                .into_iter()
                .map(ApiLineupSlot::into_player)
                .collect(),
        }
    }
}
#[derive(Debug, Deserialize)]
struct ApiLineupSlot {
    player: ApiLineupPlayer,
}
impl ApiLineupSlot {
    fn into_player(self) -> Player {
        Player {
            name: self.player.name.unwrap_or_default(),
            number: self
                .player
                .number
                .map(|n| n.clamp(0, i64::from(u8::MAX)) as u8),
            position: self.player.pos,
        }
    }
}
#[derive(Debug, Deserialize)]
struct ApiLineupPlayer {
    name: Option<String>,
    number: Option<i64>,
    pos: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiStatisticTeam {
    #[serde(default)]
    statistics: Vec<ApiStatistic>,
}
#[derive(Debug, Deserialize)]
struct ApiStatistic {
    #[serde(rename = "type")]
    type_: String,
    value: Option<serde_json::Value>,
}
fn api_team_stats(teams: &[ApiStatisticTeam]) -> Vec<TeamStat> {
    if teams.len() < 2 {
        return Vec::new();
    }
    teams[0]
        .statistics
        .iter()
        .zip(&teams[1].statistics)
        .map(|(home, away)| TeamStat {
            label: home.type_.clone(),
            home: stat_value(home.value.as_ref()),
            away: stat_value(away.value.as_ref()),
        })
        .collect()
}
fn stat_value(value: Option<&serde_json::Value>) -> String {
    value.map_or_else(String::new, |v| {
        v.as_str().map_or_else(|| v.to_string(), ToOwned::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_api_football_fixture() -> Result<()> {
        let dto: ApiResponse<ApiFixture> = serde_json::from_str(include_str!(
            "../../tests/fixtures/apifootball_scoreboard.json"
        ))
        .map_err(|e| DataError::Decode(e.to_string()))?;
        let matches = dto
            .response
            .into_iter()
            .map(|f| f.into_match(&static_calendar()))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(matches[0].home.name, "Canada");
        assert_eq!(matches[0].status, MatchStatus::Scheduled);
        assert_eq!(matches[1].stage, Stage::RoundOf32);
        Ok(())
    }

    #[test]
    fn maps_api_football_events() -> Result<()> {
        let dto: ApiResponse<ApiEvent> =
            serde_json::from_str(include_str!("../../tests/fixtures/apifootball_events.json"))
                .map_err(|e| DataError::Decode(e.to_string()))?;
        let event = dto
            .response
            .into_iter()
            .next()
            .ok_or_else(|| DataError::Decode("missing event".to_owned()))?
            .into_domain();
        assert_eq!(event.kind, MatchEventKind::PenaltyGoal);
        assert_eq!(event.player.as_deref(), Some("Alphonso Davies"));
        Ok(())
    }
}
