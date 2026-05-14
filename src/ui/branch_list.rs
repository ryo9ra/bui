use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

use crate::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        Tab::Local => draw_local(f, app, area),
        Tab::Remote => draw_placeholder(f, area, "Remote branches — coming in v0.2"),
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
    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let marker = if b.is_current { "*" } else { " " };
            let mut style = if b.is_current {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            if i == app.selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Row::new(vec![
                marker.to_string(),
                b.name.clone(),
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
