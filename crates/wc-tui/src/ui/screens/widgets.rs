//! Small shared rendering helpers used across screens.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::data::Remote;
use crate::ui::theme::Theme;

/// A bordered panel with a themed title.
#[must_use]
pub fn panel(title: &str, theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.dim))
        .title(format!(" {title} "))
        .title_style(Style::new().fg(theme.accent).add_modifier(Modifier::BOLD))
}

/// A horizontally and vertically centred sub-rect of the given size.
#[must_use]
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(row);
    cell
}

/// Render a centred status message (used for empty/loading/error states).
pub fn message(frame: &mut Frame, area: Rect, theme: &Theme, lines: Vec<Line<'static>>) {
    let height = u16::try_from(lines.len()).unwrap_or(1).max(1) + 2;
    let rect = centered(area, area.width.saturating_sub(8).min(70), height);
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::new().fg(theme.fg));
    frame.render_widget(paragraph, rect);
}

/// Render a standard view for a [`Remote`] resource: idle, loading, error, or a
/// caller-provided rendering of the loaded value. Used for empty/error states
/// and as a starting point for each screen.
pub fn remote_message<T>(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    state: &Remote<T>,
    when_ready: impl FnOnce(&T) -> Vec<Line<'static>>,
) {
    let lines = match state {
        Remote::Idle => vec![Line::from(Span::styled(
            "Waiting to load…",
            Style::new().fg(theme.dim),
        ))],
        Remote::Loading => vec![Line::from(Span::styled("Loading…", Style::new().fg(theme.warn)))],
        Remote::Failed { error, .. } => vec![
            Line::from(Span::styled(
                "Could not load data",
                Style::new().fg(theme.error).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(error.clone(), Style::new().fg(theme.dim))),
        ],
        Remote::Ready { value, .. } => when_ready(value),
    };
    message(frame, area, theme, lines);
}
