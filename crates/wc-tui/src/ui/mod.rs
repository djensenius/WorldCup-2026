//! Top-level rendering: tab bar, body, status bar, toast overlay, and help.

pub mod flags;
pub mod icons;
pub mod screen_bracket;
pub mod screen_detail;
pub mod screen_live;
pub mod screen_matches;
pub mod screen_standings;
pub mod screen_team;
pub mod screens;
pub mod theme;
pub mod toast;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs};

use crate::app::App;
use crate::ui::screens::Screen;
use crate::ui::toast::{Level, Toasts};

/// Render the entire UI for one frame.
pub fn render(app: &App, frame: &mut Frame) {
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());
    let (tabs_area, body_area, status_area) = (areas[0], areas[1], areas[2]);

    render_tabs(app, frame, tabs_area);
    screens::render(app, frame, body_area);
    render_status_bar(app, frame, status_area);
    render_toasts(app, frame, body_area);
    if app.show_help() {
        render_help(app, frame, body_area);
    }
}

fn render_tabs(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let icons = app.icons();
    let titles: Vec<String> = Screen::all()
        .into_iter()
        .enumerate()
        .map(|(index, screen)| format!("{} {}{}", index + 1, icons.tab(screen), screen.short()))
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.dim))
        .title(format!(" {}wc26 ", icons.brand()))
        .title_style(Style::new().fg(theme.accent).add_modifier(Modifier::BOLD));
    record_tab_hitboxes(app, block.inner(area), &titles);
    let tabs = Tabs::new(titles)
        .select(app.screen().index())
        .style(Style::new().fg(theme.dim))
        .highlight_style(Style::new().fg(theme.accent).add_modifier(Modifier::BOLD))
        .block(block);
    frame.render_widget(tabs, area);
}

/// Replicate `Tabs`' layout (a one-column pad on each side of every label and a
/// single-column divider between labels) to record each tab's clickable
/// x-range, so a mouse click on the bar can be mapped back to a screen.
fn record_tab_hitboxes(app: &App, inner: Rect, titles: &[String]) {
    let mut ranges = Vec::with_capacity(titles.len());
    let right = inner.right();
    let mut x = inner.left();
    for (index, title) in titles.iter().enumerate() {
        if x >= right {
            break;
        }
        let start = x;
        x = x.saturating_add(1); // left padding
        let width = u16::try_from(Line::from(title.as_str()).width()).unwrap_or(0);
        x = x.saturating_add(width).min(right);
        x = x.saturating_add(1).min(right); // right padding
        ranges.push((start, x));
        if index + 1 < titles.len() {
            x = x.saturating_add(1).min(right); // divider
        }
    }
    app.set_tab_hitboxes(inner.top(), ranges);
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let mut spans = vec![
        Span::styled(
            format!(" {} ", app.provider_name()),
            Style::new().bg(theme.bg).fg(theme.fg),
        ),
        Span::raw("  "),
    ];
    if let Some(stage) = app.current_stage_label() {
        spans.push(Span::styled(stage, Style::new().fg(theme.accent)));
        spans.push(Span::raw("  "));
    }
    if app.is_refreshing() {
        spans.push(Span::styled("⟳ refreshing", Style::new().fg(theme.warn)));
        spans.push(Span::raw("  "));
    } else if let Some(age) = app.active_data_age() {
        spans.push(Span::styled(
            format!("updated {}s ago", age.as_secs()),
            Style::new().fg(theme.dim),
        ));
        spans.push(Span::raw("  "));
    }
    if app.showing_cached() {
        spans.push(Span::styled("● cached", Style::new().fg(theme.dim)));
        spans.push(Span::raw("  "));
    }
    let hint = "q quit · ? help · Tab switch · r refresh · t theme";
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let pad = usize::from(area.width).saturating_sub(used + hint.chars().count() + 1);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(hint, Style::new().fg(theme.dim)));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_toasts(app: &App, frame: &mut Frame, area: Rect) {
    let toasts: &Toasts = app.toasts();
    if toasts.is_empty() {
        return;
    }
    let theme = app.theme();
    let lines: Vec<Line> = toasts
        .iter()
        .map(|t| {
            let color = match t.level {
                Level::Info => theme.accent,
                Level::Warn => theme.warn,
                Level::Error => theme.error,
            };
            Line::from(Span::styled(
                format!(" {} ", t.text),
                Style::new().fg(color),
            ))
        })
        .collect();
    let height = u16::try_from(lines.len()).unwrap_or(1) + 2;
    let width = area.width.saturating_sub(2).min(60);
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width + 1),
        y: area.y + area.height.saturating_sub(height + 1),
        width,
        height,
    };
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.dim))
        .title(" notices ");
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

fn render_help(app: &App, frame: &mut Frame, area: Rect) {
    let theme = app.theme();
    let head = |text: &'static str| {
        Line::from(Span::styled(
            text,
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ))
    };
    let row = |keys: &'static str, action: &'static str| {
        Line::from(vec![
            Span::styled(format!("  {keys:<18}"), Style::new().fg(theme.fg)),
            Span::styled(action, Style::new().fg(theme.dim)),
        ])
    };
    let lines = vec![
        head("Global"),
        row("1–4 / Tab / ⇧Tab", "switch screen"),
        row("r", "refresh now"),
        row("t", "cycle colour theme"),
        row("? ", "toggle this help"),
        row("q", "quit"),
        Line::from(""),
        head("Matches & Live"),
        row("j / k / ↑ / ↓", "move / switch match"),
        row("f", "favourites filter (Matches) · flags (Live)"),
        row("Enter", "open match detail"),
        Line::from(""),
        head("Standings"),
        row("h / l / ← / →", "switch group"),
        row("j / k / ↑ / ↓", "move between teams"),
        row("Enter", "open team"),
        row("*", "toggle favourite team"),
        Line::from(""),
        head("Overlays"),
        row("Enter", "team → match detail"),
        row("j / k", "scroll / move"),
        row("Esc", "back / close"),
    ];
    let width = 48u16.min(area.width.saturating_sub(2));
    let height = u16::try_from(lines.len()).unwrap_or(1) + 2;
    let rect = screens::widgets::centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.accent))
        .title(" Help ");
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left)
            .style(Style::new().fg(theme.fg)),
        rect,
    );
}
