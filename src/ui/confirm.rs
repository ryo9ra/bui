use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{ConfirmChoice, ConfirmState};

pub fn draw(f: &mut Frame, area: Rect, confirm: &ConfirmState) {
    // Multi-line prompts (e.g. clean-gone's branch list) grow the popup:
    // borders + blank/buttons/blank/hint rows + one per prompt line.
    let prompt_lines: Vec<&str> = confirm.prompt.lines().collect();
    let height = (prompt_lines.len() as u16 + 7).min(area.height);
    let popup = centered_rect(64, height, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let yes_style = button_style(Color::Green, confirm.focus == ConfirmChoice::Yes);
    let no_style = button_style(Color::Red, confirm.focus == ConfirmChoice::No);

    let mut body = vec![Line::from("")];
    body.extend(
        prompt_lines
            .iter()
            .map(|l| Line::from(Span::raw(l.to_string())).alignment(Alignment::Center)),
    );
    body.extend([
        Line::from(""),
        Line::from(vec![
            Span::styled("  Yes  ", yes_style),
            Span::raw("     "),
            Span::styled("  No  ", no_style),
        ])
        .alignment(Alignment::Center),
        Line::from(""),
        Line::from(Span::styled(
            "←/→ select  ·  Enter confirm  ·  y/n shortcut  ·  Esc cancel",
            Style::default().fg(Color::DarkGray),
        ))
        .alignment(Alignment::Center),
    ]);
    f.render_widget(Paragraph::new(body).block(block), popup);
}

fn button_style(color: Color, focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Black)
            .bg(color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(color)
            .add_modifier(Modifier::BOLD | Modifier::DIM)
    }
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
