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
Enter        checkout selected branch
c            create new branch from HEAD
r            rename selected branch
d            delete selected branch (confirm)
D            force-delete selected branch (confirm)
/            incremental search (filter by name)
Tab/BackTab  switch tab
R            refresh
?            toggle this help
q            quit
Esc          close modal / cancel input / clear filter

In search mode: type to filter (case-insensitive substring),
↑/↓ navigate, Enter confirm (keep filter), Esc clear & exit.

In confirm dialog: y/Y accept, n/N or Esc cancel.

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
