//! Live scoreboard screen — a glanceable "Live Activity" card.
//!
//! Shows one match at a time, big enough to read across a room: a large
//! block-digit score flanked by real national flags (rendered as inline images
//! when the terminal supports graphics, omitted otherwise unless
//! `WORLDCUP26_GRAPHICS=halfblocks` forces a text-cell fallback), the clock, and
//! the most recent event (goal/card). `j`/`k` cycles through the in-play matches
//! (or the soonest upcoming fixtures when nothing is live, with a countdown),
//! `Enter` opens the full match detail, and `f` toggles the flags.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui_image::Image;
use time::OffsetDateTime;
use wc_data::domain::{Match, MatchEvent, MatchEventKind, MatchStatus, Score, Stage, Team};

use crate::app::App;
use crate::data::Remote;
use crate::timefmt;
use crate::ui::flag_image;
use crate::ui::icons::Icons;
use crate::ui::screens::widgets;
use crate::ui::theme::Theme;

/// How many upcoming fixtures to cycle through when nothing is live.
const UPCOMING_LIMIT: usize = 24;
/// Rows occupied by the big-score glyphs.
const SCORE_ROWS: u16 = 5;
/// Horizontal gap (cells) between a flag and the score.
const FLAG_GAP: u16 = 3;
/// Rows reserved for the most-recent-event / context line so long commentary can
/// wrap instead of running off the right edge.
const CONTEXT_ROWS: u16 = 3;
/// Non-body rows in the card: status, names, context, pager and their gaps.
/// (status + gap + names + gap + [`CONTEXT_ROWS`] + gap + pager.)
const CHROME_ROWS: u16 = 6 + CONTEXT_ROWS;
/// Bounds (in rows) for the dynamically-sized Live flags.
const MIN_FLAG_ROWS: u16 = 6;
const MAX_FLAG_ROWS: u16 = 16;

/// Largest flag size `(cols, rows)` that fits the card: as tall as the vertical
/// space allows (bounded by [`MIN_FLAG_ROWS`]/[`MAX_FLAG_ROWS`]), then as wide as
/// the ~4:3 art implies, shrinking if two flags plus the score would overflow the
/// width. flag-icons art is 4:3, which at a typical 1:2 cell aspect is ~8:3 in
/// cells; `Resize::Fit` letterboxes, so being a cell or two off never distorts it.
fn flag_dims(inner: Rect, score_w: u16) -> (u16, u16) {
    let mut rows = inner
        .height
        .saturating_sub(CHROME_ROWS + 1)
        .clamp(MIN_FLAG_ROWS, MAX_FLAG_ROWS);
    loop {
        let cols = (rows * 8).div_ceil(3);
        let block_w = cols * 2 + FLAG_GAP * 2 + score_w;
        if block_w <= inner.width || rows <= MIN_FLAG_ROWS {
            return (cols, rows);
        }
        rows -= 1;
    }
}

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
    let (flag_cols, flag_rows) = flag_dims(inner, score_w);
    let block_w = if want_flags {
        flag_cols * 2 + FLAG_GAP * 2 + score_w
    } else {
        score_w
    };
    let flags = want_flags && block_w <= inner.width;

    // Vertically centred block: status, gap, names, body (flags/score), gap,
    // context, gap, pager.
    let body_h = if flags {
        flag_rows.max(SCORE_ROWS)
    } else {
        SCORE_ROWS
    };
    let total_h = body_h + CHROME_ROWS;
    let top = inner.y + inner.height.saturating_sub(total_h) / 2;
    let names_y = top + 2;
    let body_y = top + 3;
    let context_y = body_y + body_h + 1;
    let pager_y = context_y + CONTEXT_ROWS + 1;

    let full = |y: u16| Rect::new(inner.x, y, inner.width, 1);
    centered(frame, full(top), status_line(app, m, is_live, theme));

    if flags {
        let block_x = (inner.x + inner.width / 2).saturating_sub(block_w / 2);

        // A single centred "Home v Away" line. The flags+score are drawn as one
        // image whose on-screen width depends on the terminal's real font size
        // (which we can't know under tmux), so name labels pinned beside each
        // flag would drift out of alignment; a centred line over the whole card
        // stays correct regardless.
        centered(frame, full(names_y), names_line(app, m, theme));
        // Render the whole card body (home flag · score · away flag) as a single
        // image. Drawing two flags and the text score separately desyncs under
        // WezTerm + tmux, leaving only the first flag visible; one image is safe.
        render_card(
            app,
            frame,
            m,
            &score,
            score_color,
            CardDims {
                flag_cols,
                flag_rows,
                width_cols: block_w,
                height_rows: body_h,
            },
            Rect::new(block_x, body_y, block_w, body_h),
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
    frame.render_widget(
        Paragraph::new(context)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        Rect::new(inner.x, context_y, inner.width, CONTEXT_ROWS),
    );

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
    app.flags().is_some() && flag_image::has_flag(code)
}

/// Cell dimensions describing a composited Live-card body.
#[derive(Clone, Copy)]
struct CardDims {
    flag_cols: u16,
    flag_rows: u16,
    width_cols: u16,
    height_rows: u16,
}

/// Draw the composited Live card (home flag · score · away flag) as one image.
fn render_card(
    app: &App,
    frame: &mut Frame,
    m: &Match,
    score_str: &str,
    score_color: Color,
    dims: CardDims,
    rect: Rect,
) {
    let Some(flag_store) = app.flags() else {
        return;
    };
    let (mask, cols) = score_mask(score_str);
    let card = flag_image::FlagCard {
        home: &m.home.abbreviation,
        away: &m.away.abbreviation,
        flag_cols: dims.flag_cols,
        flag_rows: dims.flag_rows,
        gap_cols: FLAG_GAP,
        width_cols: dims.width_cols,
        height_rows: dims.height_rows,
        score: flag_image::ScoreBlocks {
            mask,
            cols,
            rgba: color_rgba(score_color),
        },
    };
    let mut flag_store = flag_store.borrow_mut();
    if let Some(protocol) = flag_store.card(&card) {
        frame.render_widget(Image::new(protocol), rect);
    }
}

/// Build a filled-cell mask (row-major, `cols`×5) for the big-glyph `score`.
fn score_mask(score: &str) -> (Vec<bool>, u16) {
    let cols = big_width(score);
    let width = usize::from(cols);
    let mut mask = vec![false; width * 5];
    let mut x = 0usize;
    for (i, ch) in score.chars().enumerate() {
        if i > 0 {
            x += 1; // inter-glyph spacing column
        }
        let rows = glyph(ch);
        for (row, cells) in rows.iter().enumerate() {
            for (col, cell) in cells.chars().enumerate() {
                if cell != ' ' && x + col < width {
                    mask[row * width + x + col] = true;
                }
            }
        }
        x += 4;
    }
    (mask, cols)
}

/// Convert a theme colour to straight RGBA, defaulting to white for non-RGB.
fn color_rgba(color: Color) -> [u8; 4] {
    match color {
        Color::Rgb(r, g, b) => [r, g, b, 255],
        _ => [255, 255, 255, 255],
    }
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

/// The most recent event by match time that carries descriptive text.
///
/// Providers don't agree on ordering (ESPN lists key events newest-first), so we
/// select by minute rather than list position. Events whose [`event_text`] is
/// empty (a bare minute with no commentary) are skipped so the card always shows
/// something meaningful about what is happening.
fn latest_event(events: &[MatchEvent]) -> Option<&MatchEvent> {
    events
        .iter()
        .filter(|event| has_event_text(event))
        .max_by_key(|event| (event.minute.unwrap_or(0), event.stoppage.unwrap_or(0)))
}

/// Whether an event carries commentary text (a player name or detail), without
/// allocating the formatted string. Mirrors `event_text` being non-empty.
fn has_event_text(event: &MatchEvent) -> bool {
    event.player.as_deref().is_some_and(|p| !p.is_empty())
        || event.detail.as_deref().is_some_and(|d| !d.is_empty())
}

fn event_line(app: &App, m: &Match, theme: &Theme) -> Line<'static> {
    let detail = app.live_focus().state().value();
    let recent = detail
        .filter(|d| d.summary.id == m.id)
        .and_then(|d| latest_event(&d.events));
    recent.map_or_else(
        || {
            Line::from(Span::styled(
                "Following the action…",
                Style::new().fg(theme.dim),
            ))
        },
        |event| {
            let minute = minute_text(event);
            let prefix = if minute.is_empty() {
                format!("{} ", event_icon(event.kind, app.icons()))
            } else {
                format!("{} {minute}", event_icon(event.kind, app.icons()))
            };
            Line::from(vec![
                Span::styled(
                    prefix,
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
            location: None,
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
    fn flag_dims_caps_on_a_tall_wide_card() {
        let (cols, rows) = flag_dims(Rect::new(0, 0, 220, 60), 14);
        assert_eq!(rows, MAX_FLAG_ROWS);
        assert!(cols > rows, "flags read wider than tall (~8:3)");
    }

    #[test]
    fn flag_dims_clamps_to_min_on_a_short_card() {
        let (_cols, rows) = flag_dims(Rect::new(0, 0, 220, 9), 14);
        assert_eq!(rows, MIN_FLAG_ROWS);
    }

    #[test]
    fn flag_dims_shrinks_to_fit_a_narrow_card() {
        let score_w = 14;
        let (cols, rows) = flag_dims(Rect::new(0, 0, 70, 60), score_w);
        assert!(rows < MAX_FLAG_ROWS);
        assert!(cols * 2 + FLAG_GAP * 2 + score_w <= 70);
    }

    fn event_at(minute: u16, stoppage: Option<u16>) -> MatchEvent {
        MatchEvent {
            minute: Some(minute),
            stoppage,
            kind: MatchEventKind::Goal,
            team_id: None,
            player: Some("Scorer".to_owned()),
            detail: Some("Goal".to_owned()),
        }
    }

    #[test]
    fn latest_event_picks_highest_minute_regardless_of_order() {
        // Newest-first, like ESPN returns.
        let events = vec![
            event_at(73, None),
            event_at(45, Some(2)),
            event_at(10, None),
        ];
        assert_eq!(latest_event(&events).and_then(|e| e.minute), Some(73));
    }

    #[test]
    fn latest_event_uses_stoppage_as_tiebreak() {
        let events = vec![event_at(90, None), event_at(90, Some(4))];
        assert_eq!(
            latest_event(&events).map(|e| (e.minute, e.stoppage)),
            Some((Some(90), Some(4)))
        );
    }

    #[test]
    fn latest_event_is_none_for_empty() {
        assert!(latest_event(&[]).is_none());
    }

    #[test]
    fn latest_event_skips_textless_events() {
        let bare = MatchEvent {
            minute: Some(80),
            stoppage: None,
            kind: MatchEventKind::Other,
            team_id: None,
            player: None,
            detail: None,
        };
        let events = vec![event_at(20, None), bare];
        // The bare minute-only event is ignored in favour of one with text.
        assert_eq!(latest_event(&events).and_then(|e| e.minute), Some(20));
    }

    #[test]
    fn score_mask_marks_filled_cells() {
        let (mask, cols) = score_mask("1-0");
        assert_eq!(cols, big_width("1-0"));
        assert_eq!(mask.len(), usize::from(cols) * 5);
        assert!(mask.iter().any(|&on| on));
    }

    #[test]
    fn color_rgba_passes_through_rgb() {
        assert_eq!(color_rgba(Color::Rgb(245, 203, 110)), [245, 203, 110, 255]);
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
