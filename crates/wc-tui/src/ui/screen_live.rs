//! Live scoreboard screen.
//!
//! A board of in-play matches (auto-refreshed on a fast cadence) followed by the
//! soonest upcoming fixtures. `j`/`k` move across both sections and `Enter`
//! opens the match detail.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use wc_data::domain::{Match, MatchStatus, Score, Stage, Team};

use crate::app::App;
use crate::data::Remote;
use crate::timefmt;
use crate::ui::icons::Icons;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

/// How many upcoming fixtures to list on the live board.
const UPCOMING_LIMIT: usize = 16;

/// Render the live scoreboard screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::screen_block("Live", "j/k move · Enter detail", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let state = app.scoreboard().state();
    let Remote::Ready { value: matches, .. } = state else {
        widgets::remote_message(frame, inner, theme, state, |_| Vec::new());
        return;
    };

    let live = live_matches(matches);
    let upcoming = upcoming_matches(matches);
    if live.is_empty() && upcoming.is_empty() {
        widgets::message(
            frame,
            inner,
            theme,
            vec![Line::from(Span::styled(
                "No matches in play, and no upcoming fixtures loaded",
                Style::new().fg(theme.dim),
            ))],
        );
        return;
    }

    let selected = app
        .ui_state
        .live_selected
        .min(live.len() + upcoming.len() - 1);
    let lines = board_lines(
        &live,
        &upcoming,
        selected,
        theme,
        app.icons(),
        app,
        usize::from(inner.height),
    );
    let paragraph = Paragraph::new(lines)
        .style(Style::new().fg(theme.fg))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Handle a key for the live screen. Returns `true` if consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(matches) = app.scoreboard().state().value() else {
        return false;
    };
    let live = live_matches(matches);
    let upcoming = upcoming_matches(matches);
    let len = live.len() + upcoming.len();
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
            let index = app.ui_state.live_selected.min(len.saturating_sub(1));
            if let Some(m) = live.iter().chain(upcoming.iter()).nth(index) {
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
    live.sort_by(|a, b| a.kickoff.cmp(&b.kickoff).then_with(|| a.id.cmp(&b.id)));
    live
}

fn upcoming_matches(matches: &[Match]) -> Vec<&Match> {
    let mut upcoming = matches
        .iter()
        .filter(|m| matches!(m.status, MatchStatus::Scheduled))
        .collect::<Vec<_>>();
    upcoming.sort_by(|a, b| a.kickoff.cmp(&b.kickoff).then_with(|| a.id.cmp(&b.id)));
    upcoming.truncate(UPCOMING_LIMIT);
    upcoming
}

fn board_lines(
    live: &[&Match],
    upcoming: &[&Match],
    selected: usize,
    theme: &Theme,
    icons: Icons,
    app: &App,
    height: usize,
) -> Vec<Line<'static>> {
    let mut all = Vec::new();
    let mut selected_line = 0usize;

    if !live.is_empty() {
        all.push(section_header("In play", theme));
        for (index, m) in live.iter().enumerate() {
            if index == selected {
                selected_line = all.len();
            }
            all.push(live_row_line(m, index == selected, theme, icons));
        }
    }

    if !upcoming.is_empty() {
        if !all.is_empty() {
            all.push(Line::from(""));
        }
        all.push(section_header("Upcoming", theme));
        for (offset, m) in upcoming.iter().enumerate() {
            let index = live.len() + offset;
            if index == selected {
                selected_line = all.len();
            }
            let when =
                timefmt::kickoff_day_time(m.kickoff, &app.config().ui.timezone, app.local_offset());
            all.push(upcoming_row_line(m, index == selected, theme, &when));
        }
    }

    let available = height.max(1);
    let start = selected_line.saturating_sub(available.saturating_sub(1));
    all.into_iter().skip(start).take(available).collect()
}

fn section_header(title: &str, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        title.to_owned(),
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    ))
}

fn live_row_line(m: &Match, selected: bool, theme: &Theme, icons: Icons) -> Line<'static> {
    let row_style = if selected {
        Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.fg)
    };
    let mark = if selected { "›" } else { " " };
    Line::from(vec![
        Span::styled(
            format!("{mark} {} ", icons.live()),
            Style::new().fg(theme.warn).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{:>6}", team_label(&m.home)), row_style),
        Span::styled(
            format!(" {:^7} ", live_score(m.score)),
            row_style.fg(theme.warn).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{:<6}", team_label(&m.away)), row_style),
        Span::styled(
            format!("  {}", live_clock(&m.status)),
            Style::new().fg(theme.warn).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn upcoming_row_line(m: &Match, selected: bool, theme: &Theme, when: &str) -> Line<'static> {
    let row_style = if selected {
        Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.fg)
    };
    let (mark, mark_style) = if selected {
        (
            "›",
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        )
    } else {
        (" ", Style::new().fg(theme.dim))
    };
    Line::from(vec![
        Span::styled(format!("{mark} "), mark_style),
        Span::styled(format!("{when:<12} "), Style::new().fg(theme.dim)),
        Span::styled(format!("{:>6}", team_label(&m.home)), row_style),
        Span::styled(" vs ", Style::new().fg(theme.dim)),
        Span::styled(format!("{:<6}", team_label(&m.away)), row_style),
        Span::styled(format!("  {}", context_tag(m)), Style::new().fg(theme.dim)),
    ])
}

fn context_tag(m: &Match) -> String {
    match (m.stage, &m.group) {
        (Stage::GroupStage, Some(group)) => format!("Group {group}"),
        (stage, _) => stage.label().to_owned(),
    }
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

    fn team(name: &str, abbreviation: &str) -> Team {
        Team {
            id: abbreviation.to_lowercase(),
            name: name.to_owned(),
            abbreviation: abbreviation.to_owned(),
            country_code: None,
            crest_url: None,
        }
    }

    fn fixture(status: MatchStatus, ts: i64) -> Match {
        Match {
            id: format!("m{ts}"),
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
            kickoff: OffsetDateTime::from_unix_timestamp(ts).unwrap_or(OffsetDateTime::UNIX_EPOCH),
            venue: None,
        }
    }

    #[test]
    fn live_filter_keeps_live_and_halftime_only() {
        let matches = vec![
            fixture(MatchStatus::Scheduled, 1),
            fixture(MatchStatus::HalfTime, 2),
            fixture(
                MatchStatus::Live {
                    minute: Some(12),
                    detail: None,
                },
                3,
            ),
        ];
        assert_eq!(live_matches(&matches).len(), 2);
    }

    #[test]
    fn upcoming_filter_keeps_scheduled_sorted_and_capped() {
        let mut matches = vec![
            fixture(MatchStatus::Scheduled, 300),
            fixture(
                MatchStatus::Live {
                    minute: None,
                    detail: None,
                },
                50,
            ),
            fixture(MatchStatus::Scheduled, 100),
            fixture(MatchStatus::FullTime, 10),
        ];
        for ts in 0..40 {
            matches.push(fixture(MatchStatus::Scheduled, 1000 + i64::from(ts)));
        }
        let upcoming = upcoming_matches(&matches);
        assert_eq!(upcoming.len(), UPCOMING_LIMIT);
        // Sorted ascending: the earliest scheduled (ts=100) comes first.
        assert_eq!(upcoming[0].id, "m100");
        assert_eq!(upcoming[1].id, "m300");
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
