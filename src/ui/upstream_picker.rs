use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::UpstreamPickerState;

pub fn draw(f: &mut Frame, area: Rect, picker: &UpstreamPickerState) {
    let popup = centered_rect(64, 14, area);
    f.render_widget(Clear, popup);

    let title = format!(" Set upstream for '{}' ", picker.branch);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let label = Style::default().fg(Color::DarkGray);
    let visible = picker.visible_candidates();
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Filter row.
    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled("/", Style::default().fg(Color::Cyan)),
        Span::raw(picker.filter.clone()),
        Span::styled("_", Style::default().fg(Color::Cyan)),
    ]));
    lines.push(Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(Color::DarkGray),
    )));

    // Candidate rows.
    if visible.is_empty() {
        lines.push(Line::from(Span::styled(
            " (no matches)",
            label.add_modifier(Modifier::DIM),
        )));
    } else {
        // Reserve room so the popup never overflows. The popup body is
        // 14 rows minus 2 borders, 2 filter rows, 2 footer rows = ~8 rows
        // for candidates.
        for (i, candidate) in visible.iter().enumerate().take(8) {
            let mut style = Style::default();
            let prefix = if i == picker.selected {
                style = style
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD);
                " ▶ "
            } else {
                "   "
            };
            lines.push(Line::from(Span::styled(
                format!("{prefix}{candidate}"),
                style,
            )));
        }
        if visible.len() > 8 {
            lines.push(Line::from(Span::styled(
                format!("   … {} more", visible.len() - 8),
                label.add_modifier(Modifier::DIM),
            )));
        }
    }

    // Footer hint.
    lines.push(Line::from(Span::styled(
        "─".repeat(60),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        " ↑/↓ select  ·  type to filter  ·  Enter confirm  ·  Esc cancel",
        label,
    )));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn centered_rect(pw: u16, height: u16, area: Rect) -> Rect {
    let pad_v = area.height.saturating_sub(height) / 2;
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(pad_v),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);
    let h = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pw) / 2),
            Constraint::Percentage(pw),
            Constraint::Percentage((100 - pw) / 2),
        ])
        .split(v[1]);
    h[1]
}
