//! Live scoreboard screen.
//!
//! A compact board of in-play matches that auto-refreshes on a fast cadence and
//! flashes on score changes. `Enter` opens the match detail.
//!
//! This is a starting stub: it renders the count of live matches and the load
//! state. The full live board is implemented separately.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::ui::screens::widgets;

/// Render the live scoreboard screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::panel("Live", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    widgets::remote_message(frame, inner, theme, app.scoreboard().state(), |matches| {
        let live = matches.iter().filter(|m| m.status.is_live()).count();
        vec![
            Line::from(Span::styled(
                format!("{live} live now"),
                Style::new().fg(if live > 0 { theme.warn } else { theme.dim }),
            )),
            Line::from(Span::styled(
                "Live board coming soon.",
                Style::new().fg(theme.dim),
            )),
        ]
    });
}

/// Handle a key for the live screen. Returns `true` if consumed.
pub fn handle_key(_app: &mut App, _key: KeyEvent) -> bool {
    false
}
