//! Knockout bracket screen.
//!
//! Renders the knockout tree (Round of 32 → Final, plus the third-place
//! play-off) as scrollable columns.
//!
//! This is a starting stub: it renders the loaded round count and the load
//! state. The full bracket widget is implemented separately.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::ui::screens::widgets;

/// Render the bracket screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::panel("Bracket", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    widgets::remote_message(frame, inner, theme, app.bracket().state(), |bracket| {
        if bracket.rounds.is_empty() {
            return vec![Line::from(Span::styled(
                "The knockout bracket appears once the group stage finishes.",
                Style::new().fg(theme.dim),
            ))];
        }
        vec![
            Line::from(Span::styled(
                format!("{} knockout rounds loaded", bracket.rounds.len()),
                Style::new().fg(theme.fg),
            )),
            Line::from(Span::styled(
                "Bracket view coming soon.",
                Style::new().fg(theme.dim),
            )),
        ]
    });
}

/// Handle a key for the bracket screen. Returns `true` if consumed.
pub fn handle_key(_app: &mut App, _key: KeyEvent) -> bool {
    false
}
