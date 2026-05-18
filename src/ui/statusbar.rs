use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let line = if app.search_active {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(app.filter.clone()),
            Span::styled("_", Style::default().fg(Color::Cyan)),
        ])
    } else if let Some(pending) = &app.pending_task {
        let frame = SPINNER[app.spinner_frame % SPINNER.len()];
        let spinner_color = app.config.theme.spinner;
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                frame.to_string(),
                Style::default()
                    .fg(spinner_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(pending.desc.clone(), Style::default().fg(spinner_color)),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(app.status.clone(), Style::default().fg(Color::DarkGray)),
        ])
    };
    f.render_widget(Paragraph::new(line), area);
}
