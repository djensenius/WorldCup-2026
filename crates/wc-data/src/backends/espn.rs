//! ESPN hidden-API backend (default).
//!
//! Endpoints (base `https://site.api.espn.com`, league `soccer/fifa.world`):
//! - `/apis/site/v2/sports/soccer/fifa.world/scoreboard` — fixtures + live state
//! - `/apis/site/v2/sports/soccer/fifa.world/summary?event={id}` — match detail
//! - `/apis/v2/sports/soccer/fifa.world/standings` — group tables

use std::collections::HashMap;

use serde::Deserialize;
use time::{Date, macros::format_description};

use crate::backends::common::{
    f64_to_i16, f64_to_u8, f64_to_u16, group_from_text, minute_from_clock, parse_time,
    parse_u8_str, stage_for_date, stage_from_label,
};
use crate::domain::{
    Bracket, BracketRound, Calendar, Group, GroupStanding, Lineup, Match, MatchDetail, MatchEvent,
    MatchEventKind, MatchStatus, Player, Score, Stage, StageWindow, Team, TeamStat,
};
use crate::error::Result;
use crate::provider::ScoreProvider;
use crate::transport::Http;

const SCOREBOARD_URL: &str =
    "https://site.api.espn.com/apis/site/v2/sports/soccer/fifa.world/scoreboard";
const STANDINGS_URL: &str = "https://site.api.espn.com/apis/v2/sports/soccer/fifa.world/standings";
const SUMMARY_URL: &str =
    "https://site.api.espn.com/apis/site/v2/sports/soccer/fifa.world/summary?event=";

/// ESPN-backed provider.
#[derive(Debug, Clone)]
pub struct EspnProvider {
    http: Http,
}

impl EspnProvider {
    /// Build the ESPN provider over a shared HTTP client.
    #[must_use]
    pub fn new(http: Http) -> Self {
        Self { http }
    }

    async fn fetch_scoreboard(&self, day: Option<Date>) -> Result<EspnScoreboard> {
        let url = day.map_or_else(
            || SCOREBOARD_URL.to_owned(),
            |date| {
                let fmt = format_description!("[year][month][day]");
                let dates = date.format(fmt).unwrap_or_default();
                format!("{SCOREBOARD_URL}?dates={dates}")
            },
        );
        self.http.get_json(&url, &[]).await
    }
}

impl ScoreProvider for EspnProvider {
    fn name(&self) -> &'static str {
        "ESPN"
    }

    async fn calendar(&self) -> Result<Calendar> {
        self.fetch_scoreboard(None).await?.calendar()
    }

    async fn scoreboard(&self, day: Option<Date>) -> Result<Vec<Match>> {
        let dto = self.fetch_scoreboard(day).await?;
        let calendar = dto.calendar()?;
        dto.matches(&calendar)
    }

    async fn standings(&self) -> Result<Vec<Group>> {
        let dto: EspnStandings = self.http.get_json(STANDINGS_URL, &[]).await?;
        Ok(dto.groups())
    }

    async fn bracket(&self) -> Result<Bracket> {
        let dto = self.fetch_scoreboard(None).await?;
        let calendar = dto.calendar()?;
        let matches = dto.matches(&calendar)?;
        let mut rounds = Vec::new();
        for stage in Stage::knockout_order() {
            let round_matches = matches
                .iter()
                .filter(|m| m.stage == stage)
                .cloned()
                .collect();
            rounds.push(BracketRound {
                stage,
                matches: round_matches,
            });
        }
        Ok(Bracket { rounds })
    }

    async fn match_detail(&self, id: &str) -> Result<MatchDetail> {
        let url = format!("{SUMMARY_URL}{id}");
        let dto: EspnSummary = self.http.get_json(&url, &[]).await?;
        dto.detail()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnScoreboard {
    leagues: Vec<EspnLeague>,
    #[serde(default)]
    events: Vec<EspnEvent>,
}

impl EspnScoreboard {
    fn calendar(&self) -> Result<Calendar> {
        let stages = self
            .leagues
            .first()
            .into_iter()
            .flat_map(|league| &league.calendar)
            .flat_map(|entry| {
                if entry.entries.is_empty() {
                    vec![entry]
                } else {
                    entry.entries.iter().collect()
                }
            })
            .filter_map(|entry| {
                let stage = stage_from_label(&entry.label)?;
                Some((stage, entry))
            })
            .map(|(stage, entry)| {
                Ok(StageWindow {
                    stage,
                    label: entry.label.clone(),
                    start: parse_time(&entry.start_date)?,
                    end: parse_time(&entry.end_date)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Calendar { stages })
    }

    fn matches(&self, calendar: &Calendar) -> Result<Vec<Match>> {
        self.events
            .iter()
            .map(|event| event.to_match(calendar))
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnLeague {
    #[serde(default)]
    calendar: Vec<EspnCalendarEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnCalendarEntry {
    label: String,
    start_date: String,
    end_date: String,
    #[serde(default)]
    entries: Vec<EspnCalendarEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnEvent {
    id: String,
    date: String,
    name: Option<String>,
    #[serde(default)]
    competitions: Vec<EspnCompetition>,
}

impl EspnEvent {
    fn to_match(&self, calendar: &Calendar) -> Result<Match> {
        let kickoff = parse_time(&self.date)?;
        let competition = self.competitions.first();
        let home = competition
            .and_then(|c| c.competitor("home"))
            .map_or_else(|| Team::placeholder("TBD"), EspnCompetitor::team);
        let away = competition
            .and_then(|c| c.competitor("away"))
            .map_or_else(|| Team::placeholder("TBD"), EspnCompetitor::team);
        let score = competition.and_then(EspnCompetition::score);
        let status = competition.map_or(MatchStatus::Scheduled, EspnCompetition::status);
        let note = competition
            .and_then(|c| c.alt_game_note.as_deref())
            .or(self.name.as_deref());
        let fallback = stage_from_label(note.unwrap_or_default()).unwrap_or(Stage::GroupStage);
        Ok(Match {
            id: self.id.clone(),
            stage: stage_for_date(calendar, kickoff, fallback),
            group: group_from_text(note),
            home,
            away,
            score,
            status,
            kickoff,
            venue: competition
                .and_then(|c| c.venue.as_ref())
                .and_then(|v| v.full_name.clone()),
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnCompetition {
    date: Option<String>,
    #[serde(default)]
    competitors: Vec<EspnCompetitor>,
    status: Option<EspnStatus>,
    venue: Option<EspnVenue>,
    alt_game_note: Option<String>,
}

impl EspnCompetition {
    fn competitor(&self, home_away: &str) -> Option<&EspnCompetitor> {
        self.competitors
            .iter()
            .find(|c| c.home_away.as_deref() == Some(home_away))
    }

    fn score(&self) -> Option<Score> {
        let status = self.status.as_ref()?;
        if status
            .type_
            .as_ref()
            .is_some_and(|t| t.state.as_deref() == Some("pre"))
        {
            return None;
        }
        Some(Score {
            home: parse_u8_str(self.competitor("home").and_then(|c| c.score.as_deref()))
                .unwrap_or_default(),
            away: parse_u8_str(self.competitor("away").and_then(|c| c.score.as_deref()))
                .unwrap_or_default(),
            home_pens: None,
            away_pens: None,
        })
    }

    fn status(&self) -> MatchStatus {
        self.status
            .as_ref()
            .map_or(MatchStatus::Unknown, EspnStatus::to_domain)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnCompetitor {
    home_away: Option<String>,
    score: Option<String>,
    team: Option<EspnTeam>,
}

impl EspnCompetitor {
    fn team(&self) -> Team {
        self.team
            .as_ref()
            .map_or_else(|| Team::placeholder("TBD"), EspnTeam::to_domain)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnTeam {
    id: Option<String>,
    display_name: Option<String>,
    abbreviation: Option<String>,
    logo: Option<String>,
    #[serde(default)]
    logos: Vec<EspnLogo>,
}

impl EspnTeam {
    fn to_domain(&self) -> Team {
        let name = self
            .display_name
            .clone()
            .unwrap_or_else(|| "TBD".to_owned());
        let abbreviation = self
            .abbreviation
            .clone()
            .unwrap_or_else(|| name.chars().take(3).collect::<String>().to_uppercase());
        Team {
            id: self.id.clone().unwrap_or_default(),
            name,
            abbreviation,
            country_code: self.abbreviation.clone(),
            crest_url: self
                .logo
                .clone()
                .or_else(|| self.logos.first().map(|logo| logo.href.clone())),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct EspnLogo {
    href: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnVenue {
    full_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct EspnStatus {
    #[serde(rename = "type")]
    type_: Option<EspnStatusType>,
    display_clock: Option<String>,
}

impl EspnStatus {
    fn to_domain(&self) -> MatchStatus {
        let detail = self.type_.as_ref().and_then(|t| t.detail.clone());
        match self.type_.as_ref().and_then(|t| t.state.as_deref()) {
            Some("pre") => MatchStatus::Scheduled,
            Some("in") => {
                let desc = self
                    .type_
                    .as_ref()
                    .and_then(|t| t.description.as_deref())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if desc.contains("half") && desc.contains("time") {
                    MatchStatus::HalfTime
                } else {
                    MatchStatus::Live {
                        minute: minute_from_clock(self.display_clock.as_deref())
                            .or_else(|| minute_from_clock(detail.as_deref())),
                        detail,
                    }
                }
            }
            Some("post") => {
                let desc = self
                    .type_
                    .as_ref()
                    .and_then(|t| t.description.as_deref())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if desc.contains("pen") {
                    MatchStatus::Penalties
                } else if desc.contains("extra") {
                    MatchStatus::AfterExtraTime
                } else {
                    MatchStatus::FullTime
                }
            }
            _ => MatchStatus::Unknown,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct EspnStatusType {
    state: Option<String>,
    description: Option<String>,
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EspnStandings {
    #[serde(default)]
    children: Vec<EspnStandingChild>,
}

impl EspnStandings {
    fn groups(&self) -> Vec<Group> {
        self.children
            .iter()
            .map(|child| Group {
                name: child
                    .name
                    .strip_prefix("Group ")
                    .unwrap_or(&child.name)
                    .to_owned(),
                standings: child
                    .standings
                    .entries
                    .iter()
                    .map(EspnStandingEntry::to_domain)
                    .collect(),
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct EspnStandingChild {
    name: String,
    standings: EspnStandingEntries,
}

#[derive(Debug, Deserialize)]
struct EspnStandingEntries {
    #[serde(default)]
    entries: Vec<EspnStandingEntry>,
}

#[derive(Debug, Deserialize)]
struct EspnStandingEntry {
    team: EspnTeam,
    #[serde(default)]
    stats: Vec<EspnStat>,
}

impl EspnStandingEntry {
    fn stat(&self, names: &[&str]) -> Option<f64> {
        self.stats
            .iter()
            .find(|s| {
                names
                    .iter()
                    .any(|name| s.name == *name || s.type_.as_deref() == Some(*name))
            })
            .and_then(|s| s.value)
    }

    fn to_domain(&self) -> GroupStanding {
        GroupStanding {
            team: self.team.to_domain(),
            rank: f64_to_u8(self.stat(&["rank"])),
            played: f64_to_u8(self.stat(&["gamesPlayed", "gamesplayed"])),
            won: f64_to_u8(self.stat(&["wins"])),
            drawn: f64_to_u8(self.stat(&["ties"])),
            lost: f64_to_u8(self.stat(&["losses"])),
            goals_for: f64_to_u16(self.stat(&["pointsFor", "pointsfor"])),
            goals_against: f64_to_u16(self.stat(&["pointsAgainst", "pointsagainst"])),
            goal_diff: f64_to_i16(self.stat(&["pointDifferential", "pointdifferential"])),
            points: f64_to_u16(self.stat(&["points"])),
        }
    }
}

#[derive(Debug, Deserialize)]
struct EspnStat {
    name: String,
    #[serde(rename = "type")]
    type_: Option<String>,
    value: Option<f64>,
    #[serde(default, rename = "displayValue")]
    display_value: String,
    #[serde(default)]
    label: String,
}

#[derive(Debug, Deserialize)]
struct EspnSummary {
    header: EspnHeader,
    #[serde(default, rename = "keyEvents")]
    key_events: Vec<EspnKeyEvent>,
    #[serde(default)]
    rosters: Vec<EspnRoster>,
    boxscore: Option<EspnBoxscore>,
}

impl EspnSummary {
    fn detail(&self) -> Result<MatchDetail> {
        let calendar = Calendar::default();
        let event = self.header.to_event();
        let summary = event.to_match(&calendar)?;
        Ok(MatchDetail {
            summary,
            events: self
                .key_events
                .iter()
                .filter_map(EspnKeyEvent::to_domain)
                .collect(),
            lineups: self.rosters.iter().map(EspnRoster::to_domain).collect(),
            stats: self
                .boxscore
                .as_ref()
                .map_or_else(Vec::new, EspnBoxscore::team_stats),
        })
    }
}

#[derive(Debug, Deserialize)]
struct EspnHeader {
    id: String,
    competitions: Vec<EspnCompetition>,
}

impl EspnHeader {
    fn to_event(&self) -> EspnEvent {
        let date = self
            .competitions
            .first()
            .and_then(|c| c.date.clone())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned());
        EspnEvent {
            id: self.id.clone(),
            date,
            name: None,
            competitions: self.competitions.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct EspnKeyEvent {
    #[serde(rename = "type")]
    type_: Option<EspnEventType>,
    text: Option<String>,
    clock: Option<EspnClock>,
    team: Option<EspnEventTeam>,
    #[serde(default)]
    participants: Vec<EspnParticipant>,
}

impl EspnKeyEvent {
    fn to_domain(&self) -> Option<MatchEvent> {
        let kind = event_kind(
            self.type_
                .as_ref()
                .and_then(|t| t.text.as_deref())
                .or_else(|| self.type_.as_ref().and_then(|t| t.type_.as_deref()))?,
        );
        let player = self
            .participants
            .first()
            .and_then(|p| p.athlete.display_name.clone());
        Some(MatchEvent {
            minute: self.clock.as_ref().and_then(EspnClock::minute),
            stoppage: self.clock.as_ref().and_then(EspnClock::stoppage),
            kind,
            team_id: self.team.as_ref().and_then(|t| t.id.clone()),
            player,
            detail: self.text.clone(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct EspnEventType {
    text: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EspnClock {
    value: Option<f64>,
    #[serde(rename = "displayValue")]
    display_value: Option<String>,
}

impl EspnClock {
    fn minute(&self) -> Option<u16> {
        minute_from_clock(self.display_value.as_deref())
            .or_else(|| self.value.map(|v| (v / 60.0).floor() as u16))
    }

    fn stoppage(&self) -> Option<u16> {
        let text = self.display_value.as_deref()?;
        let plus = text.find('+')?;
        let digits: String = text[plus + 1..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse().ok()
    }
}

#[derive(Debug, Deserialize)]
struct EspnEventTeam {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EspnParticipant {
    athlete: EspnAthlete,
}

#[derive(Debug, Deserialize)]
struct EspnAthlete {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "fullName")]
    full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EspnRoster {
    team: EspnTeam,
    #[serde(default)]
    roster: Vec<EspnRosterPlayer>,
    formation: Option<String>,
}

impl EspnRoster {
    fn to_domain(&self) -> Lineup {
        let team = self.team.to_domain();
        let mut starters = Vec::new();
        let mut substitutes = Vec::new();
        for player in &self.roster {
            if player.starter.unwrap_or_default() {
                starters.push(player.to_domain());
            } else {
                substitutes.push(player.to_domain());
            }
        }
        Lineup {
            team_id: team.id,
            formation: self.formation.clone(),
            starters,
            substitutes,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EspnRosterPlayer {
    starter: Option<bool>,
    jersey: Option<String>,
    athlete: EspnAthlete,
    position: Option<EspnPosition>,
}

impl EspnRosterPlayer {
    fn to_domain(&self) -> Player {
        Player {
            name: self
                .athlete
                .display_name
                .clone()
                .or_else(|| self.athlete.full_name.clone())
                .unwrap_or_default(),
            number: parse_u8_str(self.jersey.as_deref()),
            position: self.position.as_ref().and_then(|p| p.abbreviation.clone()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct EspnPosition {
    abbreviation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EspnBoxscore {
    #[serde(default)]
    teams: Vec<EspnBoxTeam>,
}

impl EspnBoxscore {
    fn team_stats(&self) -> Vec<TeamStat> {
        if self.teams.len() < 2 {
            return Vec::new();
        }
        let home = self
            .teams
            .iter()
            .find(|t| t.home_away.as_deref() == Some("home"))
            .unwrap_or(&self.teams[0]);
        let away = self
            .teams
            .iter()
            .find(|t| t.home_away.as_deref() == Some("away"))
            .unwrap_or(&self.teams[1]);
        let away_by_name: HashMap<&str, &EspnStat> = away
            .statistics
            .iter()
            .map(|s| (s.name.as_str(), s))
            .collect();
        home.statistics
            .iter()
            .filter_map(|stat| {
                let away_stat = away_by_name.get(stat.name.as_str())?;
                Some(TeamStat {
                    label: if stat.label.is_empty() {
                        stat.name.clone()
                    } else {
                        stat.label.clone()
                    },
                    home: stat.display_value.clone(),
                    away: away_stat.display_value.clone(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct EspnBoxTeam {
    #[serde(rename = "homeAway")]
    home_away: Option<String>,
    #[serde(default)]
    statistics: Vec<EspnStat>,
}

fn event_kind(text: &str) -> MatchEventKind {
    let lower = text.to_ascii_lowercase();
    if lower.contains("own") {
        MatchEventKind::OwnGoal
    } else if lower.contains("penalty") && lower.contains("miss") {
        MatchEventKind::PenaltyMiss
    } else if lower.contains("penalty") {
        MatchEventKind::PenaltyGoal
    } else if lower.contains("goal") {
        MatchEventKind::Goal
    } else if lower.contains("second") && lower.contains("yellow") {
        MatchEventKind::SecondYellow
    } else if lower.contains("yellow") {
        MatchEventKind::YellowCard
    } else if lower.contains("red") {
        MatchEventKind::RedCard
    } else if lower.contains("sub") {
        MatchEventKind::Substitution
    } else if lower.contains("var") {
        MatchEventKind::Var
    } else {
        MatchEventKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DataError;

    #[test]
    fn maps_scoreboard_fixture() -> Result<()> {
        let dto: EspnScoreboard =
            serde_json::from_str(include_str!("../../tests/fixtures/espn_scoreboard.json"))
                .map_err(|e| DataError::Decode(e.to_string()))?;
        let calendar = dto.calendar()?;
        let matches = dto.matches(&calendar)?;
        assert_eq!(calendar.stages.len(), 7);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].home.name, "Canada");
        assert!(matches[0].status.is_live());
        assert_eq!(matches[0].group.as_deref(), Some("B"));
        Ok(())
    }

    #[test]
    fn maps_standings_fixture() -> Result<()> {
        let dto: EspnStandings =
            serde_json::from_str(include_str!("../../tests/fixtures/espn_standings.json"))
                .map_err(|e| DataError::Decode(e.to_string()))?;
        let groups = dto.groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].standings[0].team.name, "Mexico");
        assert_eq!(groups[0].standings[0].points, 3);
        Ok(())
    }

    #[test]
    fn maps_summary_fixture() -> Result<()> {
        let dto: EspnSummary =
            serde_json::from_str(include_str!("../../tests/fixtures/espn_summary.json"))
                .map_err(|e| DataError::Decode(e.to_string()))?;
        let detail = dto.detail()?;
        assert_eq!(detail.events[0].kind, MatchEventKind::YellowCard);
        assert_eq!(
            detail.events[0].player.as_deref(),
            Some("Alistair Johnston")
        );
        assert_eq!(detail.lineups.len(), 2);
        assert!(!detail.stats.is_empty());
        Ok(())
    }
}
