use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::app::{App, FlashKind, Tab};
use crate::ui::middle_truncate;

/// Width used by every column other than the long-name column (marker +
/// sha + date + ratatui's default column spacing). Keep in sync with the
/// `widths` arrays below.
const FIXED_COLUMNS_WIDTH: u16 = 1 + 8 + 14 + 3;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        Tab::Local => draw_local(f, app, area),
        Tab::Remote => draw_remote(f, app, area),
        Tab::Worktree => draw_worktree(f, app, area),
    }
}

fn draw_local(f: &mut Frame, app: &App, area: Rect) {
    if app.local_branches.is_empty() {
        draw_placeholder(f, area, "No local branches found.");
        return;
    }
    let visible = app.visible_branches();
    if visible.is_empty() {
        draw_placeholder(f, area, "No matches.");
        return;
    }
    let theme = &app.config.theme;
    let name_budget = area
        .width
        .saturating_sub(FIXED_COLUMNS_WIDTH)
        .saturating_sub(2) as usize; // small slack for tag suffixes
    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let marker = if b.is_current { "*" } else { " " };
            let mut style = if b.is_current {
                Style::default()
                    .fg(theme.current_branch)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            if i == app.selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if let Some(flash) = &app.flash
                && matches!(&flash.kind, FlashKind::LocalBranch(n) if n == &b.name)
            {
                style = Style::default()
                    .bg(Color::Green)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
            }

            let mut name_spans: Vec<Span<'static>> =
                vec![Span::raw(middle_truncate(&b.name, name_budget))];
            if let Some(track) = &b.upstream_track {
                if track.gone {
                    name_spans.push(Span::styled("  (gone)", Style::default().fg(Color::Yellow)));
                } else if track.ahead > 0 || track.behind > 0 {
                    let s = match (track.ahead, track.behind) {
                        (a, 0) => format!("  ↑{a}"),
                        (0, b) => format!("  ↓{b}"),
                        (a, b) => format!("  ↑{a}↓{b}"),
                    };
                    name_spans.push(Span::styled(s, Style::default().fg(Color::Cyan)));
                }
            }
            if b.is_merged && !b.is_current {
                name_spans.push(Span::styled(
                    "  merged",
                    Style::default()
                        .fg(theme.merged_tag)
                        .add_modifier(Modifier::DIM),
                ));
            }
            if b.worktree_path.is_some() {
                name_spans.push(Span::styled(
                    "  worktree",
                    Style::default().fg(theme.worktree_tag),
                ));
            }

            Row::new(vec![
                Cell::from(marker.to_string()),
                Cell::from(Line::from(name_spans)),
                Cell::from(b.short_sha.clone()),
                Cell::from(b.rel_date.clone()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(1),
        Constraint::Min(20),
        Constraint::Length(8),
        Constraint::Length(14),
    ];
    let table = Table::new(rows, widths).block(Block::default().borders(Borders::NONE));
    f.render_widget(table, area);
}

fn draw_remote(f: &mut Frame, app: &App, area: Rect) {
    if app.remote_branches.is_empty() {
        draw_placeholder(f, area, "No remote branches. Run `git fetch` (f) first.");
        return;
    }
    let visible = app.visible_remote_branches();
    if visible.is_empty() {
        draw_placeholder(f, area, "No matches.");
        return;
    }
    let theme = &app.config.theme;
    let name_budget = area.width.saturating_sub(FIXED_COLUMNS_WIDTH) as usize;
    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut style = Style::default().fg(theme.remote_branch);
            if i == app.selected_remote {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Row::new(vec![
                " ".to_string(),
                middle_truncate(&b.full_name, name_budget),
                b.short_sha.clone(),
                b.rel_date.clone(),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(1),
        Constraint::Min(20),
        Constraint::Length(8),
        Constraint::Length(14),
    ];
    let table = Table::new(rows, widths).block(Block::default().borders(Borders::NONE));
    f.render_widget(table, area);
}

fn draw_worktree(f: &mut Frame, app: &App, area: Rect) {
    if app.worktrees.is_empty() {
        draw_placeholder(f, area, "No worktrees.");
        return;
    }
    let theme = &app.config.theme;
    // Layout: marker(1) + branch(min 15) + sha(8) + spacing(3). Path
    // gets the remainder; middle-truncate it so the front (often `~/`)
    // and tail (the leaf, the most identifying part) stay visible.
    let path_budget = area.width.saturating_sub(1 + 15 + 8 + 3) as usize;
    let rows: Vec<Row> = app
        .worktrees
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let marker = if w.is_current { "*" } else { " " };
            let mut style = if w.is_current {
                Style::default()
                    .fg(theme.current_branch)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            if i == app.selected_worktree {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if let Some(flash) = &app.flash
                && matches!(&flash.kind, FlashKind::Worktree(p) if p == &w.path)
            {
                style = Style::default()
                    .bg(Color::Green)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
            }
            let branch = w.branch.clone().unwrap_or_else(|| "(detached)".to_string());
            let head_short: String = w.head.chars().take(8).collect();
            Row::new(vec![
                Cell::from(marker.to_string()),
                Cell::from(branch),
                Cell::from(head_short),
                Cell::from(middle_truncate(&w.path, path_budget)),
            ])
            .style(style)
        })
        .collect();
    let widths = [
        Constraint::Length(1),
        Constraint::Min(15),
        Constraint::Length(8),
        Constraint::Min(20),
    ];
    let table = Table::new(rows, widths).block(Block::default().borders(Borders::NONE));
    f.render_widget(table, area);
}

fn draw_placeholder(f: &mut Frame, area: Rect, msg: &str) {
    let p = Paragraph::new(msg)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(p, area);
}
