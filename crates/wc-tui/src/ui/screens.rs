//! Top-level screen routing and shared rendering helpers.

pub mod widgets;

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::App;
use crate::ui::{screen_bracket, screen_detail, screen_live, screen_matches, screen_standings};

/// The top-level screens, in tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Schedule of fixtures by day/stage.
    Matches,
    /// Live scoreboard of in-play matches.
    Live,
    /// Group tables.
    Standings,
    /// Knockout bracket.
    Bracket,
}

impl Screen {
    /// All screens in tab order.
    #[must_use]
    pub fn all() -> [Screen; 4] {
        [
            Screen::Matches,
            Screen::Live,
            Screen::Standings,
            Screen::Bracket,
        ]
    }

    /// The tab index (0-based).
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Screen::Matches => 0,
            Screen::Live => 1,
            Screen::Standings => 2,
            Screen::Bracket => 3,
        }
    }

    /// Resolve a screen from a tab index, clamped to range.
    #[must_use]
    pub fn from_index(index: usize) -> Screen {
        Screen::all()
            .get(index)
            .copied()
            .unwrap_or(Screen::Matches)
    }

    /// Short label used in the tab bar.
    #[must_use]
    pub fn short(self) -> &'static str {
        match self {
            Screen::Matches => "Matches",
            Screen::Live => "Live",
            Screen::Standings => "Standings",
            Screen::Bracket => "Bracket",
        }
    }
}

/// Render the body for the active screen (or the match-detail overlay).
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    if app.detail().is_some() {
        screen_detail::render(app, frame, area);
        return;
    }
    match app.screen() {
        Screen::Matches => screen_matches::render(app, frame, area),
        Screen::Live => screen_live::render(app, frame, area),
        Screen::Standings => screen_standings::render(app, frame, area),
        Screen::Bracket => screen_bracket::render(app, frame, area),
    }
}

/// Dispatch a key to the active screen. Returns `true` if it was consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    if app.detail().is_some() {
        return screen_detail::handle_key(app, key);
    }
    match app.screen() {
        Screen::Matches => screen_matches::handle_key(app, key),
        Screen::Live => screen_live::handle_key(app, key),
        Screen::Standings => screen_standings::handle_key(app, key),
        Screen::Bracket => screen_bracket::handle_key(app, key),
    }
}
