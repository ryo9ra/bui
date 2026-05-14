use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::InputState;

pub fn draw(f: &mut Frame, area: Rect, input: &InputState) {
    let popup = centered_rect(60, 3, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(format!(" {} ", input.prompt))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let body = format!(" {}_", input.value);
    f.render_widget(Paragraph::new(body).block(block), popup);
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
