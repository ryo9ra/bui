use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, Tab};
use crate::git::DiffLine;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));

    let dim = Style::default().fg(Color::DarkGray);

    let Some(diff) = &app.branch_diff else {
        let msg = match app.active_tab {
            Tab::Local => {
                "  (no diff: selected branch is current, or computation\n   failed — try switching off current first)"
            }
            Tab::Remote | Tab::Worktree => {
                "  Diff is computed for Local-tab branches\n   against the current branch."
            }
        };
        let body = vec![
            Line::from(""),
            Line::from(Span::styled(msg.to_string(), dim)),
            Line::from(""),
            Line::from(Span::styled(
                "  v: toggle back to detail",
                dim.add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(
            Paragraph::new(body).block(block).wrap(Wrap { trim: false }),
            area,
        );
        return;
    };

    // Split: 3-row fixed header on top, patch on the rest.
    let inner = block.inner(area);
    f.render_widget(block, area);
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    // Header.
    let header = vec![
        Line::from(vec![
            Span::styled("  Diff: ", dim),
            Span::styled(
                diff.target.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("  vs  ", dim),
            Span::styled(diff.base.clone(), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  ", dim),
            Span::styled(
                format!("↑{} ", diff.ahead.len()),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("↓{}", diff.behind.len()),
                Style::default().fg(Color::Magenta),
            ),
            Span::styled(
                format!("    Ctrl-D/U scroll · {} patch lines", diff.patch.len()),
                dim.add_modifier(Modifier::DIM),
            ),
        ]),
        Line::from(Span::styled(
            "─".repeat(80),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(header), split[0]);

    // Patch.
    if diff.patch.is_empty() {
        let body = vec![Line::from(Span::styled(
            "  (no file-level changes)",
            dim.add_modifier(Modifier::DIM),
        ))];
        f.render_widget(Paragraph::new(body), split[1]);
        return;
    }

    let lines: Vec<Line<'static>> = diff.patch.iter().map(render_diff_line).collect();
    let patch = Paragraph::new(lines).scroll((app.diff_scroll, 0));
    f.render_widget(patch, split[1]);
}

fn render_diff_line(line: &DiffLine) -> Line<'static> {
    match line {
        DiffLine::FileHeader(s) => Line::from(Span::styled(
            s.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        DiffLine::Hunk(s) => Line::from(Span::styled(s.clone(), Style::default().fg(Color::Cyan))),
        DiffLine::Add(s) => Line::from(Span::styled(s.clone(), Style::default().fg(Color::Green))),
        DiffLine::Remove(s) => Line::from(Span::styled(s.clone(), Style::default().fg(Color::Red))),
        DiffLine::Context(s) => Line::from(Span::raw(s.clone())),
        DiffLine::Meta(s) => Line::from(Span::styled(
            s.clone(),
            Style::default().fg(Color::DarkGray),
        )),
    }
}
