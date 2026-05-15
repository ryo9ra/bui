use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        Tab::Local => draw_local(f, app, area),
        Tab::Remote => draw_remote(f, app, area),
        Tab::Worktree => draw_placeholder(f, area, "Worktrees — coming in v0.3"),
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

            let mut name_spans: Vec<Span<'static>> = vec![Span::raw(b.name.clone())];
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
                b.full_name.clone(),
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

fn draw_placeholder(f: &mut Frame, area: Rect, msg: &str) {
    let p = Paragraph::new(msg)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(p, area);
}
