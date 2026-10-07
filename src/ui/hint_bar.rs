use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

use crate::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let hint = match app.active_tab {
        Tab::Local => {
            " ?:help · ↵:checkout · c:create · C:from · r:rename · d:del · u:upstream · W:wt · /:search · v:diff · s:sort · F:filter · f/p/P:fetch/pull/push · X:clean-gone"
        }
        Tab::Remote => {
            " ?:help · ↵:track+switch · C:create-from · d:push --delete · /:search · v:diff · f:fetch"
        }
        Tab::Worktree => " ?:help · ↵:cd-hint · d:remove · (Local) W:add",
    };
    let p = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
    f.render_widget(p, area);
}
