use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_tab;
    let dim = Style::default().fg(Color::DarkGray);
    let line = Line::from(vec![
        tab_span("Local", Tab::Local, active),
        Span::raw(" "),
        tab_span("Remote", Tab::Remote, active),
        Span::raw(" "),
        tab_span("Worktree", Tab::Worktree, active),
        Span::raw("   "),
        Span::styled(format!("sort: {}", app.sort_mode.label()), dim),
        Span::raw("   "),
        Span::styled("bui v0.1", dim),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn tab_span(label: &str, this: Tab, active: Tab) -> Span<'static> {
    let style = if this == active {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Span::styled(format!(" {} ", label), style)
}
