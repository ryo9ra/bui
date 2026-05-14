use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn draw(f: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let text = "\
j / ↓        move down
k / ↑        move up
g / G        jump to top / bottom
Tab/BackTab  switch tab
R            refresh
?            toggle this help
q / Esc      close help / quit

v0.1 skeleton — only navigation and refresh are wired up.
See docs/spec.md for the planned key map.
";
    f.render_widget(Paragraph::new(text).block(block), popup);
}

fn centered_rect(pw: u16, ph: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - ph) / 2),
            Constraint::Percentage(ph),
            Constraint::Percentage((100 - ph) / 2),
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
