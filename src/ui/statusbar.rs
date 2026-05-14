use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled(app.status.clone(), Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}
