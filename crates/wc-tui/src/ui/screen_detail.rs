//! Match detail overlay.
//!
//! Opened from the Matches or Live screens for a specific fixture. Shows the
//! timeline (goals/cards/subs), lineups, and team statistics. `Esc` returns to
//! the previous screen (handled by the app); `j`/`k` scroll.
//!
//! This is a starting stub: it renders the header and the load state. The full
//! detail layout is implemented separately.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::ui::screens::widgets;

/// Render the match detail overlay.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let title = app
        .detail()
        .map_or_else(|| "Match".to_owned(), |d| d.label.clone());
    let block = widgets::panel(&title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    widgets::remote_message(frame, inner, theme, app.detail_state().state(), |detail| {
        vec![
            Line::from(Span::styled(
                format!(
                    "{} vs {}",
                    detail.summary.home.name, detail.summary.away.name
                ),
                Style::new().fg(theme.fg),
            )),
            Line::from(Span::styled(
                format!("{} timeline events", detail.events.len()),
                Style::new().fg(theme.dim),
            )),
            Line::from(Span::styled(
                "Detail view coming soon. Press Esc to go back.",
                Style::new().fg(theme.dim),
            )),
        ]
    });
}

/// Handle a key for the detail overlay. Returns `true` if consumed. `Esc` is
/// handled globally by the app to close the overlay.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.scroll_detail(1);
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.scroll_detail(-1);
            true
        }
        _ => false,
    }
}
