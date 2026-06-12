//! Knockout bracket screen.
//!
//! Renders the knockout tree (Round of 32 → Final, plus the third-place
//! play-off) as scrollable columns.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wc_data::domain::{Bracket, BracketRound, Match, Score, Stage, Team};

use crate::app::App;
use crate::data::Remote;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

const MIN_COLUMN_WIDTH: u16 = 20;
const CELL_HEIGHT: u16 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BracketCell {
    home: String,
    away: String,
    selected: bool,
}

/// Render the bracket screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::screen_block("Bracket", "h/l round · j/k match", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.bracket().state() {
        Remote::Ready { value: bracket, .. } => render_bracket(app, frame, inner, bracket),
        state => widgets::remote_message(frame, inner, theme, state, |_| Vec::new()),
    }
}

fn render_bracket(app: &App, frame: &mut Frame, area: Rect, bracket: &Bracket) {
    let theme = app.theme();
    if bracket.rounds.is_empty() || bracket.rounds.iter().all(|round| round.matches.is_empty()) {
        widgets::message(
            frame,
            area,
            theme,
            vec![
                Line::from(Span::styled(
                    "The knockout bracket appears once the group stage finishes.",
                    Style::new().fg(theme.dim),
                )),
                Line::from(Span::styled(
                    "Check back after the Round of 32 matchups are known.",
                    Style::new().fg(theme.dim),
                )),
            ],
        );
        return;
    }

    let selected_round = app
        .ui_state
        .bracket_round
        .min(bracket.rounds.len().saturating_sub(1));
    let selected_match = selected_match_index(bracket, selected_round, app.ui_state.bracket_match);
    let visible = visible_round_range(bracket.rounds.len(), selected_round, area.width);
    let chunks = horizontal_chunks(area, visible.len());

    for (chunk, round_index) in chunks.into_iter().zip(visible) {
        let is_selected_round = round_index == selected_round;
        let match_index = if is_selected_round { selected_match } else { 0 };
        let lines = bracket_column_lines(
            &bracket.rounds[round_index],
            match_index,
            is_selected_round,
            visible_match_capacity(chunk.height),
            theme,
        );
        frame.render_widget(Paragraph::new(lines), chunk);
    }
}

fn horizontal_chunks(area: Rect, count: usize) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let constraints = (0..count)
        .map(|_| Constraint::Percentage(100 / u16::try_from(count).unwrap_or(1)))
        .collect::<Vec<_>>();
    Layout::horizontal(constraints).split(area).to_vec()
}

fn bracket_column_lines(
    round: &BracketRound,
    selected_match: usize,
    selected_round: bool,
    match_capacity: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            round.stage.label(),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    if round.matches.is_empty() {
        lines.push(Line::from(Span::styled("TBD", Style::new().fg(theme.dim))));
        return lines;
    }

    let visible = visible_match_range(round.matches.len(), selected_match, match_capacity);
    if visible.start > 0 {
        lines.push(Line::from(Span::styled(
            "↑ more",
            Style::new().fg(theme.dim),
        )));
    }
    for index in visible {
        let cell = bracket_cell(
            &round.matches[index],
            selected_round && index == selected_match,
        );
        lines.extend(cell.into_lines(theme));
        lines.push(Line::from(""));
    }
    if selected_match + match_capacity < round.matches.len() {
        lines.push(Line::from(Span::styled(
            "↓ more",
            Style::new().fg(theme.dim),
        )));
    }
    lines
}

fn visible_round_range(
    round_count: usize,
    selected_round: usize,
    width: u16,
) -> std::ops::Range<usize> {
    if round_count == 0 {
        return 0..0;
    }
    let max_columns = usize::from((width / MIN_COLUMN_WIDTH).max(1)).min(round_count);
    let half = max_columns / 2;
    let mut start = selected_round.saturating_sub(half);
    if start + max_columns > round_count {
        start = round_count.saturating_sub(max_columns);
    }
    start..start + max_columns
}

fn visible_match_capacity(height: u16) -> usize {
    usize::from(height.saturating_sub(3) / CELL_HEIGHT).max(1)
}

fn visible_match_range(
    match_count: usize,
    selected_match: usize,
    match_capacity: usize,
) -> std::ops::Range<usize> {
    if match_count == 0 {
        return 0..0;
    }
    let capacity = match_capacity.max(1).min(match_count);
    let half = capacity / 2;
    let mut start = selected_match.saturating_sub(half);
    if start + capacity > match_count {
        start = match_count.saturating_sub(capacity);
    }
    start..start + capacity
}

fn selected_match_index(bracket: &Bracket, round_index: usize, requested: usize) -> usize {
    bracket.rounds.get(round_index).map_or(0, |round| {
        requested.min(round.matches.len().saturating_sub(1))
    })
}

fn bracket_cell(match_: &Match, selected: bool) -> BracketCell {
    BracketCell {
        home: team_line(&match_.home, Side::Home, match_.score),
        away: team_line(&match_.away, Side::Away, match_.score),
        selected,
    }
}

impl BracketCell {
    fn into_lines(self, theme: &Theme) -> Vec<Line<'static>> {
        let style = if self.selected {
            Style::new()
                .fg(theme.accent)
                .bg(theme.bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.fg)
        };
        vec![
            Line::from(Span::styled(self.home, style)),
            Line::from(Span::styled(self.away, style)),
        ]
    }
}

#[derive(Debug, Clone, Copy)]
enum Side {
    Home,
    Away,
}

fn team_line(team: &Team, side: Side, full_score: Option<Score>) -> String {
    let name = if team.is_placeholder() || team.abbreviation.is_empty() {
        team.name.clone()
    } else {
        team.abbreviation.clone()
    };
    let Some(full_score) = full_score else {
        return name;
    };
    match side {
        Side::Home => full_score.home_pens.map_or_else(
            || format!("{name} {}", full_score.home),
            |pens| format!("{name} {} ({pens})", full_score.home),
        ),
        Side::Away => full_score.away_pens.map_or_else(
            || format!("{name} {}", full_score.away),
            |pens| format!("{name} {} ({pens})", full_score.away),
        ),
    }
}

#[cfg(test)]
fn round_titles(bracket: &Bracket) -> Vec<&'static str> {
    bracket
        .rounds
        .iter()
        .map(|round| round.stage.label())
        .collect()
}

/// Handle a key for the bracket screen. Returns `true` if consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let (round_count, current_match_count) = bracket_counts(app);
    match key.code {
        KeyCode::Char('h') | KeyCode::Left => {
            app.ui_state.bracket_round = app.ui_state.bracket_round.saturating_sub(1);
            clamp_bracket_match(app, round_count);
            true
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.ui_state.bracket_round =
                (app.ui_state.bracket_round + 1).min(round_count.saturating_sub(1));
            clamp_bracket_match(app, round_count);
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.ui_state.bracket_match = app.ui_state.bracket_match.saturating_sub(1);
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.ui_state.bracket_match =
                (app.ui_state.bracket_match + 1).min(current_match_count.saturating_sub(1));
            true
        }
        _ => false,
    }
}

fn bracket_counts(app: &App) -> (usize, usize) {
    let round_count = app
        .bracket()
        .state()
        .value()
        .map_or_else(
            || Stage::knockout_order().len(),
            |bracket| bracket.rounds.len(),
        )
        .max(1);
    let current_round = app
        .ui_state
        .bracket_round
        .min(round_count.saturating_sub(1));
    let current_match_count = app
        .bracket()
        .state()
        .value()
        .and_then(|bracket| bracket.rounds.get(current_round))
        .map_or(1, |round| round.matches.len().max(1));
    (round_count, current_match_count)
}

fn clamp_bracket_match(app: &mut App, round_count: usize) {
    let round_index = app
        .ui_state
        .bracket_round
        .min(round_count.saturating_sub(1));
    let match_count = app
        .bracket()
        .state()
        .value()
        .and_then(|bracket| bracket.rounds.get(round_index))
        .map_or(1, |round| round.matches.len().max(1));
    app.ui_state.bracket_match = app
        .ui_state
        .bracket_match
        .min(match_count.saturating_sub(1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;
    use wc_data::domain::MatchStatus;

    fn team(name: &str, abbreviation: &str) -> Team {
        Team {
            id: name.to_owned(),
            name: name.to_owned(),
            abbreviation: abbreviation.to_owned(),
            country_code: None,
            crest_url: None,
        }
    }

    fn fixture(stage: Stage, home: Team, away: Team) -> Match {
        Match {
            id: format!("{}-{}", home.name, away.name),
            stage,
            group: None,
            home,
            away,
            score: None,
            status: MatchStatus::Scheduled,
            kickoff: OffsetDateTime::UNIX_EPOCH,
            venue: None,
        }
    }

    #[test]
    fn tbd_bracket_cell_uses_placeholder_text() {
        let match_ = fixture(
            Stage::RoundOf32,
            Team::placeholder("Winner Group A"),
            Team::placeholder("Third Group C/D/E"),
        );

        let cell = bracket_cell(&match_, true);

        assert_eq!(cell.home, "Winner Group A");
        assert_eq!(cell.away, "Third Group C/D/E");
        assert!(cell.selected);
    }

    #[test]
    fn round_titles_follow_bracket_stage_order() {
        let bracket = Bracket {
            rounds: vec![
                BracketRound {
                    stage: Stage::RoundOf32,
                    matches: vec![fixture(
                        Stage::RoundOf32,
                        team("Canada", "CAN"),
                        team("Italy", "ITA"),
                    )],
                },
                BracketRound {
                    stage: Stage::RoundOf16,
                    matches: vec![fixture(
                        Stage::RoundOf16,
                        team("Brazil", "BRA"),
                        team("Japan", "JPN"),
                    )],
                },
                BracketRound {
                    stage: Stage::QuarterFinal,
                    matches: Vec::new(),
                },
            ],
        };

        assert_eq!(
            round_titles(&bracket),
            vec!["Round of 32", "Round of 16", "Quarter-final"]
        );
    }

    #[test]
    fn visible_rounds_keep_selected_round_in_view() {
        assert_eq!(visible_round_range(6, 4, MIN_COLUMN_WIDTH * 3), 3..6);
        assert_eq!(visible_round_range(6, 0, MIN_COLUMN_WIDTH * 3), 0..3);
    }
}
