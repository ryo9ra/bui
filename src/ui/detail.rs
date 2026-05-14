use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, Tab};
use crate::git::{Branch, RemoteBranch};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));

    match app.active_tab {
        Tab::Local => {
            if let Some(branch) = app.selected_branch() {
                draw_branch_detail(f, area, block, branch);
            } else {
                f.render_widget(Paragraph::new("").block(block), area);
            }
        }
        Tab::Remote => {
            if let Some(branch) = app.selected_remote_branch() {
                draw_remote_detail(f, area, block, branch);
            } else {
                f.render_widget(Paragraph::new("").block(block), area);
            }
        }
        Tab::Worktree => draw_placeholder(f, area, block, "Worktree details — v0.3"),
    }
}

fn draw_branch_detail(f: &mut Frame, area: Rect, block: Block<'_>, b: &Branch) {
    let label = Style::default().fg(Color::DarkGray);
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Branch:  ", label),
            Span::styled(
                b.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  SHA:     ", label),
            Span::styled(b.short_sha.clone(), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("  Date:    ", label),
            Span::raw(b.rel_date.clone()),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Subject:", label)),
        Line::from(vec![Span::raw("    "), Span::raw(b.subject.clone())]),
        Line::from(""),
        Line::from(Span::styled(
            "  (full body / author / upstream coming later)",
            label.add_modifier(Modifier::DIM),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_remote_detail(f: &mut Frame, area: Rect, block: Block<'_>, b: &RemoteBranch) {
    let label = Style::default().fg(Color::DarkGray);
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Remote:  ", label),
            Span::styled(b.remote.clone(), Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::styled("  Branch:  ", label),
            Span::styled(
                b.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Full:    ", label),
            Span::styled(b.full_name.clone(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  SHA:     ", label),
            Span::styled(b.short_sha.clone(), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("  Date:    ", label),
            Span::raw(b.rel_date.clone()),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Subject:", label)),
        Line::from(vec![Span::raw("    "), Span::raw(b.subject.clone())]),
        Line::from(""),
        Line::from(Span::styled(
            "  (track-as-local / push / pull — coming)",
            label.add_modifier(Modifier::DIM),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_placeholder(f: &mut Frame, area: Rect, block: Block<'_>, msg: &str) {
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(body).block(block), area);
}
