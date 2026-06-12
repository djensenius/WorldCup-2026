//! Standings screen.
//!
//! Shows the 12 group tables (A–L). One group is selected at a time; the table
//! lists P/W/D/L/GF/GA/GD/Pts with qualification highlighting.
//!
//! This is a starting stub: it renders the loaded group count and the load
//! state. The full selectable group tables are implemented separately.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::ui::screens::widgets;

/// Render the standings screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::panel("Standings", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    widgets::remote_message(frame, inner, theme, app.standings().state(), |groups| {
        vec![
            Line::from(Span::styled(
                format!("{} groups loaded", groups.len()),
                Style::new().fg(theme.fg),
            )),
            Line::from(Span::styled(
                "Group tables coming soon.",
                Style::new().fg(theme.dim),
            )),
        ]
    });
}

/// Handle a key for the standings screen. Returns `true` if consumed.
pub fn handle_key(_app: &mut App, _key: KeyEvent) -> bool {
    false
}
