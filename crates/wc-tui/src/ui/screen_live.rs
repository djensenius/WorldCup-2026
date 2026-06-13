//! Live scoreboard screen — a glanceable "Live Activity" card.
//!
//! Shows one match at a time, big enough to read across a room: a large
//! block-digit score flanked by colored ASCII-art flags, the clock, and the
//! most recent event (goal/card). `j`/`k` cycles through the in-play matches
//! (or the soonest upcoming fixtures when nothing is live, with a countdown),
//! `Enter` opens the full match detail, and `f` toggles the flags.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui_image::Image;
use time::OffsetDateTime;
use wc_data::domain::{Match, MatchEvent, MatchEventKind, MatchStatus, Score, Stage, Team};

use crate::app::App;
use crate::data::Remote;
use crate::timefmt;
use crate::ui::icons::Icons;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

/// How many upcoming fixtures to cycle through when nothing is live.
const UPCOMING_LIMIT: usize = 24;
/// Flag image size in cells (about 4:3 at a typical 1:2 cell aspect).
const FLAG_COLS: u16 = 14;
const FLAG_ROWS: u16 = 5;
/// Rows occupied by the big-score glyphs.
const SCORE_ROWS: u16 = 5;
/// Horizontal gap (cells) between a flag and the score.
const FLAG_GAP: u16 = 3;

/// Render the live screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let block = widgets::screen_block("Live", "j/k switch · Enter details · f flags", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let state = app.scoreboard().state();
    let Remote::Ready { value: matches, .. } = state else {
        widgets::remote_message(frame, inner, theme, state, |_| Vec::new());
        return;
    };

    let live = live_matches(matches);
    let upcoming = upcoming_matches(matches);
    let (cards, is_live) = if live.is_empty() {
        (upcoming, false)
    } else {
        (live, true)
    };
    if cards.is_empty() {
        widgets::message(
            frame,
            inner,
            theme,
            vec![Line::from(Span::styled(
                "No matches in play, and no upcoming fixtures loaded.",
                Style::new().fg(theme.dim),
            ))],
        );
        return;
    }

    let selected = app.ui_state.live_selected.min(cards.len() - 1);
    let m = cards[selected];
    let score = score_centre(app, m, is_live);
    let score_w = big_width(&score);
    let score_color = if is_live { theme.warn } else { theme.accent };

    // A flag column is shown either side of the score when flags are enabled,
    // both teams have a flag, and there is room.
    let want_flags = app.config().ui.show_flags
        && app.flags().is_some()
        && flag_available(app, &m.home.abbreviation)
        && flag_available(app, &m.away.abbreviation);
    let block_w = if want_flags {
        FLAG_COLS * 2 + FLAG_GAP * 2 + score_w
    } else {
        score_w
    };
    let flags = want_flags && block_w <= inner.width;

    // Vertically centred block: status, gap, names, body (flags/score), gap,
    // context, gap, pager.
    let body_h = FLAG_ROWS.max(SCORE_ROWS);
    let total_h = body_h + 7;
    let top = inner.y + inner.height.saturating_sub(total_h) / 2;
    let names_y = top + 2;
    let body_y = top + 3;
    let context_y = body_y + body_h + 1;
    let pager_y = context_y + 2;

    let full = |y: u16| Rect::new(inner.x, y, inner.width, 1);
    centered(frame, full(top), status_line(app, m, is_live, theme));

    if flags {
        let block_x = (inner.x + inner.width / 2).saturating_sub(block_w / 2);
        let home_x = block_x;
        let score_x = block_x + FLAG_COLS + FLAG_GAP;
        let away_x = score_x + score_w + FLAG_GAP;
        let flag_y = body_y + (body_h - FLAG_ROWS) / 2;
        let score_y = body_y + (body_h - SCORE_ROWS) / 2;

        centered(
            frame,
            Rect::new(home_x, names_y, FLAG_COLS, 1),
            team_name_line(app, &m.home, FLAG_COLS, theme),
        );
        centered(
            frame,
            Rect::new(away_x, names_y, FLAG_COLS, 1),
            team_name_line(app, &m.away, FLAG_COLS, theme),
        );
        render_flag(
            app,
            frame,
            &m.home.abbreviation,
            Rect::new(home_x, flag_y, FLAG_COLS, FLAG_ROWS),
        );
        render_flag(
            app,
            frame,
            &m.away.abbreviation,
            Rect::new(away_x, flag_y, FLAG_COLS, FLAG_ROWS),
        );
        frame.render_widget(
            Paragraph::new(big_glyphs(&score, score_color)),
            Rect::new(score_x, score_y, score_w, SCORE_ROWS),
        );
    } else {
        centered(frame, full(names_y), names_line(app, m, theme));
        let score_y = body_y + (body_h - SCORE_ROWS) / 2;
        frame.render_widget(
            Paragraph::new(big_glyphs(&score, score_color)).alignment(Alignment::Center),
            Rect::new(inner.x, score_y, inner.width, SCORE_ROWS),
        );
    }

    let context = if is_live {
        event_line(app, m, theme)
    } else {
        Line::from(Span::styled(context_tag(m), Style::new().fg(theme.dim)))
    };
    centered(frame, full(context_y), context);

    let label = if is_live { "live" } else { "upcoming" };
    centered(
        frame,
        full(pager_y),
        Line::from(Span::styled(
            format!("‹ {} / {} {label} ›", selected + 1, cards.len()),
            Style::new().fg(theme.dim),
        )),
    );
}

/// Render a single line centred within `rect`.
fn centered(frame: &mut Frame, rect: Rect, line: Line<'static>) {
    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), rect);
}

/// The text shown big and centred: the live score, or the kickoff time.
fn score_centre(app: &App, m: &Match, is_live: bool) -> String {
    if is_live {
        score_text(m.score)
    } else {
        timefmt::time_hm(m.kickoff, &app.config().ui.timezone, app.local_offset())
    }
}

fn flag_available(app: &App, code: &str) -> bool {
    app.flags().is_some_and(|store| {
        store
            .borrow_mut()
            .flag(code, FLAG_COLS, FLAG_ROWS)
            .is_some()
    })
}

/// Draw a team's flag image into `rect`, if a flag exists for the code.
fn render_flag(app: &App, frame: &mut Frame, code: &str, rect: Rect) {
    let Some(store) = app.flags() else {
        return;
    };
    let mut store = store.borrow_mut();
    if let Some(protocol) = store.flag(code, rect.width, rect.height) {
        frame.render_widget(Image::new(protocol), rect);
    }
}

/// A single team's name (or abbreviation when it would overflow `width`),
/// styled bold and accented when the team is a favourite.
fn team_name_line(app: &App, team: &Team, width: u16, theme: &Theme) -> Line<'static> {
    let fav = app.config().is_favorite(&team.name, &team.abbreviation);
    let style = if fav {
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
    };
    let name = if team.name.chars().count() <= usize::from(width) || team.abbreviation.is_empty() {
        team.name.clone()
    } else {
        team.abbreviation.clone()
    };
    Line::from(Span::styled(name, style))
}

/// Width in cells of a big-glyph string (each glyph is 4 wide, 1-cell spaced).
fn big_width(text: &str) -> u16 {
    let n = u16::try_from(text.chars().count()).unwrap_or(0);
    n.saturating_mul(4) + n.saturating_sub(1)
}

/// Handle a key for the live screen. Returns `true` if consumed.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(matches) = app.scoreboard().state().value() else {
        return false;
    };
    let live = live_matches(matches);
    let len = if live.is_empty() {
        upcoming_matches(matches).len()
    } else {
        live.len()
    };
    match key.code {
        KeyCode::Char('j') | KeyCode::Down | KeyCode::Right => {
            if len > 0 {
                app.ui_state.live_selected = (app.ui_state.live_selected + 1).min(len - 1);
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up | KeyCode::Left => {
            app.ui_state.live_selected = app.ui_state.live_selected.saturating_sub(1);
            true
        }
        KeyCode::Char('f') => {
            app.toggle_flags();
            true
        }
        KeyCode::Enter => {
            let cards = current_cards(matches);
            if let Some(m) = cards.get(app.ui_state.live_selected.min(len.saturating_sub(1))) {
                app.open_detail(m.id.clone(), match_label(m));
            }
            true
        }
        _ => false,
    }
}

/// The id of the focused match if it is in play (drives the live-detail poll).
#[must_use]
pub fn focused_live_id(app: &App) -> Option<String> {
    let matches = app.scoreboard().state().value()?;
    let live = live_matches(matches);
    if live.is_empty() {
        return None;
    }
    let selected = app.ui_state.live_selected.min(live.len() - 1);
    Some(live[selected].id.clone())
}

fn current_cards(matches: &[Match]) -> Vec<&Match> {
    let live = live_matches(matches);
    if live.is_empty() {
        upcoming_matches(matches)
    } else {
        live
    }
}

fn live_matches(matches: &[Match]) -> Vec<&Match> {
    let mut live = matches
        .iter()
        .filter(|m| m.status.is_live())
        .collect::<Vec<_>>();
    live.sort_by(|a, b| a.kickoff.cmp(&b.kickoff).then_with(|| a.id.cmp(&b.id)));
    live
}

fn upcoming_matches(matches: &[Match]) -> Vec<&Match> {
    let mut upcoming = matches
        .iter()
        .filter(|m| matches!(m.status, MatchStatus::Scheduled))
        .collect::<Vec<_>>();
    upcoming.sort_by(|a, b| a.kickoff.cmp(&b.kickoff).then_with(|| a.id.cmp(&b.id)));
    upcoming.truncate(UPCOMING_LIMIT);
    upcoming
}

fn status_line(app: &App, m: &Match, is_live: bool, theme: &Theme) -> Line<'static> {
    if is_live {
        let clock = live_clock(&m.status);
        Line::from(vec![
            Span::styled(
                format!("{} {clock}", app.icons().live()),
                Style::new().fg(theme.warn).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ·  {}", context_tag(m)),
                Style::new().fg(theme.dim),
            ),
        ])
    } else {
        let day = timefmt::date_heading(m.kickoff, &app.config().ui.timezone, app.local_offset());
        let countdown = countdown(m.kickoff);
        Line::from(vec![
            Span::styled(day, Style::new().fg(theme.fg)),
            Span::styled(format!("  ·  {countdown}"), Style::new().fg(theme.accent)),
        ])
    }
}

fn names_line(app: &App, m: &Match, theme: &Theme) -> Line<'static> {
    let home_fav = app.config().is_favorite(&m.home.name, &m.home.abbreviation);
    let away_fav = app.config().is_favorite(&m.away.name, &m.away.abbreviation);
    let name_style = |fav: bool| {
        if fav {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
        }
    };
    Line::from(vec![
        Span::styled(m.home.name.clone(), name_style(home_fav)),
        Span::styled("   v   ", Style::new().fg(theme.dim)),
        Span::styled(m.away.name.clone(), name_style(away_fav)),
    ])
}

fn event_line(app: &App, m: &Match, theme: &Theme) -> Line<'static> {
    let detail = app.live_focus().state().value();
    let recent = detail
        .filter(|d| d.summary.id == m.id)
        .and_then(|d| d.events.last());
    recent.map_or_else(
        || {
            Line::from(Span::styled(
                "Following the action…",
                Style::new().fg(theme.dim),
            ))
        },
        |event| {
            Line::from(vec![
                Span::styled(
                    format!(
                        "{} {}",
                        event_icon(event.kind, app.icons()),
                        minute_text(event)
                    ),
                    Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", event_text(event)),
                    Style::new().fg(theme.fg),
                ),
            ])
        },
    )
}

fn context_tag(m: &Match) -> String {
    match (m.stage, &m.group) {
        (Stage::GroupStage, Some(group)) => format!("Group {group}"),
        (stage, _) => stage.label().to_owned(),
    }
}

fn countdown(kickoff: OffsetDateTime) -> String {
    let delta = kickoff - OffsetDateTime::now_utc();
    let mins = delta.whole_minutes();
    if mins <= 0 {
        return "kicking off".to_owned();
    }
    let (hours, minutes) = (mins / 60, mins % 60);
    if hours >= 24 {
        format!("in {}d {}h", hours / 24, hours % 24)
    } else if hours > 0 {
        format!("in {hours}h {minutes}m")
    } else {
        format!("in {minutes}m")
    }
}

fn live_clock(status: &MatchStatus) -> String {
    match status {
        MatchStatus::Live { minute, detail } => minute.map_or_else(
            || detail.clone().unwrap_or_else(|| "LIVE".to_owned()),
            |m| format!("{m}'"),
        ),
        MatchStatus::HalfTime => "HALF-TIME".to_owned(),
        _ => "LIVE".to_owned(),
    }
}

fn score_text(score: Option<Score>) -> String {
    score.map_or_else(|| "0-0".to_owned(), |s| format!("{}-{}", s.home, s.away))
}

fn minute_text(event: &MatchEvent) -> String {
    match (event.minute, event.stoppage) {
        (Some(m), Some(s)) if s > 0 => format!("{m}+{s}'"),
        (Some(m), _) => format!("{m}'"),
        (None, _) => String::new(),
    }
}

fn event_text(event: &MatchEvent) -> String {
    let who = event.player.clone().unwrap_or_default();
    match &event.detail {
        Some(detail) if !who.is_empty() => format!("{who} ({detail})"),
        Some(detail) => detail.clone(),
        None => who,
    }
}

fn event_icon(kind: MatchEventKind, icons: Icons) -> &'static str {
    match kind {
        MatchEventKind::Goal | MatchEventKind::OwnGoal | MatchEventKind::PenaltyGoal => "\u{26bd}",
        MatchEventKind::PenaltyMiss => "\u{00d7}",
        MatchEventKind::YellowCard | MatchEventKind::SecondYellow => "YC",
        MatchEventKind::RedCard => "RC",
        MatchEventKind::Substitution => "\u{2194}",
        MatchEventKind::Var => "VAR",
        MatchEventKind::Other => icons.bullet(),
    }
}

fn match_label(m: &Match) -> String {
    format!("{} vs {}", team_label(&m.home), team_label(&m.away))
}

fn team_label(team: &Team) -> String {
    if team.abbreviation.is_empty() {
        team.name.clone()
    } else {
        team.abbreviation.clone()
    }
}

// --- big block digits ------------------------------------------------------

fn big_glyphs(text: &str, color: Color) -> Vec<Line<'static>> {
    let style = Style::new().fg(color).add_modifier(Modifier::BOLD);
    (0..5)
        .map(|row| {
            let mut s = String::new();
            for (i, ch) in text.chars().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(glyph(ch)[row]);
            }
            Line::from(Span::styled(s, style))
        })
        .collect()
}

fn glyph(c: char) -> [&'static str; 5] {
    match c {
        '0' => ["████", "█  █", "█  █", "█  █", "████"],
        '1' => ["  █ ", " ██ ", "  █ ", "  █ ", " ███"],
        '2' => ["████", "   █", "████", "█   ", "████"],
        '3' => ["████", "   █", " ███", "   █", "████"],
        '4' => ["█  █", "█  █", "████", "   █", "   █"],
        '5' => ["████", "█   ", "████", "   █", "████"],
        '6' => ["████", "█   ", "████", "█  █", "████"],
        '7' => ["████", "   █", "  █ ", " █  ", " █  "],
        '8' => ["████", "█  █", "████", "█  █", "████"],
        '9' => ["████", "█  █", "████", "   █", "████"],
        '-' => ["    ", "    ", "████", "    ", "    "],
        ':' => ["    ", " ██ ", "    ", " ██ ", "    "],
        _ => ["    ", "    ", "    ", "    ", "    "],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team(name: &str, abbreviation: &str) -> Team {
        Team {
            id: abbreviation.to_lowercase(),
            name: name.to_owned(),
            abbreviation: abbreviation.to_owned(),
            country_code: None,
            crest_url: None,
        }
    }

    fn fixture(status: MatchStatus, ts: i64) -> Match {
        Match {
            id: format!("m{ts}"),
            stage: Stage::GroupStage,
            group: Some("A".to_owned()),
            home: team("Canada", "CAN"),
            away: team("Mexico", "MEX"),
            score: Some(Score {
                home: 1,
                away: 0,
                home_pens: None,
                away_pens: None,
            }),
            status,
            kickoff: OffsetDateTime::from_unix_timestamp(ts).unwrap_or(OffsetDateTime::UNIX_EPOCH),
            venue: None,
        }
    }

    #[test]
    fn live_filter_keeps_live_and_halftime_only() {
        let matches = vec![
            fixture(MatchStatus::Scheduled, 1),
            fixture(MatchStatus::HalfTime, 2),
            fixture(
                MatchStatus::Live {
                    minute: Some(12),
                    detail: None,
                },
                3,
            ),
        ];
        assert_eq!(live_matches(&matches).len(), 2);
    }

    #[test]
    fn upcoming_filter_is_sorted_and_capped() {
        let mut matches = vec![
            fixture(MatchStatus::Scheduled, 300),
            fixture(MatchStatus::Scheduled, 100),
        ];
        for ts in 0..40 {
            matches.push(fixture(MatchStatus::Scheduled, 1000 + i64::from(ts)));
        }
        let upcoming = upcoming_matches(&matches);
        assert_eq!(upcoming.len(), UPCOMING_LIMIT);
        assert_eq!(upcoming[0].id, "m100");
    }

    #[test]
    fn big_glyphs_have_five_rows() {
        assert_eq!(big_glyphs("2-1", Color::Rgb(1, 2, 3)).len(), 5);
    }

    #[test]
    fn live_clock_shows_minute() {
        let clock = live_clock(&MatchStatus::Live {
            minute: Some(42),
            detail: None,
        });
        assert_eq!(clock, "42'");
    }
}
