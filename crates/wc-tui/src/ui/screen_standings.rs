//! Standings screen.
//!
//! Shows the 12 group tables (A–L). One group is selected at a time; the table
//! lists P/W/D/L/GF/GA/GD/Pts with qualification highlighting.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};
use wc_data::domain::{Group, GroupStanding};

use crate::app::App;
use crate::data::Remote;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

const GROUP_COUNT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Qualification {
    Qualified,
    Third,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StandingDisplayRow {
    team: String,
    played: String,
    won: String,
    drawn: String,
    lost: String,
    goals_for: String,
    goals_against: String,
    goal_diff: String,
    points: String,
    qualification: Qualification,
}

/// Render the standings screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let selected = app
        .ui_state
        .standings_group
        .min(GROUP_COUNT.saturating_sub(1));
    let title = app
        .standings()
        .state()
        .value()
        .and_then(|groups| groups.get(selected))
        .map_or_else(
            || "Standings".to_owned(),
            |group| format!("Group {}", group.name),
        );
    let block = widgets::panel(&title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.standings().state() {
        Remote::Ready { value: groups, .. } => render_groups(app, frame, inner, groups),
        state => widgets::remote_message(frame, inner, theme, state, |_| Vec::new()),
    }
}

fn render_groups(app: &App, frame: &mut Frame, area: Rect, groups: &[Group]) {
    let theme = app.theme();
    if groups.is_empty() {
        widgets::remote_message(frame, area, theme, app.standings().state(), |_| {
            vec![Line::from(Span::styled(
                "Standings are not available yet.",
                Style::new().fg(theme.dim),
            ))]
        });
        return;
    }

    let selected = app
        .ui_state
        .standings_group
        .min(groups.len().saturating_sub(1));
    let group = &groups[selected];
    if group.standings.is_empty() {
        widgets::message(
            frame,
            area,
            theme,
            vec![Line::from(Span::styled(
                format!("Group {} standings are not available yet.", group.name),
                Style::new().fg(theme.dim),
            ))],
        );
        return;
    }

    let [selector_area, hint_area, table_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
    ])
    .areas(area);

    let selector = group_selector(groups, selected, theme);
    frame.render_widget(Paragraph::new(selector), selector_area);

    let hint = Line::from(vec![
        Span::styled("←/h ", Style::new().fg(theme.dim)),
        Span::styled(app.icons().bullet(), Style::new().fg(theme.dim)),
        Span::styled(" l/→ cycle groups  ", Style::new().fg(theme.dim)),
        Span::styled("top 2 qualify", Style::new().fg(theme.ok)),
        Span::styled("  rank 3", Style::new().fg(theme.warn)),
    ]);
    frame.render_widget(Paragraph::new(hint), hint_area);

    let header = Row::new(["Team", "P", "W", "D", "L", "GF", "GA", "GD", "Pts"])
        .style(Style::new().fg(theme.accent).add_modifier(Modifier::BOLD));
    let rows = standing_rows(group)
        .into_iter()
        .map(|row| row.into_table_row(theme));
    let table = Table::new(
        rows,
        [
            Constraint::Min(16),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ],
    )
    .header(header)
    .column_spacing(1);
    frame.render_widget(table, table_area);
}

fn group_selector(groups: &[Group], selected: usize, theme: &Theme) -> Line<'static> {
    let spans = groups
        .iter()
        .enumerate()
        .flat_map(|(index, group)| {
            let style = if index == selected {
                Style::new()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::new().fg(theme.dim)
            };
            [
                Span::styled(format!(" {} ", group.name), style),
                Span::raw(" "),
            ]
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn standing_rows(group: &Group) -> Vec<StandingDisplayRow> {
    let mut standings = group.standings.clone();
    standings.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then_with(|| b.points.cmp(&a.points))
            .then_with(|| b.goal_diff.cmp(&a.goal_diff))
            .then_with(|| b.goals_for.cmp(&a.goals_for))
            .then_with(|| a.team.name.cmp(&b.team.name))
    });

    standings
        .into_iter()
        .map(StandingDisplayRow::from)
        .collect()
}

impl From<GroupStanding> for StandingDisplayRow {
    fn from(standing: GroupStanding) -> Self {
        let qualification = match standing.rank {
            1 | 2 => Qualification::Qualified,
            3 => Qualification::Third,
            _ => Qualification::Other,
        };
        Self {
            team: format!("{}. {}", standing.rank, standing.team.name),
            played: standing.played.to_string(),
            won: standing.won.to_string(),
            drawn: standing.drawn.to_string(),
            lost: standing.lost.to_string(),
            goals_for: standing.goals_for.to_string(),
            goals_against: standing.goals_against.to_string(),
            goal_diff: format_goal_diff(standing.goal_diff),
            points: standing.points.to_string(),
            qualification,
        }
    }
}

impl StandingDisplayRow {
    fn into_table_row(self, theme: &Theme) -> Row<'static> {
        let style = match self.qualification {
            Qualification::Qualified => Style::new().fg(theme.ok),
            Qualification::Third => Style::new().fg(theme.warn),
            Qualification::Other => Style::new().fg(theme.fg),
        };
        Row::new(vec![
            Cell::from(self.team),
            Cell::from(self.played),
            Cell::from(self.won),
            Cell::from(self.drawn),
            Cell::from(self.lost),
            Cell::from(self.goals_for),
            Cell::from(self.goals_against),
            Cell::from(self.goal_diff),
            Cell::from(self.points),
        ])
        .style(style)
    }
}

fn format_goal_diff(diff: i16) -> String {
    if diff > 0 {
        format!("+{diff}")
    } else {
        diff.to_string()
    }
}

/// Handle a key for the standings screen. Returns `true` if consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('h') | KeyCode::Left => {
            app.ui_state.standings_group =
                (app.ui_state.standings_group + GROUP_COUNT - 1) % GROUP_COUNT;
            true
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => {
            app.ui_state.standings_group = (app.ui_state.standings_group + 1) % GROUP_COUNT;
            true
        }
        KeyCode::Char(c) => select_group(app, c),
        _ => false,
    }
}

fn select_group(app: &mut App, c: char) -> bool {
    let upper = c.to_ascii_uppercase();
    if ('A'..='L').contains(&upper) {
        app.ui_state.standings_group = upper as usize - 'A' as usize;
        return true;
    }
    if let Some(digit) = c.to_digit(10).and_then(|value| usize::try_from(value).ok())
        && (1..=9).contains(&digit)
    {
        app.ui_state.standings_group = digit - 1;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use wc_data::domain::Team;

    fn team(name: &str) -> Team {
        Team {
            id: name.to_owned(),
            name: name.to_owned(),
            abbreviation: name.chars().take(3).collect::<String>().to_uppercase(),
            country_code: None,
            crest_url: None,
        }
    }

    fn standing(name: &str, rank: u8, points: u16, goal_diff: i16) -> GroupStanding {
        GroupStanding {
            team: team(name),
            rank,
            played: 3,
            won: 1,
            drawn: 1,
            lost: 1,
            goals_for: 4,
            goals_against: 3,
            goal_diff,
            points,
        }
    }

    #[test]
    fn top_two_rows_are_marked_as_qualified() {
        let group = Group {
            name: "A".to_owned(),
            standings: vec![
                standing("Third", 3, 4, 0),
                standing("Winner", 1, 7, 3),
                standing("Runner-up", 2, 5, 1),
                standing("Fourth", 4, 1, -4),
            ],
        };

        let rows = standing_rows(&group);

        assert_eq!(rows[0].team, "1. Winner");
        assert_eq!(rows[0].qualification, Qualification::Qualified);
        assert_eq!(rows[1].qualification, Qualification::Qualified);
        assert_eq!(rows[2].qualification, Qualification::Third);
        assert_eq!(rows[3].qualification, Qualification::Other);
    }

    #[test]
    fn goal_difference_has_plus_sign_for_positive_values() {
        assert_eq!(format_goal_diff(2), "+2");
        assert_eq!(format_goal_diff(0), "0");
        assert_eq!(format_goal_diff(-3), "-3");
    }
}
