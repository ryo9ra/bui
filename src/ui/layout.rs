use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct LayoutSpec {
    pub tabs: bool,
    pub main: MainSpec,
    pub statusbar: bool,
}

pub enum MainSpec {
    BranchList,
    /// Left = branch list, right = pane chosen at draw time via
    /// `App::right_pane`.
    Split,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RightPane {
    Detail,
    Diff,
}

pub struct LayoutRects {
    pub tabs: Rect,
    pub main: Rect,
    pub main_split: Option<(Rect, Rect)>,
    pub statusbar: Rect,
}

impl LayoutSpec {
    pub fn default_layout() -> Self {
        Self {
            tabs: true,
            main: MainSpec::Split,
            statusbar: true,
        }
    }

    pub fn compute(&self, area: Rect) -> LayoutRects {
        let tabs_h = if self.tabs { 1 } else { 0 };
        let status_h = if self.statusbar { 1 } else { 0 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(tabs_h),
                Constraint::Min(1),
                Constraint::Length(status_h),
            ])
            .split(area);
        let main = chunks[1];
        let main_split = match self.main {
            MainSpec::Split => {
                let parts = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(30), Constraint::Length(40)])
                    .split(main);
                Some((parts[0], parts[1]))
            }
            MainSpec::BranchList => None,
        };
        LayoutRects {
            tabs: chunks[0],
            main,
            main_split,
            statusbar: chunks[2],
        }
    }
}
