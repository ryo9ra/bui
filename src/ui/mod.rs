pub mod branch_list;
pub mod confirm;
pub mod detail;
pub mod diff;
pub mod help;
pub mod input;
pub mod layout;
pub mod statusbar;
pub mod tabs;
pub mod upstream_picker;

use ratatui::Frame;

use crate::app::{App, Modal};
use crate::ui::layout::{MainSpec, RightPane};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let rects = app.layout.compute(area);

    if app.layout.tabs {
        tabs::draw(f, app, rects.tabs);
    }
    match &app.layout.main {
        MainSpec::BranchList => branch_list::draw(f, app, rects.main),
        MainSpec::Split => {
            if let Some((left, right_area)) = rects.main_split {
                branch_list::draw(f, app, left);
                match app.right_pane {
                    RightPane::Detail => detail::draw(f, app, right_area),
                    RightPane::Diff => diff::draw(f, app, right_area),
                }
            } else {
                branch_list::draw(f, app, rects.main);
            }
        }
    }
    if app.layout.statusbar {
        statusbar::draw(f, app, rects.statusbar);
    }
    if let Some(input_state) = &app.input {
        input::draw(f, area, input_state);
    }
    if let Some(picker) = &app.upstream_picker {
        upstream_picker::draw(f, area, picker);
    }
    if let Some(confirm_state) = &app.confirm {
        confirm::draw(f, area, confirm_state);
    }
    if let Some(modal) = &app.modal {
        match modal {
            Modal::Help => help::draw(f, area),
        }
    }
}
