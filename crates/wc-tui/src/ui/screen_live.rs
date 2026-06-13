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
/// Flag image size in cells (≈3:2 at a typical 1:2 cell aspect).
const FLAG_COLS: u16 = 15;
const FLAG_ROWS: u16 = 5;
/// Lines of text in a card (status, gap, names, 5 score rows, gap, event, gap,
/// footer). Used to centre the card vertically and to place the flag images.
const CARD_LINES: u16 = 12;
/// Row offset of the big-score block within the card.
const SCORE_ROW: u16 = 3;

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
    let lines = card_lines(app, m, selected, cards.len(), is_live, &score, theme);
    let pad = usize::from(inner.height).saturating_sub(lines.len()) / 2;
    let mut all = vec![Line::from(""); pad];
    all.extend(lines);
    frame.render_widget(
        Paragraph::new(all)
            .alignment(Alignment::Center)
            .style(Style::new().fg(theme.fg)),
        inner,
    );

    render_flags(app, frame, inner, m, &score, pad);
}

/// The text shown big and centred: the live score, or the kickoff time.
fn score_centre(app: &App, m: &Match, is_live: bool) -> String {
    if is_live {
        score_text(m.score)
    } else {
        timefmt::time_hm(m.kickoff, &app.config().ui.timezone, app.local_offset())
    }
}

/// Overlay the two teams' flag images either side of the big-score block.
fn render_flags(app: &App, frame: &mut Frame, inner: Rect, m: &Match, score: &str, pad: usize) {
    if !app.config().ui.show_flags {
        return;
    }
    let Some(flag_store) = app.flags() else {
        return;
    };
    if inner.height < CARD_LINES || pad > usize::from(u16::MAX) {
        return;
    }
    let score_w = big_width(score);
    let gap = 2u16;
    let block_w = u32::from(FLAG_COLS) * 2 + u32::from(gap) * 2 + u32::from(score_w);
    if block_w > u32::from(inner.width) {
        return;
    }
    let pad = u16::try_from(pad).unwrap_or(u16::MAX);
    let centre_x = inner.x + inner.width / 2;
    let score_left = centre_x.saturating_sub(score_w / 2);
    let flag_y = inner.y + pad + SCORE_ROW;
    if flag_y + FLAG_ROWS > inner.y + inner.height {
        return;
    }
    let left_x = score_left.saturating_sub(gap + FLAG_COLS);
    let right_x = score_left + score_w + gap;

    let mut flag_store = flag_store.borrow_mut();
    for (code, x) in [
        (&m.home.abbreviation, left_x),
        (&m.away.abbreviation, right_x),
    ] {
        if let Some(protocol) = flag_store.flag(code, FLAG_COLS, FLAG_ROWS) {
            let rect = Rect::new(x, flag_y, FLAG_COLS, FLAG_ROWS);
            frame.render_widget(Image::new(protocol), rect);
        }
    }
}

/// Width in cells of a big-glyph string (each glyph is 4 wide, 1-cell spaced).
fn big_width(text: &str) -> u16 {
    let n = text.chars().count() as u16;
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

fn card_lines(
    app: &App,
    m: &Match,
    index: usize,
    total: usize,
    is_live: bool,
    score: &str,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Status line: clock + stage (live) or kickoff countdown (upcoming).
    lines.push(status_line(app, m, is_live, theme));
    lines.push(Line::from(""));

    // Team names.
    lines.push(names_line(app, m, theme));

    // Big score (or kickoff time). Flags are overlaid separately as images.
    let big = big_glyphs(score, if is_live { theme.warn } else { theme.accent });
    lines.extend(big);

    // Most recent event for the in-play match.
    lines.push(Line::from(""));
    if is_live {
        lines.push(event_line(app, m, theme));
    } else {
        lines.push(Line::from(Span::styled(
            context_tag(m),
            Style::new().fg(theme.dim),
        )));
    }

    // Footer: position + hints.
    lines.push(Line::from(""));
    let label = if is_live { "live" } else { "upcoming" };
    lines.push(Line::from(Span::styled(
        format!("‹ {} / {total} {label} ›", index + 1),
        Style::new().fg(theme.dim),
    )));
    lines
}

fn status_line(app: &App, m: &Match, is_live: bool, theme: &Theme) -> Line<'static> {
    if is_live {
        let clock = live_clock(&m.status);
        Line::from(vec![
            Span::styled(
                format!("{} {clock}", app.icons().live()),
                Style::new().fg(theme.warn).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("   {}", context_tag(m)), Style::new().fg(theme.dim)),
        ])
    } else {
        let day = timefmt::date_heading(m.kickoff, &app.config().ui.timezone, app.local_offset());
        let countdown = countdown(m.kickoff);
        Line::from(vec![
            Span::styled(day, Style::new().fg(theme.fg)),
            Span::styled(format!("   {countdown}"), Style::new().fg(theme.accent)),
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
