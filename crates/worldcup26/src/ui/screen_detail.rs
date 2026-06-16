//! Match detail overlay.
//!
//! Opened from the Matches or Live screens for a specific fixture. Shows the
//! timeline (goals/cards/subs), lineups, and team statistics. `Esc` returns to
//! the previous screen (handled by the app); `j`/`k` scroll.

use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use wc_data::domain::{
    Lineup, Match, MatchDetail, MatchEvent, MatchEventKind, MatchStatus, Player, Team, TeamStat,
};

use crate::app::App;
use crate::data::Remote;
use crate::timefmt;
use crate::ui::icons::Icons;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

/// Render the match detail overlay.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let title = app
        .detail()
        .map_or_else(|| "Match".to_owned(), |d| d.label.clone());
    let block = widgets::screen_block(&title, "j/k scroll · Esc close", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let state = app.detail_state().state();
    let Remote::Ready { value: detail, .. } = state else {
        widgets::remote_message(frame, inner, theme, state, |_| Vec::new());
        return;
    };

    let [header_area, body_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(inner);
    let kickoff = timefmt::kickoff_day_time(
        detail.summary.kickoff,
        &app.config().ui.timezone,
        app.local_offset(),
    );
    frame.render_widget(
        Paragraph::new(detail_header_lines(detail, theme, app.icons(), &kickoff))
            .style(Style::new().fg(theme.fg)),
        header_area,
    );

    let body = detail_body_lines(detail, theme, app.icons());
    let paragraph = Paragraph::new(body)
        .style(Style::new().fg(theme.fg))
        .wrap(Wrap { trim: false })
        .scroll((app.ui_state.detail_scroll, 0));
    frame.render_widget(paragraph, body_area);
}

/// Handle a key for the detail overlay. Returns `true` if consumed. `Esc` is
/// handled globally by the app to close the overlay.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.scroll_detail(1);
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.scroll_detail(-1);
            true
        }
        _ => false,
    }
}

fn detail_header_lines(
    detail: &MatchDetail,
    theme: &Theme,
    icons: Icons,
    kickoff: &str,
) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled(
                team_label(&detail.summary.home),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}  ", score_text(&detail.summary)),
                Style::new().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                team_label(&detail.summary.away),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            status_span(&detail.summary.status, theme, icons),
        ]),
        Line::from(Span::styled(
            format!(
                "{} · {}",
                kickoff,
                detail.summary.venue.clone().unwrap_or_else(|| detail
                    .summary
                    .stage
                    .label()
                    .to_owned())
            ),
            Style::new().fg(theme.dim),
        )),
        Line::from(Span::styled(
            "j/k: scroll · Esc: back",
            Style::new().fg(theme.dim),
        )),
    ]
}

fn detail_body_lines(detail: &MatchDetail, theme: &Theme, icons: Icons) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    push_heading(&mut lines, "Timeline", theme);
    if detail.events.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No timeline events yet.",
            Style::new().fg(theme.dim),
        )));
    } else {
        lines.extend(
            detail
                .events
                .iter()
                .map(|event| event_line(event, detail, theme, icons)),
        );
    }

    lines.push(Line::default());
    push_heading(&mut lines, "Lineups", theme);
    let home_lineup = lineup_for(&detail.lineups, &detail.summary.home.id);
    let away_lineup = lineup_for(&detail.lineups, &detail.summary.away.id);
    append_lineup_pair(
        &mut lines,
        &detail.summary.home,
        home_lineup,
        &detail.summary.away,
        away_lineup,
        theme,
    );

    lines.push(Line::default());
    push_heading(&mut lines, "Team stats", theme);
    if detail.stats.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No team stats available.",
            Style::new().fg(theme.dim),
        )));
    } else {
        lines.extend(detail.stats.iter().map(|stat| stat_line(stat, theme)));
    }
    lines
}

fn push_heading(lines: &mut Vec<Line<'static>>, text: &str, theme: &Theme) {
    lines.push(Line::from(Span::styled(
        text.to_owned(),
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    )));
}

fn event_line(
    event: &MatchEvent,
    detail: &MatchDetail,
    theme: &Theme,
    icons: Icons,
) -> Line<'static> {
    let team = event
        .team_id
        .as_deref()
        .and_then(|id| team_by_id(&detail.summary, id))
        .map_or_else(String::new, team_label);
    let player = event
        .player
        .clone()
        .unwrap_or_else(|| "Unknown player".to_owned());
    let extra = event
        .detail
        .as_ref()
        .map_or_else(String::new, |d| format!(" — {d}"));
    Line::from(vec![
        Span::styled(
            format!("  {:>4} ", minute_text(event)),
            Style::new().fg(theme.dim),
        ),
        Span::styled(
            format!("{} ", event_icon(event.kind, icons)),
            event_style(event.kind, theme),
        ),
        Span::styled(
            event_label(event.kind),
            event_style(event.kind, theme).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {player}"), Style::new().fg(theme.fg)),
        Span::styled(extra, Style::new().fg(theme.dim)),
        Span::styled(
            if team.is_empty() {
                String::new()
            } else {
                format!("  ({team})")
            },
            Style::new().fg(theme.dim),
        ),
    ])
}

fn minute_text(event: &MatchEvent) -> String {
    match (event.minute, event.stoppage) {
        (Some(minute), Some(stoppage)) => format!("{minute}+{stoppage}'"),
        (Some(minute), None) => format!("{minute}'"),
        (None, _) => "--".to_owned(),
    }
}

fn event_icon(kind: MatchEventKind, icons: Icons) -> &'static str {
    match kind {
        MatchEventKind::Goal | MatchEventKind::OwnGoal | MatchEventKind::PenaltyGoal => "⚽",
        MatchEventKind::PenaltyMiss => "×",
        MatchEventKind::YellowCard | MatchEventKind::SecondYellow => "YC",
        MatchEventKind::RedCard => "RC",
        MatchEventKind::Substitution => "↔",
        MatchEventKind::Var => "VAR",
        MatchEventKind::Other => icons.bullet(),
    }
}

fn event_label(kind: MatchEventKind) -> &'static str {
    match kind {
        MatchEventKind::Goal => "Goal",
        MatchEventKind::OwnGoal => "Own goal",
        MatchEventKind::PenaltyGoal => "Penalty goal",
        MatchEventKind::PenaltyMiss => "Penalty missed",
        MatchEventKind::YellowCard => "Yellow card",
        MatchEventKind::SecondYellow => "Second yellow",
        MatchEventKind::RedCard => "Red card",
        MatchEventKind::Substitution => "Substitution",
        MatchEventKind::Var => "VAR",
        MatchEventKind::Other => "Event",
    }
}

fn event_style(kind: MatchEventKind, theme: &Theme) -> Style {
    let color = match kind {
        MatchEventKind::Goal | MatchEventKind::PenaltyGoal => theme.ok,
        MatchEventKind::OwnGoal | MatchEventKind::PenaltyMiss => theme.warn,
        MatchEventKind::YellowCard | MatchEventKind::SecondYellow | MatchEventKind::Var => {
            theme.warn
        }
        MatchEventKind::RedCard => theme.error,
        MatchEventKind::Substitution | MatchEventKind::Other => theme.accent,
    };
    Style::new().fg(color)
}

fn append_lineup_pair(
    lines: &mut Vec<Line<'static>>,
    home: &Team,
    home_lineup: Option<&Lineup>,
    away: &Team,
    away_lineup: Option<&Lineup>,
    theme: &Theme,
) {
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {:<34}", lineup_title(home, home_lineup)),
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            lineup_title(away, away_lineup),
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ]));

    let home_players = lineup_players(home_lineup);
    let away_players = lineup_players(away_lineup);
    let count = home_players.len().max(away_players.len()).max(1);
    for index in 0..count {
        let left = home_players
            .get(index)
            .cloned()
            .unwrap_or_else(|| "—".to_owned());
        let right = away_players
            .get(index)
            .cloned()
            .unwrap_or_else(|| "—".to_owned());
        lines.push(Line::from(vec![
            Span::styled(format!("  {left:<34}"), Style::new().fg(theme.fg)),
            Span::styled(right, Style::new().fg(theme.fg)),
        ]));
    }
}

fn lineup_title(team: &Team, lineup: Option<&Lineup>) -> String {
    lineup.and_then(|l| l.formation.as_ref()).map_or_else(
        || team_label(team),
        |formation| format!("{} ({formation})", team_label(team)),
    )
}

fn lineup_players(lineup: Option<&Lineup>) -> Vec<String> {
    let Some(lineup) = lineup else {
        return vec!["Lineup unavailable".to_owned()];
    };
    let mut players = Vec::new();
    if !lineup.starters.is_empty() {
        players.push("Starters".to_owned());
        players.extend(lineup.starters.iter().map(player_text));
    }
    if !lineup.substitutes.is_empty() {
        players.push("Subs".to_owned());
        players.extend(lineup.substitutes.iter().map(player_text));
    }
    if players.is_empty() {
        players.push("Lineup unavailable".to_owned());
    }
    players
}

fn player_text(player: &Player) -> String {
    let number = player
        .number
        .map_or_else(String::new, |n| format!("{n:>2} "));
    let position = player
        .position
        .as_ref()
        .map_or_else(String::new, |p| format!(" {p}"));
    format!("{number}{}{}", player.name, position)
}

fn stat_line(stat: &TeamStat, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:>10} ", stat.home),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{:<24}", stat.label), Style::new().fg(theme.fg)),
        Span::styled(
            format!(" {:<10}", stat.away),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn lineup_for<'a>(lineups: &'a [Lineup], team_id: &str) -> Option<&'a Lineup> {
    lineups.iter().find(|lineup| lineup.team_id == team_id)
}

fn team_by_id<'a>(summary: &'a Match, id: &str) -> Option<&'a Team> {
    if summary.home.id == id {
        Some(&summary.home)
    } else if summary.away.id == id {
        Some(&summary.away)
    } else {
        None
    }
}

fn status_span(status: &MatchStatus, theme: &Theme, icons: Icons) -> Span<'static> {
    let (text, color) = match status {
        MatchStatus::Scheduled => ("upcoming".to_owned(), theme.dim),
        MatchStatus::Live { minute, .. } => (
            minute.map_or_else(
                || format!("{} LIVE", icons.live()),
                |m| format!("{} LIVE {m}'", icons.live()),
            ),
            theme.warn,
        ),
        MatchStatus::HalfTime => (format!("{} HT", icons.live()), theme.warn),
        MatchStatus::FullTime => ("FT".to_owned(), theme.ok),
        MatchStatus::AfterExtraTime => ("AET".to_owned(), theme.ok),
        MatchStatus::Penalties => ("PEN".to_owned(), theme.ok),
        MatchStatus::Postponed => ("PPD".to_owned(), theme.error),
        MatchStatus::Canceled => ("CAN".to_owned(), theme.error),
        MatchStatus::Unknown => ("TBD".to_owned(), theme.dim),
    };
    Span::styled(text, Style::new().fg(color).add_modifier(Modifier::BOLD))
}

fn score_text(m: &Match) -> String {
    match (&m.status, m.score) {
        (MatchStatus::Scheduled, _) | (_, None) => "vs".to_owned(),
        (_, Some(score)) => {
            let mut text = format!("{}-{}", score.home, score.away);
            if let (Some(home), Some(away)) = (score.home_pens, score.away_pens) {
                let _ = write!(text, " ({home}-{away}p)");
            }
            text
        }
    }
}

fn team_label(team: &Team) -> String {
    if team.abbreviation.is_empty() {
        team.name.clone()
    } else {
        team.abbreviation.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;
    use wc_data::domain::{Score, Stage};

    fn team(id: &str, name: &str, abbreviation: &str) -> Team {
        Team {
            id: id.to_owned(),
            name: name.to_owned(),
            abbreviation: abbreviation.to_owned(),
            country_code: None,
            crest_url: None,
        }
    }

    fn detail() -> MatchDetail {
        MatchDetail {
            summary: Match {
                id: "m1".to_owned(),
                stage: Stage::GroupStage,
                group: Some("A".to_owned()),
                home: team("can", "Canada", "CAN"),
                away: team("mex", "Mexico", "MEX"),
                score: Some(Score {
                    home: 1,
                    away: 0,
                    home_pens: None,
                    away_pens: None,
                }),
                status: MatchStatus::Live {
                    minute: Some(30),
                    detail: None,
                },
                kickoff: OffsetDateTime::UNIX_EPOCH,
                venue: Some("Toronto".to_owned()),
            },
            events: vec![MatchEvent {
                minute: Some(12),
                stoppage: None,
                kind: MatchEventKind::Goal,
                team_id: Some("can".to_owned()),
                player: Some("Jessie Fleming".to_owned()),
                detail: Some("Assist: Lawrence".to_owned()),
            }],
            lineups: vec![Lineup {
                team_id: "can".to_owned(),
                formation: Some("4-3-3".to_owned()),
                starters: vec![Player {
                    name: "Player One".to_owned(),
                    number: Some(1),
                    position: Some("GK".to_owned()),
                }],
                substitutes: Vec::new(),
            }],
            stats: vec![TeamStat {
                label: "Possession".to_owned(),
                home: "55%".to_owned(),
                away: "45%".to_owned(),
            }],
        }
    }

    #[test]
    fn goal_event_line_shows_scorer() {
        let detail = detail();
        let theme = Theme::world_night();
        let icons = Icons::new(false);
        let line = event_line(&detail.events[0], &detail, &theme, icons);
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(rendered.contains("Goal"));
        assert!(rendered.contains("Jessie Fleming"));
        assert!(rendered.contains("CAN"));
    }

    #[test]
    fn stats_line_contains_both_values() {
        let theme = Theme::world_night();
        let line = stat_line(
            &TeamStat {
                label: "Shots".to_owned(),
                home: "8".to_owned(),
                away: "5".to_owned(),
            },
            &theme,
        );
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(rendered.contains('8'));
        assert!(rendered.contains("Shots"));
        assert!(rendered.contains('5'));
    }
}
