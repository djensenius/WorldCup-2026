//! Live scoreboard screen.
//!
//! A compact board of in-play matches that auto-refreshes on a fast cadence.
//! `Enter` opens the match detail.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use wc_data::domain::{Match, MatchStatus, Score, Team};

use crate::app::App;
use crate::data::Remote;
use crate::ui::icons::Icons;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

/// Render the live scoreboard screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::panel("Live", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let state = app.scoreboard().state();
    let Remote::Ready { value: matches, .. } = state else {
        widgets::remote_message(frame, inner, theme, state, |_| Vec::new());
        return;
    };

    let live = live_matches(matches);
    if live.is_empty() {
        widgets::message(
            frame,
            inner,
            theme,
            vec![Line::from(Span::styled(
                "No matches in play right now",
                Style::new().fg(theme.dim),
            ))],
        );
        return;
    }

    let selected = app.ui_state.live_selected.min(live.len().saturating_sub(1));
    let lines = live_lines(
        &live,
        selected,
        theme,
        app.icons(),
        usize::from(inner.height),
    );
    let paragraph = Paragraph::new(lines)
        .style(Style::new().fg(theme.fg))
        .wrap(Wrap { trim: false })
        .block(widgets::panel("j/k: move · Enter: detail", theme));
    frame.render_widget(paragraph, inner);
}

/// Handle a key for the live screen. Returns `true` if consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(matches) = app.scoreboard().state().value() else {
        return false;
    };
    let live = live_matches(matches);
    let len = live.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.ui_state.live_selected = (app.ui_state.live_selected + 1).min(len - 1);
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.ui_state.live_selected = app.ui_state.live_selected.saturating_sub(1);
            true
        }
        KeyCode::Enter => {
            if let Some(m) = live.get(app.ui_state.live_selected.min(len.saturating_sub(1))) {
                app.open_detail(m.id.clone(), match_label(m));
            }
            true
        }
        _ => false,
    }
}

fn live_matches(matches: &[Match]) -> Vec<&Match> {
    let mut live = matches
        .iter()
        .filter(|m| m.status.is_live())
        .collect::<Vec<_>>();
    live.sort_by_key(|m| (m.kickoff, m.id.clone()));
    live
}

fn live_lines(
    rows: &[&Match],
    selected: usize,
    theme: &Theme,
    icons: Icons,
    height: usize,
) -> Vec<Line<'static>> {
    let available = height.saturating_sub(2).max(1);
    let start = selected.saturating_sub(available.saturating_sub(1));
    rows.iter()
        .enumerate()
        .skip(start)
        .take(available)
        .map(|(index, m)| live_row_line(m, index == selected, theme, icons))
        .collect()
}

fn live_row_line(m: &Match, selected: bool, theme: &Theme, icons: Icons) -> Line<'static> {
    let row_style = if selected {
        Style::new()
            .fg(theme.fg)
            .bg(theme.bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.fg)
    };
    let mark = if selected { "›" } else { " " };
    Line::from(vec![
        Span::styled(
            format!("{mark} {} ", icons.live()),
            Style::new().fg(theme.warn),
        ),
        Span::styled(format!("{:<12}", team_label(&m.home)), row_style),
        Span::styled(
            format!(" {:^7} ", live_score(m.score)),
            row_style.fg(theme.warn).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{:<12}", team_label(&m.away)), row_style),
        Span::styled(
            format!("  {}", live_clock(&m.status)),
            Style::new().fg(theme.warn).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn live_clock(status: &MatchStatus) -> String {
    match status {
        MatchStatus::Live { minute, detail } => minute.map_or_else(
            || detail.clone().unwrap_or_else(|| "LIVE".to_owned()),
            |m| format!("{m}'"),
        ),
        MatchStatus::HalfTime => "HT".to_owned(),
        _ => String::new(),
    }
}

fn live_score(score: Option<Score>) -> String {
    score.map_or_else(|| "0-0".to_owned(), |s| format!("{}-{}", s.home, s.away))
}

fn match_label(m: &Match) -> String {
    format!("{} vs {}", team_label(&m.home), team_label(&m.away))
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
    use wc_data::domain::Stage;

    fn team(name: &str, abbreviation: &str) -> Team {
        Team {
            id: abbreviation.to_lowercase(),
            name: name.to_owned(),
            abbreviation: abbreviation.to_owned(),
            country_code: None,
            crest_url: None,
        }
    }

    fn fixture(status: MatchStatus) -> Match {
        Match {
            id: "m1".to_owned(),
            stage: Stage::GroupStage,
            group: Some("A".to_owned()),
            home: team("Canada", "CAN"),
            away: team("Mexico", "MEX"),
            score: Some(Score {
                home: 1,
                away: 0,
                home_pens: None,
                away_pens: None,
            }),
            status,
            kickoff: OffsetDateTime::UNIX_EPOCH,
            venue: None,
        }
    }

    #[test]
    fn live_filter_keeps_live_and_halftime_only() {
        let matches = vec![
            fixture(MatchStatus::Scheduled),
            fixture(MatchStatus::HalfTime),
            fixture(MatchStatus::Live {
                minute: Some(12),
                detail: None,
            }),
        ];
        assert_eq!(live_matches(&matches).len(), 2);
    }

    #[test]
    fn live_clock_shows_minute() {
        let clock = live_clock(&MatchStatus::Live {
            minute: Some(42),
            detail: None,
        });
        assert_eq!(clock, "42'");
    }
}
