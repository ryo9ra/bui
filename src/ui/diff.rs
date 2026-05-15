use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, Tab};
use crate::git::Commit;

const VISIBLE_COMMITS: usize = 8;

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
            Line::from(Span::styled("  v: toggle back to detail", dim.add_modifier(Modifier::DIM))),
        ];
        f.render_widget(Paragraph::new(body).block(block).wrap(Wrap { trim: false }), area);
        return;
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Diff:   ", dim),
            Span::styled(
                diff.target.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("  vs  ", dim),
            Span::styled(diff.base.clone(), Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
    ];

    push_section(
        &mut lines,
        &diff.ahead,
        format!("  Ahead ({}):", diff.ahead.len()),
        Color::Cyan,
    );
    lines.push(Line::from(""));
    push_section(
        &mut lines,
        &diff.behind,
        format!("  Behind ({}):", diff.behind.len()),
        Color::Magenta,
    );

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  v: back to detail",
        dim.add_modifier(Modifier::DIM),
    )));

    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn push_section(lines: &mut Vec<Line<'static>>, commits: &[Commit], heading: String, color: Color) {
    lines.push(Line::from(Span::styled(
        heading,
        Style::default().fg(color),
    )));
    if commits.is_empty() {
        lines.push(Line::from(Span::styled(
            "    (none)",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        )));
        return;
    }
    for c in commits.iter().take(VISIBLE_COMMITS) {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(c.short_sha.clone(), Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::raw(c.subject.clone()),
        ]));
    }
    if commits.len() > VISIBLE_COMMITS {
        lines.push(Line::from(Span::styled(
            format!("    … {} more", commits.len() - VISIBLE_COMMITS),
            Style::default().fg(Color::DarkGray),
        )));
    }
}
