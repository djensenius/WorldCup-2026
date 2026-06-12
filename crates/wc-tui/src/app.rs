//! Application state and the main event loop.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use time::{OffsetDateTime, UtcOffset};
use wc_data::Provider;
use wc_data::domain::{Bracket, Calendar, Group, Match, MatchDetail};

use crate::config::Config;
use crate::data::{Cache, Poller, SharedProvider};
use crate::event::{AppEvent, EventLoop};
use crate::tui::Tui;
use crate::ui;
use crate::ui::icons::Icons;
use crate::ui::screens::{self, Screen};
use crate::ui::theme::{self, Theme};
use crate::ui::toast::Toasts;

/// UI tick cadence (drives toast expiry and polling checks).
const TICK: Duration = Duration::from_millis(250);
/// Scoreboard poll interval while a match is in play.
const LIVE_POLL: Duration = Duration::from_secs(15);
/// Scoreboard poll interval when nothing is live.
const IDLE_POLL: Duration = Duration::from_mins(1);
/// Poll interval for slow-changing data (standings, bracket, calendar).
const SLOW_POLL: Duration = Duration::from_mins(5);
/// Poll interval for an open match-detail view.
const DETAIL_POLL: Duration = Duration::from_secs(20);

/// Cache key for the persisted scoreboard payload.
const CACHE_SCOREBOARD: &str = "scoreboard";
/// Cache key for the persisted standings payload.
const CACHE_STANDINGS: &str = "standings";
/// Cache key for the persisted knockout-bracket payload.
const CACHE_BRACKET: &str = "bracket";
/// Cache key for the persisted competition-calendar payload.
const CACHE_CALENDAR: &str = "calendar";

/// Navigation target for the match-detail overlay.
#[derive(Debug, Clone)]
pub struct DetailNav {
    /// Provider match id to fetch detail for.
    pub match_id: String,
    /// Title shown in the detail panel.
    pub label: String,
}

/// Navigation target for the team overlay (a team's standing, form, fixtures).
#[derive(Debug, Clone)]
pub struct TeamNav {
    /// Provider team id, used to match fixtures and the standings row.
    pub team_id: String,
    /// Team display name.
    pub name: String,
    /// Team short code.
    pub abbreviation: String,
    /// Group letter the team belongs to, when known.
    pub group: Option<String>,
}

/// Mutable per-screen UI state (selections, scroll, filters). Screens read and
/// write these directly; centralising them here keeps the screen modules free
/// of their own state plumbing.
#[derive(Debug, Default)]
pub struct ScreenState {
    /// Selected row on the Matches screen.
    pub matches_selected: usize,
    /// Whether the Matches screen is filtered to favourites only.
    pub matches_favorites_only: bool,
    /// Selected row on the Live screen.
    pub live_selected: usize,
    /// Selected group index (0–11) on the Standings screen.
    pub standings_group: usize,
    /// Selected team row within the current standings group.
    pub standings_row: usize,
    /// Selected fixture row in the team overlay.
    pub team_selected: usize,
    /// Selected round on the Bracket screen.
    pub bracket_round: usize,
    /// Selected match within the bracket round.
    pub bracket_match: usize,
    /// Vertical scroll offset in the detail overlay.
    pub detail_scroll: u16,
}

/// Recorded screen-space x-ranges of the tab labels, captured during render so
/// a mouse click on the tab bar can be mapped back to a screen index.
#[derive(Debug, Default)]
struct TabHitboxes {
    row: u16,
    ranges: Vec<(u16, u16)>,
}

/// The running application.
pub struct App {
    config: Config,
    config_path: PathBuf,
    provider: SharedProvider,
    theme: Theme,
    theme_index: usize,
    icons: Icons,
    toasts: Toasts,
    local_offset: UtcOffset,
    screen: Screen,
    detail: Option<DetailNav>,
    team: Option<TeamNav>,
    show_help: bool,
    should_quit: bool,

    scoreboard: Poller<Vec<Match>>,
    standings: Poller<Vec<Group>>,
    bracket: Poller<Bracket>,
    calendar: Poller<Calendar>,
    detail_poller: Poller<MatchDetail>,
    cache: Cache,
    tab_hitboxes: RefCell<TabHitboxes>,

    /// Mutable per-screen UI state.
    pub ui_state: ScreenState,
}

impl App {
    /// Build the application.
    #[must_use]
    pub fn new(
        config: Config,
        config_path: PathBuf,
        provider: Provider,
        local_offset: UtcOffset,
    ) -> Self {
        let theme_index = theme::NAMES
            .iter()
            .position(|n| *n == config.ui.theme)
            .unwrap_or(0);
        let theme = Theme::from_name(theme::NAMES[theme_index]);
        let icons = Icons::new(config.ui.nerd_fonts);
        let mut toasts = Toasts::default();
        toasts.info("Welcome to wc26. Press ? for help, q to quit.");

        let cache = Cache::new();
        let mut scoreboard = Poller::new();
        if let Some(matches) = cache.load::<Vec<Match>>(CACHE_SCOREBOARD) {
            scoreboard.seed(matches);
        }
        let mut standings = Poller::new();
        if let Some(groups) = cache.load::<Vec<Group>>(CACHE_STANDINGS) {
            standings.seed(groups);
        }
        let mut bracket = Poller::new();
        if let Some(tree) = cache.load::<Bracket>(CACHE_BRACKET) {
            bracket.seed(tree);
        }
        let mut calendar = Poller::new();
        if let Some(cal) = cache.load::<Calendar>(CACHE_CALENDAR) {
            calendar.seed(cal);
        }

        Self {
            config,
            config_path,
            provider: Arc::new(provider),
            theme,
            theme_index,
            icons,
            toasts,
            local_offset,
            screen: Screen::Matches,
            detail: None,
            team: None,
            show_help: false,
            should_quit: false,
            scoreboard,
            standings,
            bracket,
            calendar,
            detail_poller: Poller::new(),
            cache,
            tab_hitboxes: RefCell::new(TabHitboxes::default()),
            ui_state: ScreenState::default(),
        }
    }

    /// Push a warning toast (used by startup for non-fatal config issues).
    pub fn warn(&mut self, message: impl Into<String>) {
        self.toasts.warn(message);
    }

    /// Run the main loop until the user quits.
    ///
    /// # Errors
    /// Returns an error only if drawing to the terminal fails.
    pub async fn run(mut self, terminal: &mut Tui) -> Result<()> {
        let mut events = EventLoop::new(TICK);
        terminal.draw(|frame| ui::render(&self, frame))?;
        loop {
            match events.next().await {
                AppEvent::Tick => self.on_tick(),
                AppEvent::Key(key) => self.on_key(key),
                AppEvent::Mouse(mouse) => self.on_mouse(mouse),
                AppEvent::Resize => {}
                AppEvent::Error(err) => self.toasts.error(err),
            }
            if self.should_quit {
                break;
            }
            terminal.draw(|frame| ui::render(&self, frame))?;
        }
        Ok(())
    }

    // --- accessors used by the UI -----------------------------------------

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The icon set.
    pub fn icons(&self) -> Icons {
        self.icons
    }

    /// The loaded configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The captured local UTC offset (for time formatting).
    pub fn local_offset(&self) -> UtcOffset {
        self.local_offset
    }

    /// The active top-level screen.
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// Whether the help overlay is shown.
    pub fn show_help(&self) -> bool {
        self.show_help
    }

    /// The active toasts.
    pub fn toasts(&self) -> &Toasts {
        &self.toasts
    }

    /// The active match-detail navigation target, if the overlay is open.
    pub fn detail(&self) -> Option<&DetailNav> {
        self.detail.as_ref()
    }

    /// The active team overlay target, if open.
    pub fn team(&self) -> Option<&TeamNav> {
        self.team.as_ref()
    }

    /// The active provider's display name.
    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    /// Whether any tracked resource is currently refreshing.
    pub fn is_refreshing(&self) -> bool {
        self.scoreboard.is_refreshing()
            || self.standings.is_refreshing()
            || self.bracket.is_refreshing()
            || self.calendar.is_refreshing()
            || self.detail_poller.is_refreshing()
    }

    /// Scoreboard data (used by Matches and Live).
    pub fn scoreboard(&self) -> &Poller<Vec<Match>> {
        &self.scoreboard
    }

    /// Group standings.
    pub fn standings(&self) -> &Poller<Vec<Group>> {
        &self.standings
    }

    /// Knockout bracket.
    pub fn bracket(&self) -> &Poller<Bracket> {
        &self.bracket
    }

    /// The label of the calendar stage window currently in progress, or the
    /// next upcoming one, for the status-bar phase indicator.
    pub fn current_stage_label(&self) -> Option<String> {
        let calendar = self.calendar.state().value()?;
        let now = OffsetDateTime::now_utc();
        if let Some(window) = calendar
            .stages
            .iter()
            .find(|w| (w.start..=w.end).contains(&now))
        {
            return Some(window.label.clone());
        }
        calendar
            .stages
            .iter()
            .find(|w| w.start > now)
            .map(|w| format!("Upcoming: {}", w.label))
    }

    /// Age of the data shown on the active screen, for the freshness indicator.
    pub fn active_data_age(&self) -> Option<Duration> {
        if self.detail.is_some() {
            return self.detail_poller.state().age();
        }
        if self.team.is_some() {
            return self.scoreboard.state().age();
        }
        match self.screen {
            Screen::Matches | Screen::Live => self.scoreboard.state().age(),
            Screen::Standings => self.standings.state().age(),
            Screen::Bracket => self.bracket.state().age(),
        }
    }

    /// Match detail for the open overlay.
    pub fn detail_state(&self) -> &Poller<MatchDetail> {
        &self.detail_poller
    }

    /// Whether any displayed resource is currently served from the offline
    /// cache (loaded at startup and not yet refreshed this session).
    pub fn showing_cached(&self) -> bool {
        self.scoreboard.is_stale()
            || self.standings.is_stale()
            || self.bracket.is_stale()
            || self.calendar.is_stale()
    }

    /// Record the tab bar's clickable x-ranges (called by the renderer each
    /// frame) so a mouse click can be mapped back to a screen. `row` is the
    /// bar's y coordinate.
    pub fn set_tab_hitboxes(&self, row: u16, ranges: Vec<(u16, u16)>) {
        let mut hits = self.tab_hitboxes.borrow_mut();
        hits.row = row;
        hits.ranges = ranges;
    }

    fn tab_at(&self, column: u16, row: u16) -> Option<usize> {
        let hits = self.tab_hitboxes.borrow();
        if row != hits.row {
            return None;
        }
        hits.ranges
            .iter()
            .position(|&(start, end)| column >= start && column < end)
    }

    // --- navigation invoked by screens ------------------------------------

    /// Open the match-detail overlay for a fixture and start loading it.
    pub fn open_detail(&mut self, match_id: impl Into<String>, label: impl Into<String>) {
        self.detail = Some(DetailNav {
            match_id: match_id.into(),
            label: label.into(),
        });
        self.detail_poller = Poller::new();
        self.ui_state.detail_scroll = 0;
        self.refresh_detail();
    }

    /// Close the match-detail overlay.
    pub fn close_detail(&mut self) {
        self.detail = None;
    }

    /// Open the team overlay for a team and reset its selection.
    pub fn open_team(
        &mut self,
        team_id: impl Into<String>,
        name: impl Into<String>,
        abbreviation: impl Into<String>,
        group: Option<String>,
    ) {
        self.team = Some(TeamNav {
            team_id: team_id.into(),
            name: name.into(),
            abbreviation: abbreviation.into(),
            group,
        });
        self.ui_state.team_selected = 0;
    }

    /// Close the team overlay.
    pub fn close_team(&mut self) {
        self.team = None;
    }

    /// Toggle a team's favourite status, persist the config, and notify.
    pub fn toggle_favorite(&mut self, name: &str, abbreviation: &str) {
        let now_favorite = self.config.toggle_favorite(name, abbreviation);
        let message = if now_favorite {
            format!("{} Favourited {name}", self.icons.star())
        } else {
            format!("Removed {name} from favourites")
        };
        match self.config.save_to(&self.config_path) {
            Ok(()) => self.toasts.info(message),
            Err(err) => self
                .toasts
                .warn(format!("Could not save favourites: {err}")),
        }
    }

    /// Scroll the detail overlay by `delta` lines (clamped at zero).
    pub fn scroll_detail(&mut self, delta: i16) {
        let next = i32::from(self.ui_state.detail_scroll) + i32::from(delta);
        self.ui_state.detail_scroll = next.clamp(0, i32::from(u16::MAX)) as u16;
    }

    // --- input ------------------------------------------------------------

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if self.show_help {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?' | 'q')) {
                self.show_help = false;
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                return;
            }
            KeyCode::Esc => {
                if self.detail.is_some() {
                    self.close_detail();
                } else if self.team.is_some() {
                    self.close_team();
                }
                return;
            }
            KeyCode::Char('r') => {
                self.refresh_active();
                return;
            }
            KeyCode::Char('t') => {
                self.cycle_theme();
                return;
            }
            KeyCode::Tab if self.detail.is_none() && self.team.is_none() => {
                self.next_screen();
                return;
            }
            KeyCode::BackTab if self.detail.is_none() && self.team.is_none() => {
                self.prev_screen();
                return;
            }
            KeyCode::Char(c @ '1'..='4') if self.detail.is_none() && self.team.is_none() => {
                let index = c as usize - '1' as usize;
                self.screen = Screen::from_index(index);
                return;
            }
            _ => {}
        }
        let _ = screens::handle_key(self, key);
    }

    fn on_mouse(&mut self, mouse: MouseEvent) {
        if self.show_help {
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollDown => self.on_scroll(1),
            MouseEventKind::ScrollUp => self.on_scroll(-1),
            MouseEventKind::Down(MouseButton::Left)
                if self.detail.is_none() && self.team.is_none() =>
            {
                if let Some(index) = self.tab_at(mouse.column, mouse.row) {
                    self.screen = Screen::from_index(index);
                }
            }
            _ => {}
        }
    }

    /// Translate a mouse-wheel notch into movement by scrolling the detail view
    /// when open, otherwise reusing the active screen's up/down key handling.
    fn on_scroll(&mut self, delta: i16) {
        if self.detail.is_some() {
            self.scroll_detail(delta);
            return;
        }
        let code = if delta > 0 {
            KeyCode::Down
        } else {
            KeyCode::Up
        };
        let _ = screens::handle_key(self, KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn next_screen(&mut self) {
        let next = (self.screen.index() + 1) % Screen::all().len();
        self.screen = Screen::from_index(next);
    }

    fn prev_screen(&mut self) {
        let count = Screen::all().len();
        let prev = (self.screen.index() + count - 1) % count;
        self.screen = Screen::from_index(prev);
    }

    fn cycle_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % theme::NAMES.len();
        let name = theme::NAMES[self.theme_index];
        self.theme = Theme::from_name(name);
        name.clone_into(&mut self.config.ui.theme);
        match self.config.save_to(&self.config_path) {
            Ok(()) => self.toasts.info(format!("Theme: {name}")),
            Err(err) => self.toasts.warn(format!("Could not save theme: {err}")),
        }
    }

    // --- polling ----------------------------------------------------------

    fn on_tick(&mut self) {
        self.toasts.expire();
        if matches!(self.scoreboard.drain(), Some(Ok(())))
            && let Some(matches) = self.scoreboard.state().value()
        {
            self.cache.store(CACHE_SCOREBOARD, matches);
        }
        if matches!(self.standings.drain(), Some(Ok(())))
            && let Some(groups) = self.standings.state().value()
        {
            self.cache.store(CACHE_STANDINGS, groups);
        }
        if matches!(self.bracket.drain(), Some(Ok(())))
            && let Some(tree) = self.bracket.state().value()
        {
            self.cache.store(CACHE_BRACKET, tree);
        }
        if matches!(self.calendar.drain(), Some(Ok(())))
            && let Some(cal) = self.calendar.state().value()
        {
            self.cache.store(CACHE_CALENDAR, cal);
        }
        self.detail_poller.drain();

        let interval = if self.any_live() {
            LIVE_POLL
        } else {
            IDLE_POLL
        };
        if self.scoreboard.is_due(interval) {
            self.refresh_scoreboard();
        }
        if self.calendar.is_due(SLOW_POLL) {
            self.refresh_calendar();
        }
        match self.screen {
            Screen::Standings if self.standings.is_due(SLOW_POLL) => self.refresh_standings(),
            Screen::Bracket if self.bracket.is_due(SLOW_POLL) => self.refresh_bracket(),
            _ => {}
        }
        if self.detail.is_some() && self.detail_poller.is_due(DETAIL_POLL) {
            self.refresh_detail();
        }
    }

    fn any_live(&self) -> bool {
        self.scoreboard
            .state()
            .value()
            .is_some_and(|matches| matches.iter().any(|m| m.status.is_live()))
    }

    fn refresh_active(&mut self) {
        if self.detail.is_some() {
            self.refresh_detail();
        } else if self.team.is_some() {
            self.refresh_scoreboard();
            self.refresh_standings();
        } else {
            match self.screen {
                Screen::Matches | Screen::Live => self.refresh_scoreboard(),
                Screen::Standings => self.refresh_standings(),
                Screen::Bracket => self.refresh_bracket(),
            }
        }
        self.toasts.info("Refreshing…");
    }

    fn refresh_scoreboard(&mut self) {
        let provider = Arc::clone(&self.provider);
        self.scoreboard
            .refresh(async move { provider.scoreboard(None).await.map_err(|e| e.to_string()) });
    }

    fn refresh_standings(&mut self) {
        let provider = Arc::clone(&self.provider);
        self.standings
            .refresh(async move { provider.standings().await.map_err(|e| e.to_string()) });
    }

    fn refresh_bracket(&mut self) {
        let provider = Arc::clone(&self.provider);
        self.bracket
            .refresh(async move { provider.bracket().await.map_err(|e| e.to_string()) });
    }

    fn refresh_calendar(&mut self) {
        let provider = Arc::clone(&self.provider);
        self.calendar
            .refresh(async move { provider.calendar().await.map_err(|e| e.to_string()) });
    }

    fn refresh_detail(&mut self) {
        let Some(nav) = &self.detail else { return };
        let id = nav.match_id.clone();
        let provider = Arc::clone(&self.provider);
        self.detail_poller
            .refresh(async move { provider.match_detail(&id).await.map_err(|e| e.to_string()) });
    }
}
