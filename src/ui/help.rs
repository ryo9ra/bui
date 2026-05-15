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
Enter        (Local) checkout selected branch
c            (Local) create new branch from HEAD
C            create new branch from selected ref (Local or Remote)
r            (Local) rename selected branch
d            (Local) delete selected branch (confirm)
D            (Local) force-delete selected branch (confirm)
u            (Local) set upstream — pick a remote (e.g. origin)
/            incremental search (filter by name)
s            toggle sort: recency ↔ name
f            fetch all remotes (async)
p            pull current branch (async)
P            push current branch (async)
Tab/BackTab  switch tab (Local · Remote · Worktree)
R            refresh local + remote
?            toggle this help
q            quit
Esc          close modal / cancel input / clear filter

Remote tab: navigation and search work; fetch / pull / push and
tracking-from-remote are coming.

In search mode: type to filter (case-insensitive substring),
↑/↓ navigate, Enter confirm (keep filter), Esc clear & exit.

In confirm dialog: ←/→ or Tab to select Yes/No, Enter to confirm,
y/Y direct accept, n/N or Esc cancel. Default focus is No.

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
