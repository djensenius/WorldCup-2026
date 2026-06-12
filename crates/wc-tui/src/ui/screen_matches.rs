//! Matches / schedule screen.
//!
//! Lists fixtures grouped by day and stage with status badges, local-timezone
//! kickoff times, and a favourites filter. `Enter` opens the match detail.

use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use wc_data::domain::{Match, MatchStatus, Stage, Team};

use crate::app::App;
use crate::data::Remote;
use crate::timefmt;
use crate::ui::icons::Icons;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

/// Render the matches screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let hint = if app.ui_state.matches_favorites_only {
        "j/k move · f all · Enter detail"
    } else {
        "j/k move · f favourites · Enter detail"
    };
    let block = widgets::screen_block("Matches", hint, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let state = app.scoreboard().state();
    let Remote::Ready { value: matches, .. } = state else {
        widgets::remote_message(frame, inner, theme, state, |_| Vec::new());
        return;
    };

    let rows = visible_matches(app, matches);
    if rows.is_empty() {
        let text = if app.ui_state.matches_favorites_only {
            "No favourite-team fixtures found. Press f to show all matches."
        } else {
            "No fixtures loaded yet."
        };
        widgets::message(
            frame,
            inner,
            theme,
            vec![Line::from(Span::styled(text, Style::new().fg(theme.dim)))],
        );
        return;
    }

    let selected = app
        .ui_state
        .matches_selected
        .min(rows.len().saturating_sub(1));
    let lines = schedule_lines(
        &rows,
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

/// Handle a key for the matches screen. Returns `true` if consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(matches) = app.scoreboard().state().value() else {
        return false;
    };
    let rows = visible_matches(app, matches);
    let len = rows.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.ui_state.matches_selected = (app.ui_state.matches_selected + 1).min(len - 1);
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.ui_state.matches_selected = app.ui_state.matches_selected.saturating_sub(1);
            true
        }
        KeyCode::Char('f') => {
            app.ui_state.matches_favorites_only = !app.ui_state.matches_favorites_only;
            app.ui_state.matches_selected = 0;
            true
        }
        KeyCode::Enter => {
            if let Some(m) = rows.get(app.ui_state.matches_selected.min(len.saturating_sub(1))) {
                app.open_detail(m.id.clone(), match_label(m));
            }
            true
        }
        _ => false,
    }
}

fn visible_matches<'a>(app: &App, matches: &'a [Match]) -> Vec<&'a Match> {
    let mut rows = matches
        .iter()
        .filter(|m| !app.ui_state.matches_favorites_only || involves_favorite(app, m))
        .collect::<Vec<_>>();
    rows.sort_by_key(|m| {
        (
            m.kickoff,
            stage_order(m.stage),
            m.group.clone(),
            m.id.clone(),
        )
    });
    rows
}

fn schedule_lines(
    rows: &[&Match],
    selected: usize,
    theme: &Theme,
    icons: Icons,
    app: &App,
    height: usize,
) -> Vec<Line<'static>> {
    let mut all = Vec::new();
    let mut current_day = String::new();
    let mut current_stage = String::new();
    let selected_id = rows.get(selected).map(|m| m.id.as_str());
    let mut selected_line = 0usize;

    for m in rows {
        let day = timefmt::date_heading(m.kickoff, &app.config().ui.timezone, app.local_offset());
        if day != current_day {
            if !all.is_empty() {
                all.push(Line::from(""));
            }
            current_day.clone_from(&day);
            all.push(Line::from(Span::styled(
                day,
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            )));
            current_stage.clear();
        }
        let stage = section_label(m);
        if stage != current_stage {
            current_stage.clone_from(&stage);
            all.push(Line::from(Span::styled(
                format!("  {stage}"),
                Style::new().fg(theme.dim).add_modifier(Modifier::BOLD),
            )));
        }
        if Some(m.id.as_str()) == selected_id {
            selected_line = all.len();
        }
        all.push(match_row_line(
            m,
            theme,
            icons,
            &timefmt::time_hm(m.kickoff, &app.config().ui.timezone, app.local_offset()),
            involves_favorite(app, m),
            Some(m.id.as_str()) == selected_id,
        ));
    }

    let available = height.max(1);
    let start = selected_line.saturating_sub(available.saturating_sub(1));
    all.into_iter().skip(start).take(available).collect()
}

fn match_row_line(
    m: &Match,
    theme: &Theme,
    icons: Icons,
    kickoff: &str,
    favorite: bool,
    selected: bool,
) -> Line<'static> {
    let row_style = if selected {
        Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.fg)
    };
    let team_style = if favorite {
        row_style.fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        row_style
    };
    let (marker, marker_style) = if selected {
        (
            "›",
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        )
    } else if favorite {
        (icons.star(), Style::new().fg(theme.accent))
    } else {
        (" ", Style::new().fg(theme.dim))
    };
    Line::from(vec![
        Span::styled(format!("{marker} "), marker_style),
        Span::styled(format!("{kickoff:<5}  "), Style::new().fg(theme.dim)),
        Span::styled(format!("{:>6}", team_label(&m.home)), team_style),
        Span::styled(format!(" {:^11} ", score_text(m)), row_style),
        Span::styled(format!("{:<6}", team_label(&m.away)), team_style),
        Span::styled("  ", row_style),
        status_span(&m.status, theme, icons),
    ])
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

fn section_label(m: &Match) -> String {
    match (&m.stage, &m.group) {
        (Stage::GroupStage, Some(group)) => format!("{} · Group {group}", m.stage.label()),
        _ => m.stage.label().to_owned(),
    }
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

fn involves_favorite(app: &App, m: &Match) -> bool {
    app.config().is_favorite(&m.home.name, &m.home.abbreviation)
        || app.config().is_favorite(&m.away.name, &m.away.abbreviation)
}

const fn stage_order(stage: Stage) -> u8 {
    match stage {
        Stage::GroupStage => 0,
        Stage::RoundOf32 => 1,
        Stage::RoundOf16 => 2,
        Stage::QuarterFinal => 3,
        Stage::SemiFinal => 4,
        Stage::ThirdPlace => 5,
        Stage::Final => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;
    use wc_data::domain::{Score, Stage};

    fn team(name: &str, abbreviation: &str) -> Team {
        Team {
            id: abbreviation.to_lowercase(),
            name: name.to_owned(),
            abbreviation: abbreviation.to_owned(),
            country_code: None,
            crest_url: None,
        }
    }

    fn fixture(status: MatchStatus, score: Option<Score>) -> Match {
        Match {
            id: "m1".to_owned(),
            stage: Stage::GroupStage,
            group: Some("A".to_owned()),
            home: team("Canada", "CAN"),
            away: team("Mexico", "MEX"),
            score,
            status,
            kickoff: OffsetDateTime::UNIX_EPOCH,
            venue: Some("Toronto".to_owned()),
        }
    }

    #[test]
    fn scheduled_match_uses_vs() {
        let m = fixture(MatchStatus::Scheduled, None);
        assert_eq!(score_text(&m), "vs");
    }

    #[test]
    fn live_badge_shows_minute() {
        let theme = Theme::world_night();
        let icons = Icons::new(false);
        let span = status_span(
            &MatchStatus::Live {
                minute: Some(67),
                detail: None,
            },
            &theme,
            icons,
        );
        assert!(span.content.contains("67'"));
        assert!(span.content.contains("LIVE"));
    }

    #[test]
    fn finished_match_uses_score() {
        let m = fixture(
            MatchStatus::FullTime,
            Some(Score {
                home: 2,
                away: 1,
                home_pens: None,
                away_pens: None,
            }),
        );
        assert_eq!(score_text(&m), "2-1");
    }
}
