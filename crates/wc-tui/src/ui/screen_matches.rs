//! Matches / schedule screen.
//!
//! Lists fixtures grouped by day and stage with status badges, local-timezone
//! kickoff times, and a favourites filter. `Enter` opens the match detail.
//!
//! This is a starting stub: it renders the loaded fixture count and the load
//! state. The full grouped/scrollable list is implemented separately.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::ui::screens::widgets;

/// Render the matches screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::panel("Matches", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    widgets::remote_message(frame, inner, theme, app.scoreboard().state(), |matches| {
        vec![
            Line::from(Span::styled(
                format!("{} fixtures loaded", matches.len()),
                Style::new().fg(theme.fg),
            )),
            Line::from(Span::styled(
                "Schedule view coming soon.",
                Style::new().fg(theme.dim),
            )),
        ]
    });
}

/// Handle a key for the matches screen. Returns `true` if consumed.
pub fn handle_key(_app: &mut App, _key: KeyEvent) -> bool {
    false
}
