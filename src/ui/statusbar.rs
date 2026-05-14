use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let line = if app.search_active {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(app.filter.clone()),
            Span::styled("_", Style::default().fg(Color::Cyan)),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(app.status.clone(), Style::default().fg(Color::DarkGray)),
        ])
    };
    f.render_widget(Paragraph::new(line), area);
}
