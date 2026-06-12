//! Normalized, provider-agnostic domain model for the World Cup.
//!
//! Every backend maps its upstream representation into these types so the TUI
//! and the rest of the app depend only on this module, never on a specific data
//! source. Times are always stored in UTC ([`OffsetDateTime`]); the UI converts
//! to the user's local zone for display.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A stage of the tournament. The 2026 format is a 48-team group stage (12
/// groups of 4) followed by a 32-team knockout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Group stage (groups A–L).
    GroupStage,
    /// Round of 32 (first knockout round in the 2026 format).
    RoundOf32,
    /// Round of 16.
    RoundOf16,
    /// Quarter-final.
    QuarterFinal,
    /// Semi-final.
    SemiFinal,
    /// Third-place play-off.
    ThirdPlace,
    /// Final.
    Final,
}

impl Stage {
    /// All knockout rounds in bracket order (excludes the group stage).
    #[must_use]
    pub fn knockout_order() -> [Stage; 6] {
        [
            Stage::RoundOf32,
            Stage::RoundOf16,
            Stage::QuarterFinal,
            Stage::SemiFinal,
            Stage::ThirdPlace,
            Stage::Final,
        ]
    }

    /// Whether this stage is part of the knockout bracket.
    #[must_use]
    pub fn is_knockout(self) -> bool {
        !matches!(self, Stage::GroupStage)
    }

    /// A short human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Stage::GroupStage => "Group Stage",
            Stage::RoundOf32 => "Round of 32",
            Stage::RoundOf16 => "Round of 16",
            Stage::QuarterFinal => "Quarter-final",
            Stage::SemiFinal => "Semi-final",
            Stage::ThirdPlace => "Third place",
            Stage::Final => "Final",
        }
    }
}

/// A national team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    /// Provider-specific identifier (opaque to the UI).
    pub id: String,
    /// Display name, e.g. "Canada".
    pub name: String,
    /// Short code, e.g. "CAN".
    pub abbreviation: String,
    /// ISO-ish country code where available.
    pub country_code: Option<String>,
    /// URL to a crest/flag image, where available.
    pub crest_url: Option<String>,
}

impl Team {
    /// A placeholder team for not-yet-decided bracket slots.
    #[must_use]
    pub fn placeholder(label: impl Into<String>) -> Self {
        let name = label.into();
        Self {
            id: String::new(),
            abbreviation: name.chars().take(3).collect::<String>().to_uppercase(),
            name,
            country_code: None,
            crest_url: None,
        }
    }

    /// Whether this is a placeholder (unknown) team.
    #[must_use]
    pub fn is_placeholder(&self) -> bool {
        self.id.is_empty()
    }
}

/// The live/finished state of a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    /// Not started yet.
    Scheduled,
    /// In play. `minute` is the displayed clock minute when known.
    Live {
        /// Current match minute, when reported.
        minute: Option<u16>,
        /// Optional period detail, e.g. "1st Half", "ET".
        detail: Option<String>,
    },
    /// Half-time interval.
    HalfTime,
    /// Finished in regulation.
    FullTime,
    /// Finished after extra time.
    AfterExtraTime,
    /// Decided on penalties.
    Penalties,
    /// Postponed.
    Postponed,
    /// Cancelled.
    Canceled,
    /// Unknown / unmapped status.
    Unknown,
}

impl MatchStatus {
    /// Whether the match is currently being played (including half-time).
    #[must_use]
    pub fn is_live(&self) -> bool {
        matches!(self, MatchStatus::Live { .. } | MatchStatus::HalfTime)
    }

    /// Whether the match has finished by any means.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            MatchStatus::FullTime | MatchStatus::AfterExtraTime | MatchStatus::Penalties
        )
    }
}

/// The score of a match, including a penalty-shootout tally when applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Score {
    /// Home goals.
    pub home: u8,
    /// Away goals.
    pub away: u8,
    /// Home penalty-shootout goals, when the match went to penalties.
    pub home_pens: Option<u8>,
    /// Away penalty-shootout goals, when the match went to penalties.
    pub away_pens: Option<u8>,
}

/// A single fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    /// Provider-specific identifier used to fetch detail.
    pub id: String,
    /// Tournament stage.
    pub stage: Stage,
    /// Group letter ("A".."L") for group-stage matches.
    pub group: Option<String>,
    /// Home team.
    pub home: Team,
    /// Away team.
    pub away: Team,
    /// Current score, if the match has started.
    pub score: Option<Score>,
    /// Match status.
    pub status: MatchStatus,
    /// Kickoff time in UTC.
    #[serde(with = "time::serde::rfc3339")]
    pub kickoff: OffsetDateTime,
    /// Venue name, where available.
    pub venue: Option<String>,
}

/// One team's row in a group table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupStanding {
    /// The team this row describes.
    pub team: Team,
    /// 1-based rank within the group.
    pub rank: u8,
    /// Matches played.
    pub played: u8,
    /// Wins.
    pub won: u8,
    /// Draws.
    pub drawn: u8,
    /// Losses.
    pub lost: u8,
    /// Goals scored.
    pub goals_for: u16,
    /// Goals conceded.
    pub goals_against: u16,
    /// Goal difference (`goals_for - goals_against`).
    pub goal_diff: i16,
    /// Points.
    pub points: u16,
}

/// A group table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// Group name/letter, e.g. "A".
    pub name: String,
    /// Standings, already sorted by rank.
    pub standings: Vec<GroupStanding>,
}

/// The kind of in-match event on a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchEventKind {
    /// A goal from open play.
    Goal,
    /// An own goal.
    OwnGoal,
    /// A goal from a penalty.
    PenaltyGoal,
    /// A missed/saved penalty.
    PenaltyMiss,
    /// A yellow card.
    YellowCard,
    /// A second yellow (booking leading to a red).
    SecondYellow,
    /// A straight red card.
    RedCard,
    /// A substitution.
    Substitution,
    /// A VAR decision.
    Var,
    /// Anything else.
    Other,
}

/// A single timeline event in a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchEvent {
    /// Match minute, when known.
    pub minute: Option<u16>,
    /// Added/stoppage-time minutes beyond `minute`, when known.
    pub stoppage: Option<u16>,
    /// What happened.
    pub kind: MatchEventKind,
    /// The team this event belongs to (provider team id), when known.
    pub team_id: Option<String>,
    /// Primary player involved (scorer, booked player, player coming on).
    pub player: Option<String>,
    /// Free-form detail (assist, reason, player going off, etc.).
    pub detail: Option<String>,
}

/// A player in a lineup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    /// Player name.
    pub name: String,
    /// Shirt number, when known.
    pub number: Option<u8>,
    /// Position abbreviation, when known.
    pub position: Option<String>,
}

/// A team's lineup for a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineup {
    /// Provider team id this lineup belongs to.
    pub team_id: String,
    /// Formation string, e.g. "4-3-3", when known.
    pub formation: Option<String>,
    /// Starting XI.
    pub starters: Vec<Player>,
    /// Substitutes.
    pub substitutes: Vec<Player>,
}

/// A single comparable team statistic (e.g. possession, shots).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamStat {
    /// Stat label, e.g. "Possession".
    pub label: String,
    /// Home value, formatted for display (e.g. "57%").
    pub home: String,
    /// Away value, formatted for display.
    pub away: String,
}

/// Full detail for a single match: the fixture plus timeline, lineups, stats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchDetail {
    /// The fixture summary (teams, score, status).
    pub summary: Match,
    /// Timeline events, ordered chronologically.
    pub events: Vec<MatchEvent>,
    /// Lineups, typically one per team when available.
    pub lineups: Vec<Lineup>,
    /// Comparable team statistics.
    pub stats: Vec<TeamStat>,
}

/// One round of the knockout bracket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BracketRound {
    /// The stage this round represents.
    pub stage: Stage,
    /// Matches in this round, in bracket order.
    pub matches: Vec<Match>,
}

/// The knockout bracket as an ordered list of rounds.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Bracket {
    /// Rounds from [`Stage::RoundOf32`] through [`Stage::Final`].
    pub rounds: Vec<BracketRound>,
}

/// A scheduling window for a stage, from the competition calendar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageWindow {
    /// The stage.
    pub stage: Stage,
    /// Provider label, e.g. "Round of 32".
    pub label: String,
    /// Window start (UTC).
    #[serde(with = "time::serde::rfc3339")]
    pub start: OffsetDateTime,
    /// Window end (UTC).
    #[serde(with = "time::serde::rfc3339")]
    pub end: OffsetDateTime,
}

/// The competition calendar: the set of stage windows.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Calendar {
    /// Stage windows, in chronological order.
    pub stages: Vec<StageWindow>,
}
