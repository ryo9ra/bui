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

/// Shorten `s` to at most `max` characters by collapsing the middle with
/// `…`. Used for branch names and worktree paths that overflow narrow
/// table columns. `max < 3` returns the unchanged string (no useful
/// truncation possible).
pub(crate) fn middle_truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max || max < 3 {
        return s.to_string();
    }
    // Reserve one slot for the ellipsis. Lean toward the head when odd —
    // names usually read left-to-right (e.g. `feature/oauth-...`).
    let keep = max - 1;
    let left = keep.div_ceil(2);
    let right = keep - left;
    let head: String = chars.iter().take(left).collect();
    let tail: String = chars[chars.len() - right..].iter().collect();
    format!("{head}…{tail}")
}

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

#[cfg(test)]
mod truncate_tests {
    use super::middle_truncate;

    #[test]
    fn short_string_returns_unchanged() {
        assert_eq!(middle_truncate("main", 10), "main");
    }

    #[test]
    fn long_string_gets_ellipsis_in_middle() {
        let s = "feature/oauth-login-overhaul-pr-1234";
        let t = middle_truncate(s, 16);
        assert_eq!(t.chars().count(), 16);
        assert!(t.contains('…'));
        assert!(t.starts_with("feature"));
        assert!(t.ends_with("1234"));
    }

    #[test]
    fn handles_too_small_max_gracefully() {
        // max=2 leaves no room for head+tail+ellipsis. Return original.
        assert_eq!(middle_truncate("abcdef", 2), "abcdef");
    }

    #[test]
    fn boundary_at_exact_max_no_change() {
        assert_eq!(middle_truncate("exactly10c", 10), "exactly10c");
    }

    #[test]
    fn favours_head_when_odd_budget() {
        // budget 5 = 1 ellipsis + 2 left + 2 right (would be 4 keep) but
        // div_ceil pushes left to 2 even on odd. With keep=4: left=2,
        // right=2. Easy case to verify.
        let s = "abcdefgh";
        let t = middle_truncate(s, 5);
        assert_eq!(t.chars().count(), 5);
        assert_eq!(t, "ab…gh");
    }
}
