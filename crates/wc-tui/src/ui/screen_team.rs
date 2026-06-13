//! Team overlay.
//!
//! Opened from the standings screen (`Enter` on a row), this overlay shows a
//! single team's group standing, recent form, and full list of fixtures. It
//! derives everything from the already-loaded scoreboard and standings pollers,
//! so it needs no extra provider calls.
//!
//! Keys: `j`/`k` (or arrows) move the fixture selection, `Enter` opens the
//! match detail for the selected fixture, `*` toggles the team as a favourite,
//! and `Esc` closes the overlay.

use std::cmp::Ordering;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use time::UtcOffset;
use wc_data::domain::{Group, GroupStanding, Match, MatchStatus, Team};

use crate::app::{App, TeamNav};
use crate::config::TimezonePref;
use crate::data::Remote;
use crate::timefmt;
use crate::ui::flag_image;
use crate::ui::icons::Icons;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

/// A fixture paired with whether the focused team played at home.
type Fixture<'a> = (&'a Match, bool);

/// Render the team overlay.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let icons = app.icons();
    let Some(nav) = app.team() else {
        return;
    };

    let is_fav = app.config().is_favorite(&nav.name, &nav.abbreviation);
    let title = team_title(nav, is_fav, icons);
    let block = widgets::screen_block(
        &title,
        "j/k move · Enter match · * favourite · Esc back",
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let state = app.scoreboard().state();
    let Remote::Ready { value: matches, .. } = state else {
        widgets::remote_message(frame, inner, theme, state, |_| Vec::new());
        return;
    };

    let fixtures = team_fixtures(matches, &nav.team_id, &nav.name);
    let standing = app
        .standings()
        .state()
        .value()
        .and_then(|groups| find_standing(groups, &nav.team_id, &nav.name));

    let header = header_lines(nav, standing, &fixtures, theme);
    let header_height = u16::try_from(header.len()).unwrap_or(0);
    let [header_area, list_area] =
        Layout::vertical([Constraint::Length(header_height), Constraint::Min(1)]).areas(inner);
    frame.render_widget(
        Paragraph::new(header).style(Style::new().fg(theme.fg)),
        header_area,
    );

    if fixtures.is_empty() {
        widgets::message(
            frame,
            list_area,
            theme,
            vec![Line::from(Span::styled(
                "No fixtures loaded for this team yet",
                Style::new().fg(theme.dim),
            ))],
        );
        return;
    }

    let selected = app
        .ui_state
        .team_selected
        .min(fixtures.len().saturating_sub(1));
    let pref = &app.config().ui.timezone;
    let offset = app.local_offset();
    let show_flags = app.config().ui.show_flags;
    let (lines, placements) = fixture_lines(
        &fixtures,
        selected,
        usize::from(list_area.height),
        FixtureCtx {
            theme,
            icons,
            pref,
            offset,
            show_flags,
        },
    );
    frame.render_widget(
        Paragraph::new(lines).style(Style::new().fg(theme.fg)),
        list_area,
    );
    for (row_offset, code) in placements {
        let y = list_area.y + row_offset;
        flag_image::render_inline(
            app.flags(),
            frame,
            &code,
            Rect::new(list_area.x + TEAM_FLAG_X, y, TEAM_FLAG_COLS, 1),
        );
    }
}

/// Inline flag width (cells) and x-offset within a fixture row.
const TEAM_FLAG_COLS: u16 = 4;
const TEAM_FLAG_X: u16 = 17;

/// Handle a key for the team overlay. Returns `true` if consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let len = fixtures_len(app);
            if len > 0 {
                app.ui_state.team_selected = (app.ui_state.team_selected + 1).min(len - 1);
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.ui_state.team_selected = app.ui_state.team_selected.saturating_sub(1);
            true
        }
        KeyCode::Enter => {
            if let (Some(nav), Some(matches)) = (app.team(), app.scoreboard().state().value()) {
                let fixtures = team_fixtures(matches, &nav.team_id, &nav.name);
                let idx = app
                    .ui_state
                    .team_selected
                    .min(fixtures.len().saturating_sub(1));
                if let Some((m, _)) = fixtures.get(idx).copied() {
                    let id = m.id.clone();
                    let label = match_label(m);
                    app.open_detail(id, label);
                }
            }
            true
        }
        KeyCode::Char('*') => {
            if let Some(nav) = app.team() {
                let name = nav.name.clone();
                let abbreviation = nav.abbreviation.clone();
                app.toggle_favorite(&name, &abbreviation);
            }
            true
        }
        _ => false,
    }
}

/// Number of fixtures currently available for the open team (0 if none).
fn fixtures_len(app: &App) -> usize {
    match (app.team(), app.scoreboard().state().value()) {
        (Some(nav), Some(matches)) => team_fixtures_count(matches, &nav.team_id, &nav.name),
        _ => 0,
    }
}

fn team_fixtures_count(matches: &[Match], id: &str, name: &str) -> usize {
    matches
        .iter()
        .filter(|m| team_is_home(m, id, name).is_some())
        .count()
}

/// The panel title: an optional favourite star, the team name, and its code.
fn team_title(nav: &TeamNav, is_fav: bool, icons: Icons) -> String {
    let star = if is_fav {
        format!("{} ", icons.star())
    } else {
        String::new()
    };
    if nav.abbreviation.is_empty() {
        format!("{star}{}", nav.name)
    } else {
        format!("{star}{} ({})", nav.name, nav.abbreviation)
    }
}

/// Header lines: the group-standing summary, recent form, and a section label.
fn header_lines(
    nav: &TeamNav,
    standing: Option<&GroupStanding>,
    fixtures: &[Fixture<'_>],
    theme: &Theme,
) -> Vec<Line<'static>> {
    let dim = Style::new().fg(theme.dim);
    let accent = Style::new().fg(theme.accent);
    let accent_bold = accent.add_modifier(Modifier::BOLD);

    let standing_line = standing.map_or_else(
        || {
            let label = nav.group.as_ref().map_or_else(
                || "Standing unavailable".to_owned(),
                |group| format!("Group {group} — standing unavailable"),
            );
            Line::from(Span::styled(label, dim))
        },
        |s| {
            let mut spans = Vec::new();
            if let Some(group) = &nav.group {
                spans.push(Span::styled(format!("Group {group}"), accent_bold));
                spans.push(Span::styled("    ", dim));
            }
            spans.push(Span::styled(format!("Rank {}", s.rank), accent));
            spans.push(Span::styled(
                format!(
                    "    Pld {}    {}W-{}D-{}L    GF {} GA {}    GD {:+}    ",
                    s.played, s.won, s.drawn, s.lost, s.goals_for, s.goals_against, s.goal_diff
                ),
                dim,
            ));
            spans.push(Span::styled(
                format!("{} pts", s.points),
                Style::new().fg(theme.ok).add_modifier(Modifier::BOLD),
            ));
            Line::from(spans)
        },
    );

    let mut form = vec![Span::styled("Form  ", dim)];
    let recent = form_spans(fixtures, theme);
    if recent.is_empty() {
        form.push(Span::styled("—", dim));
    } else {
        form.extend(recent);
    }

    vec![
        standing_line,
        Line::from(form),
        Line::from(""),
        Line::from(Span::styled(
            "Fixtures",
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
    ]
}

/// The last (up to five) finished results as coloured W/D/L spans.
fn form_spans(fixtures: &[Fixture<'_>], theme: &Theme) -> Vec<Span<'static>> {
    let results = fixtures
        .iter()
        .filter_map(|&(m, is_home)| outcome(m, is_home))
        .collect::<Vec<_>>();
    let start = results.len().saturating_sub(5);
    results[start..]
        .iter()
        .flat_map(|o| {
            [
                Span::styled(o.letter().to_owned(), o.style(theme)),
                Span::raw(" "),
            ]
        })
        .collect()
}

/// Shared context for rendering fixture rows.
#[derive(Clone, Copy)]
struct FixtureCtx<'a> {
    theme: &'a Theme,
    icons: Icons,
    pref: &'a TimezonePref,
    offset: UtcOffset,
    show_flags: bool,
}

/// Build the windowed fixture rows that fit in `height` lines, plus the flag
/// overlay placements (visible-row offset and opponent code) for each row.
fn fixture_lines(
    rows: &[Fixture<'_>],
    selected: usize,
    height: usize,
    ctx: FixtureCtx,
) -> (Vec<Line<'static>>, Vec<(u16, String)>) {
    let available = height.max(1);
    let start = selected.saturating_sub(available.saturating_sub(1));
    let mut lines = Vec::new();
    let mut placements = Vec::new();
    for (offset, (index, &entry)) in rows
        .iter()
        .enumerate()
        .skip(start)
        .take(available)
        .enumerate()
    {
        lines.push(fixture_row_line(entry, index == selected, ctx));
        if ctx.show_flags
            && let Ok(row_offset) = u16::try_from(offset)
        {
            let (m, is_home) = entry;
            let opponent = if is_home { &m.away } else { &m.home };
            placements.push((row_offset, opponent.abbreviation.clone()));
        }
    }
    (lines, placements)
}

/// A single selectable fixture row.
fn fixture_row_line(entry: Fixture<'_>, selected: bool, ctx: FixtureCtx) -> Line<'static> {
    let FixtureCtx {
        theme,
        icons,
        pref,
        offset,
        show_flags,
    } = ctx;
    let (m, is_home) = entry;
    let base = row_base_style(m, selected, theme);
    let mark_style = if selected {
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        base
    };
    let mark = if selected { "›" } else { " " };
    let date = timefmt::date_heading(m.kickoff, pref, offset);
    let side = if is_home { "vs" } else { "@ " };
    let opponent = if is_home { &m.away } else { &m.home };
    let opponent_name = truncate(&display_name(opponent), 18);

    let mut spans = vec![
        Span::styled(format!("{mark} "), mark_style),
        Span::styled(format!("{date:<11} "), Style::new().fg(theme.dim)),
        Span::styled(format!("{side} "), Style::new().fg(theme.dim)),
    ];
    if show_flags {
        // Blank slot for an overlaid flag image (see TEAM_FLAG_X).
        spans.push(Span::raw(" ".repeat(usize::from(TEAM_FLAG_COLS) + 1)));
    }
    spans.push(Span::styled(format!("{opponent_name:<18} "), base));

    if has_started(m) {
        spans.push(Span::styled(
            format!("{:^7}", score_text(m)),
            score_style(m, theme),
        ));
    } else {
        spans.push(Span::styled(
            format!("{:^7}", timefmt::time_hm(m.kickoff, pref, offset)),
            Style::new().fg(theme.fg),
        ));
    }

    spans.push(Span::raw("  "));
    spans.push(status_badge(m, is_home, theme, icons));
    Line::from(spans)
}

/// Base text style for a fixture row (finished rows are de-emphasised).
fn row_base_style(m: &Match, selected: bool, theme: &Theme) -> Style {
    let mut style = if m.status.is_finished() {
        Style::new().fg(theme.dim)
    } else {
        Style::new().fg(theme.fg)
    };
    if selected {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// The trailing badge: result letter (finished), clock (live), or empty.
fn status_badge(m: &Match, is_home: bool, theme: &Theme, icons: Icons) -> Span<'static> {
    if m.status.is_live() {
        Span::styled(
            format!("{} {}", icons.live(), live_clock(&m.status)),
            Style::new().fg(theme.warn).add_modifier(Modifier::BOLD),
        )
    } else if let Some(o) = outcome(m, is_home) {
        Span::styled(
            o.letter().to_owned(),
            o.style(theme).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    }
}

/// Whether a fixture has a score worth showing (in play or finished).
fn has_started(m: &Match) -> bool {
    m.status.is_live() || m.status.is_finished()
}

/// "home-away" score text, defaulting to "0-0" before any goals are reported.
fn score_text(m: &Match) -> String {
    m.score
        .map_or_else(|| "0-0".to_owned(), |s| format!("{}-{}", s.home, s.away))
}

/// Style for the score cell: warn while live, otherwise a readable bold.
fn score_style(m: &Match, theme: &Theme) -> Style {
    if m.status.is_live() {
        Style::new().fg(theme.warn).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
    }
}

/// The live clock label, e.g. "45'", "HT", or "LIVE".
fn live_clock(status: &MatchStatus) -> String {
    match status {
        MatchStatus::Live { minute, detail } => minute.map_or_else(
            || detail.clone().unwrap_or_else(|| "LIVE".to_owned()),
            |m| format!("{m}'"),
        ),
        MatchStatus::HalfTime => "HT".to_owned(),
        _ => String::new(),
    }
}

/// The fixtures involving the focused team, paired with the home/away flag and
/// sorted chronologically.
fn team_fixtures<'a>(matches: &'a [Match], id: &str, name: &str) -> Vec<Fixture<'a>> {
    let mut fixtures = matches
        .iter()
        .filter_map(|m| team_is_home(m, id, name).map(|is_home| (m, is_home)))
        .collect::<Vec<_>>();
    fixtures.sort_by(|a, b| {
        a.0.kickoff
            .cmp(&b.0.kickoff)
            .then_with(|| a.0.id.cmp(&b.0.id))
    });
    fixtures
}

/// Whether the focused team plays this fixture, and on which side (`true` =
/// home). Returns `None` if the team is not involved.
fn team_is_home(m: &Match, id: &str, name: &str) -> Option<bool> {
    if team_matches(&m.home, id, name) {
        Some(true)
    } else if team_matches(&m.away, id, name) {
        Some(false)
    } else {
        None
    }
}

/// The focused team's standing row, if it appears in any group.
fn find_standing<'a>(groups: &'a [Group], id: &str, name: &str) -> Option<&'a GroupStanding> {
    groups
        .iter()
        .flat_map(|group| &group.standings)
        .find(|s| team_matches(&s.team, id, name))
}

/// Match a team by provider id (when known) or a case-insensitive name.
fn team_matches(team: &Team, id: &str, name: &str) -> bool {
    (!id.is_empty() && team.id == id) || team.name.eq_ignore_ascii_case(name)
}

/// The result of a finished fixture from the focused team's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Win,
    Draw,
    Loss,
}

impl Outcome {
    /// The single-letter label.
    fn letter(self) -> &'static str {
        match self {
            Outcome::Win => "W",
            Outcome::Draw => "D",
            Outcome::Loss => "L",
        }
    }

    /// The themed colour for this outcome.
    fn style(self, theme: &Theme) -> Style {
        let color = match self {
            Outcome::Win => theme.ok,
            Outcome::Draw => theme.warn,
            Outcome::Loss => theme.error,
        };
        Style::new().fg(color)
    }
}

/// The outcome of a fixture for the focused team, or `None` if unfinished.
fn outcome(m: &Match, is_home: bool) -> Option<Outcome> {
    if !m.status.is_finished() {
        return None;
    }
    let score = m.score?;
    let (mine, theirs) = if is_home {
        (score.home, score.away)
    } else {
        (score.away, score.home)
    };
    match mine.cmp(&theirs) {
        Ordering::Greater => Some(Outcome::Win),
        Ordering::Less => Some(Outcome::Loss),
        Ordering::Equal => match (score.home_pens, score.away_pens) {
            (Some(home_pens), Some(away_pens)) => {
                let (mine_pens, their_pens) = if is_home {
                    (home_pens, away_pens)
                } else {
                    (away_pens, home_pens)
                };
                match mine_pens.cmp(&their_pens) {
                    Ordering::Greater => Some(Outcome::Win),
                    Ordering::Less => Some(Outcome::Loss),
                    Ordering::Equal => Some(Outcome::Draw),
                }
            }
            _ => Some(Outcome::Draw),
        },
    }
}

/// A short label for the detail overlay title.
fn match_label(m: &Match) -> String {
    format!("{} vs {}", display_name(&m.home), display_name(&m.away))
}

/// A team's display name, falling back to its code when the name is empty.
fn display_name(team: &Team) -> String {
    if team.name.is_empty() {
        team.abbreviation.clone()
    } else {
        team.name.clone()
    }
}

/// Truncate `s` to at most `max` characters, appending an ellipsis when cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let keep = max.saturating_sub(1);
    let mut out = s.chars().take(keep).collect::<String>();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;
    use wc_data::domain::{Score, Stage};

    fn team(name: &str, abbreviation: &str) -> Team {
        Team {
            id: abbreviation.to_lowercase(),
            name: name.to_owned(),
            abbreviation: abbreviation.to_owned(),
            country_code: None,
            crest_url: None,
        }
    }

    fn fixture(
        home: &str,
        away: &str,
        status: MatchStatus,
        score: Option<Score>,
        ts: i64,
    ) -> Match {
        Match {
            id: format!("{home}-{away}"),
            stage: Stage::GroupStage,
            group: Some("B".to_owned()),
            home: team(home, &home[..3.min(home.len())].to_uppercase()),
            away: team(away, &away[..3.min(away.len())].to_uppercase()),
            score,
            status,
            kickoff: OffsetDateTime::from_unix_timestamp(ts).unwrap_or(OffsetDateTime::UNIX_EPOCH),
            venue: None,
        }
    }

    fn score(home: u8, away: u8) -> Score {
        Score {
            home,
            away,
            home_pens: None,
            away_pens: None,
        }
    }

    #[test]
    fn team_fixtures_filters_and_sorts_chronologically() {
        let matches = vec![
            fixture("Canada", "Mexico", MatchStatus::Scheduled, None, 300),
            fixture("Spain", "Brazil", MatchStatus::Scheduled, None, 100),
            fixture("Brazil", "Canada", MatchStatus::Scheduled, None, 200),
        ];
        let fixtures = team_fixtures(&matches, "can", "Canada");
        assert_eq!(fixtures.len(), 2);
        // Sorted by kickoff: Brazil(200) before Canada(300).
        assert_eq!(fixtures[0].0.id, "Brazil-Canada");
        assert!(!fixtures[0].1, "Canada is away vs Brazil");
        assert_eq!(fixtures[1].0.id, "Canada-Mexico");
        assert!(fixtures[1].1, "Canada is home vs Mexico");
    }

    #[test]
    fn team_matches_by_name_when_id_missing() {
        let placeholder = Team {
            id: String::new(),
            name: "Canada".to_owned(),
            abbreviation: "CAN".to_owned(),
            country_code: None,
            crest_url: None,
        };
        assert!(team_matches(&placeholder, "", "canada"));
        assert!(!team_matches(&placeholder, "xyz", "Mexico"));
    }

    #[test]
    fn outcome_reflects_team_side() {
        let win_home = fixture(
            "Canada",
            "Mexico",
            MatchStatus::FullTime,
            Some(score(2, 1)),
            10,
        );
        assert_eq!(outcome(&win_home, true), Some(Outcome::Win));
        assert_eq!(outcome(&win_home, false), Some(Outcome::Loss));

        let draw = fixture(
            "Canada",
            "Mexico",
            MatchStatus::FullTime,
            Some(score(1, 1)),
            10,
        );
        assert_eq!(outcome(&draw, true), Some(Outcome::Draw));

        let scheduled = fixture("Canada", "Mexico", MatchStatus::Scheduled, None, 10);
        assert_eq!(outcome(&scheduled, true), None);
    }

    #[test]
    fn outcome_uses_penalties_when_drawn() {
        let mut pens = fixture(
            "Canada",
            "Mexico",
            MatchStatus::Penalties,
            Some(score(1, 1)),
            10,
        );
        pens.score = Some(Score {
            home: 1,
            away: 1,
            home_pens: Some(4),
            away_pens: Some(2),
        });
        assert_eq!(outcome(&pens, true), Some(Outcome::Win));
        assert_eq!(outcome(&pens, false), Some(Outcome::Loss));
    }

    #[test]
    fn form_spans_keeps_last_five_results() {
        let mut matches = Vec::new();
        for i in 0..7 {
            matches.push(fixture(
                "Canada",
                "Mexico",
                MatchStatus::FullTime,
                Some(score(1, 0)),
                i64::from(i),
            ));
        }
        let fixtures = team_fixtures(&matches, "can", "Canada");
        let theme = Theme::world_night();
        let spans = form_spans(&fixtures, &theme);
        // Five results, each followed by a spacer span.
        assert_eq!(spans.len(), 10);
    }

    #[test]
    fn truncate_adds_ellipsis_when_too_long() {
        assert_eq!(truncate("Switzerland", 6), "Switz…");
        assert_eq!(truncate("Spain", 6), "Spain");
    }
}
