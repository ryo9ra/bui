use std::io;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::event::{Event, EventChannel, Outcome, TaskId};
use crate::git::{Branch, RemoteBranch, Repo};
use crate::task::Action;
use crate::ui;
use crate::ui::layout::LayoutSpec;

pub struct App {
    pub repo: Arc<dyn Repo>,
    pub task_tx: Sender<(TaskId, Action)>,
    pub next_task_id: TaskId,
    pub pending_task: Option<PendingTask>,
    pub spinner_frame: usize,
    pub local_branches: Vec<Branch>,
    pub remote_branches: Vec<RemoteBranch>,
    /// Index into `visible_branches()` (Local tab).
    pub selected: usize,
    /// Index into `visible_remote_branches()` (Remote tab).
    pub selected_remote: usize,
    pub filter: String,
    pub search_active: bool,
    pub status: String,
    pub active_tab: Tab,
    pub modal: Option<Modal>,
    pub input: Option<InputState>,
    pub confirm: Option<ConfirmState>,
    pub layout: LayoutSpec,
    pub should_quit: bool,
    pub dirty: bool,
}

pub struct PendingTask {
    pub id: TaskId,
    pub desc: String,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Tab {
    Local,
    Remote,
    Worktree,
}

pub enum Modal {
    Help,
}

pub struct InputState {
    pub prompt: String,
    pub value: String,
    pub mode: InputMode,
}

pub enum InputMode {
    CreateBranch,
    RenameBranch { old: String },
}

impl InputState {
    pub fn create_branch() -> Self {
        Self {
            prompt: "Create branch".to_string(),
            value: String::new(),
            mode: InputMode::CreateBranch,
        }
    }

    pub fn rename_branch(old: String) -> Self {
        Self {
            prompt: format!("Rename '{old}' to"),
            value: old.clone(),
            mode: InputMode::RenameBranch { old },
        }
    }
}

pub struct ConfirmState {
    pub prompt: String,
    pub action: ConfirmAction,
    pub focus: ConfirmChoice,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ConfirmChoice {
    Yes,
    No,
}

pub enum ConfirmAction {
    DeleteBranch { name: String, force: bool },
}

impl App {
    pub fn new(repo: Arc<dyn Repo>, task_tx: Sender<(TaskId, Action)>) -> Self {
        Self {
            repo,
            task_tx,
            next_task_id: 1,
            pending_task: None,
            spinner_frame: 0,
            local_branches: Vec::new(),
            remote_branches: Vec::new(),
            selected: 0,
            selected_remote: 0,
            filter: String::new(),
            search_active: false,
            status: "ready".to_string(),
            active_tab: Tab::Local,
            modal: None,
            input: None,
            confirm: None,
            layout: LayoutSpec::default_layout(),
            should_quit: false,
            dirty: true,
        }
    }

    fn dispatch(&mut self, action: Action, desc: impl Into<String>) {
        let id = self.next_task_id;
        self.next_task_id += 1;
        let desc = desc.into();
        if self.task_tx.send((id, action)).is_err() {
            self.status = "worker channel closed".to_string();
            return;
        }
        self.status = format!("{desc}...");
        self.pending_task = Some(PendingTask { id, desc });
    }

    pub fn on_task_result(&mut self, id: TaskId, result: Result<Outcome, String>) {
        // Ignore stray results from a no-longer-pending task.
        if !matches!(&self.pending_task, Some(p) if p.id == id) {
            return;
        }
        self.pending_task = None;
        match result {
            Ok(Outcome::Fetched) => {
                self.refresh(None);
                self.status = "fetched".to_string();
            }
            Ok(Outcome::Pulled) => {
                self.refresh(None);
                self.status = "pulled".to_string();
            }
            Ok(Outcome::Pushed) => {
                self.refresh(None);
                self.status = "pushed".to_string();
            }
            Err(e) => self.status = format!("error: {e}"),
        }
        self.dirty = true;
    }

    pub fn visible_branches(&self) -> Vec<&Branch> {
        if self.filter.is_empty() {
            return self.local_branches.iter().collect();
        }
        let needle = self.filter.to_lowercase();
        self.local_branches
            .iter()
            .filter(|b| b.name.to_lowercase().contains(&needle))
            .collect()
    }

    pub fn visible_remote_branches(&self) -> Vec<&RemoteBranch> {
        if self.filter.is_empty() {
            return self.remote_branches.iter().collect();
        }
        let needle = self.filter.to_lowercase();
        self.remote_branches
            .iter()
            .filter(|b| b.full_name.to_lowercase().contains(&needle))
            .collect()
    }

    pub fn selected_branch(&self) -> Option<&Branch> {
        self.visible_branches().get(self.selected).copied()
    }

    pub fn selected_remote_branch(&self) -> Option<&RemoteBranch> {
        self.visible_remote_branches()
            .get(self.selected_remote)
            .copied()
    }

    fn selected_name(&self) -> Option<String> {
        self.selected_branch().map(|b| b.name.clone())
    }

    fn selected_is_current(&self) -> bool {
        self.visible_branches()
            .get(self.selected)
            .is_some_and(|b| b.is_current)
    }

    fn current_visible_count(&self) -> usize {
        match self.active_tab {
            Tab::Local => self.visible_branches().len(),
            Tab::Remote => self.visible_remote_branches().len(),
            Tab::Worktree => 0,
        }
    }

    fn current_selected(&self) -> usize {
        match self.active_tab {
            Tab::Local => self.selected,
            Tab::Remote => self.selected_remote,
            Tab::Worktree => 0,
        }
    }

    fn set_current_selected(&mut self, i: usize) {
        match self.active_tab {
            Tab::Local => self.selected = i,
            Tab::Remote => self.selected_remote = i,
            Tab::Worktree => {}
        }
    }

    pub fn refresh(&mut self, prefer: Option<&str>) {
        match self.repo.list_local_branches() {
            Ok(bs) => {
                self.status = format!("{} branches", bs.len());
                self.local_branches = bs;
                self.fix_selection(prefer);
            }
            Err(e) => self.status = format!("error: {e}"),
        }
        // Remote refresh is best-effort: a missing or unreadable refs/remotes
        // shouldn't clobber the local view's status. Errors leave the previous
        // remote list intact.
        if let Ok(bs) = self.repo.list_remote_branches() {
            self.remote_branches = bs;
            if self.selected_remote >= self.remote_branches.len() {
                self.selected_remote = self
                    .visible_remote_branches()
                    .len()
                    .saturating_sub(1);
            }
        }
        self.dirty = true;
    }

    fn refresh_keeping_cursor(&mut self) {
        let prefer = self.selected_name();
        self.refresh(prefer.as_deref());
    }

    fn fix_selection(&mut self, prefer: Option<&str>) {
        let visible = self.visible_branches();
        if let Some(name) = prefer
            && let Some(i) = visible.iter().position(|b| b.name == name)
        {
            self.selected = i;
            return;
        }
        if self.selected >= visible.len() {
            self.selected = visible.len().saturating_sub(1);
        }
    }

    fn checkout_selected(&mut self) {
        let Some(name) = self.selected_name() else {
            return;
        };
        if self.selected_is_current() {
            self.status = format!("already on {name}");
            return;
        }
        match self.repo.checkout(&name) {
            Ok(()) => {
                self.refresh(Some(&name));
                self.status = format!("switched to {name}");
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }
        if self.input.is_some() {
            self.handle_input_key(key);
            return;
        }
        if self.confirm.is_some() {
            self.handle_confirm_key(key);
            return;
        }
        if self.search_active {
            self.handle_search_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('g') | KeyCode::Home => self.set_current_selected(0),
            KeyCode::Char('G') | KeyCode::End => {
                let count = self.current_visible_count();
                self.set_current_selected(count.saturating_sub(1));
            }
            KeyCode::Tab => self.cycle_tab(true),
            KeyCode::BackTab => self.cycle_tab(false),
            KeyCode::Char('R') => self.refresh_keeping_cursor(),
            KeyCode::Enter if self.active_tab == Tab::Local => self.checkout_selected(),
            KeyCode::Char('c') if self.active_tab == Tab::Local => {
                self.input = Some(InputState::create_branch());
            }
            KeyCode::Char('r') if self.active_tab == Tab::Local => {
                if let Some(old) = self.selected_name() {
                    self.input = Some(InputState::rename_branch(old));
                }
            }
            KeyCode::Char('/') if self.active_tab != Tab::Worktree => self.start_search(),
            KeyCode::Esc if !self.filter.is_empty() => self.clear_filter(),
            KeyCode::Char('d') if self.active_tab == Tab::Local => self.request_delete(false),
            KeyCode::Char('D') if self.active_tab == Tab::Local => self.request_delete(true),
            KeyCode::Char('f') if self.pending_task.is_none() => {
                self.dispatch(Action::Fetch { remote: None }, "fetching");
            }
            KeyCode::Char('p') if self.pending_task.is_none() => {
                self.dispatch(Action::Pull, "pulling");
            }
            KeyCode::Char('P') if self.pending_task.is_none() => {
                self.dispatch(Action::Push, "pushing");
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn request_delete(&mut self, force: bool) {
        let Some(name) = self.selected_name() else {
            return;
        };
        if self.selected_is_current() {
            self.status = "cannot delete the current branch".to_string();
            return;
        }
        let prompt = if force {
            format!("Force-delete '{name}'? (unmerged work will be lost)")
        } else {
            format!("Delete '{name}'?")
        };
        self.confirm = Some(ConfirmState {
            prompt,
            action: ConfirmAction::DeleteBranch { name, force },
            focus: ConfirmChoice::No,
        });
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        let Some(state) = self.confirm.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.accept_confirm(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.cancel_confirm(),
            KeyCode::Left | KeyCode::Char('h') => state.focus = ConfirmChoice::Yes,
            KeyCode::Right | KeyCode::Char('l') => state.focus = ConfirmChoice::No,
            KeyCode::Tab | KeyCode::BackTab => {
                state.focus = match state.focus {
                    ConfirmChoice::Yes => ConfirmChoice::No,
                    ConfirmChoice::No => ConfirmChoice::Yes,
                };
            }
            KeyCode::Enter => match state.focus {
                ConfirmChoice::Yes => self.accept_confirm(),
                ConfirmChoice::No => self.cancel_confirm(),
            },
            _ => {}
        }
        self.dirty = true;
    }

    fn accept_confirm(&mut self) {
        if let Some(state) = self.confirm.take() {
            self.execute_confirm(state.action);
        }
    }

    fn cancel_confirm(&mut self) {
        self.confirm = None;
        self.status = "cancelled".to_string();
    }

    fn execute_confirm(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::DeleteBranch { name, force } => self.do_delete_branch(&name, force),
        }
    }

    fn do_delete_branch(&mut self, name: &str, force: bool) {
        match self.repo.delete_branch(name, force) {
            Ok(()) => {
                self.refresh(None);
                self.status = format!("deleted {name}");
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    fn start_search(&mut self) {
        self.filter.clear();
        self.search_active = true;
        self.set_current_selected(0);
        self.status = "search".to_string();
    }

    fn clear_filter(&mut self) {
        self.filter.clear();
        self.set_current_selected(0);
        self.status = "filter cleared".to_string();
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_active = false;
                self.filter.clear();
                self.set_current_selected(0);
                self.status = "search cancelled".to_string();
            }
            KeyCode::Enter => {
                self.search_active = false;
                let (shown, total) = match self.active_tab {
                    Tab::Local => (self.visible_branches().len(), self.local_branches.len()),
                    Tab::Remote => (
                        self.visible_remote_branches().len(),
                        self.remote_branches.len(),
                    ),
                    Tab::Worktree => (0, 0),
                };
                self.status = if self.filter.is_empty() {
                    format!("{total} branches")
                } else {
                    format!("{shown}/{total} matching '{}'", self.filter)
                };
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.set_current_selected(0);
            }
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.set_current_selected(0);
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn handle_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => self.modal = None,
            _ => {}
        }
        self.dirty = true;
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        let Some(input) = self.input.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.input = None;
                self.status = "cancelled".to_string();
            }
            KeyCode::Enter => self.submit_input(),
            KeyCode::Backspace => {
                input.value.pop();
            }
            KeyCode::Char(c) => input.value.push(c),
            _ => {}
        }
        self.dirty = true;
    }

    fn submit_input(&mut self) {
        let Some(input) = self.input.take() else {
            return;
        };
        let value = input.value.trim().to_string();
        if value.is_empty() {
            self.status = "input cancelled (empty)".to_string();
            return;
        }
        match input.mode {
            InputMode::CreateBranch => self.do_create_branch(&value),
            InputMode::RenameBranch { old } => self.do_rename_branch(&old, &value),
        }
    }

    fn do_create_branch(&mut self, name: &str) {
        match self.repo.create_branch(name, None) {
            Ok(()) => {
                self.refresh(Some(name));
                self.status = format!("created {name}");
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    fn do_rename_branch(&mut self, old: &str, new: &str) {
        if old == new {
            self.status = "rename skipped (no change)".to_string();
            return;
        }
        match self.repo.rename_branch(old, new) {
            Ok(()) => {
                self.refresh(Some(new));
                self.status = format!("renamed {old} -> {new}");
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    pub fn on_tick(&mut self) {
        if self.pending_task.is_some() {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
            self.dirty = true;
        }
    }

    fn move_down(&mut self) {
        let cur = self.current_selected();
        if cur + 1 < self.current_visible_count() {
            self.set_current_selected(cur + 1);
        }
    }

    fn move_up(&mut self) {
        let cur = self.current_selected();
        if cur > 0 {
            self.set_current_selected(cur - 1);
        }
    }

    fn cycle_tab(&mut self, forward: bool) {
        self.active_tab = if forward {
            match self.active_tab {
                Tab::Local => Tab::Remote,
                Tab::Remote => Tab::Worktree,
                Tab::Worktree => Tab::Local,
            }
        } else {
            match self.active_tab {
                Tab::Local => Tab::Worktree,
                Tab::Remote => Tab::Local,
                Tab::Worktree => Tab::Remote,
            }
        };
    }
}

pub fn run_loop(
    app: &mut App,
    events: &EventChannel,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    loop {
        if app.dirty {
            terminal.draw(|f| ui::draw(f, app))?;
            app.dirty = false;
        }
        match events.recv()? {
            Event::Input(key) => app.on_key(key),
            Event::Tick => app.on_tick(),
            Event::TaskResult(id, result) => app.on_task_result(id, result),
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    struct NoopRepo;
    impl Repo for NoopRepo {
        fn list_local_branches(&self) -> anyhow::Result<Vec<Branch>> {
            Ok(vec![])
        }
        fn list_remote_branches(&self) -> anyhow::Result<Vec<RemoteBranch>> {
            Ok(vec![])
        }
        fn checkout(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn create_branch(&self, _: &str, _: Option<&str>) -> anyhow::Result<()> {
            Ok(())
        }
        fn delete_branch(&self, _: &str, _: bool) -> anyhow::Result<()> {
            Ok(())
        }
        fn rename_branch(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn fetch(&self, _: Option<&str>) -> anyhow::Result<()> {
            Ok(())
        }
        fn pull(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn push(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn br(name: &str, current: bool) -> Branch {
        Branch {
            name: name.to_string(),
            is_current: current,
            short_sha: "abc1234".to_string(),
            subject: "subject".to_string(),
            rel_date: "1 day ago".to_string(),
        }
    }

    fn app_with(branches: Vec<Branch>) -> App {
        let (tx, rx) = std::sync::mpsc::channel();
        // Leak the receiver so `dispatch()` calls don't fail with a closed
        // channel; tests that inspect dispatched actions build their own
        // channel directly.
        std::mem::forget(rx);
        let mut a = App::new(Arc::new(NoopRepo), tx);
        a.local_branches = branches;
        a
    }

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn visible_branches_empty_filter_returns_all() {
        let app = app_with(vec![br("main", true), br("foo", false)]);
        assert_eq!(app.visible_branches().len(), 2);
    }

    #[test]
    fn visible_branches_substring_match_is_case_insensitive() {
        let mut app = app_with(vec![
            br("main", true),
            br("feature/foo", false),
            br("FEATURE/Bar", false),
        ]);
        app.filter = "feature".to_string();
        let names: Vec<_> = app
            .visible_branches()
            .iter()
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(names, ["feature/foo", "FEATURE/Bar"]);
    }

    #[test]
    fn visible_branches_no_match_returns_empty() {
        let mut app = app_with(vec![br("main", true)]);
        app.filter = "xyz".to_string();
        assert!(app.visible_branches().is_empty());
    }

    #[test]
    fn fix_selection_clamps_when_out_of_range() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 5;
        app.fix_selection(None);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn fix_selection_finds_preferred_branch() {
        let mut app = app_with(vec![br("main", true), br("foo", false), br("bar", false)]);
        app.selected = 0;
        app.fix_selection(Some("bar"));
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn fix_selection_falls_back_when_preferred_missing() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 5;
        app.fix_selection(Some("nonexistent"));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn fix_selection_uses_visible_count_when_filter_active() {
        let mut app = app_with(vec![br("main", true), br("foo", false), br("foobar", false)]);
        app.filter = "foo".to_string();
        app.selected = 5;
        app.fix_selection(None);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn cycle_tab_forward_wraps_around() {
        let mut app = app_with(vec![]);
        app.cycle_tab(true);
        assert_eq!(app.active_tab, Tab::Remote);
        app.cycle_tab(true);
        assert_eq!(app.active_tab, Tab::Worktree);
        app.cycle_tab(true);
        assert_eq!(app.active_tab, Tab::Local);
    }

    #[test]
    fn cycle_tab_backward_wraps_around() {
        let mut app = app_with(vec![]);
        app.cycle_tab(false);
        assert_eq!(app.active_tab, Tab::Worktree);
        app.cycle_tab(false);
        assert_eq!(app.active_tab, Tab::Remote);
        app.cycle_tab(false);
        assert_eq!(app.active_tab, Tab::Local);
    }

    #[test]
    fn start_search_resets_filter_and_cursor() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.filter = "stale".to_string();
        app.selected = 1;
        app.start_search();
        assert!(app.search_active);
        assert!(app.filter.is_empty());
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn clear_filter_resets_filter() {
        let mut app = app_with(vec![br("main", true)]);
        app.filter = "x".to_string();
        app.clear_filter();
        assert!(app.filter.is_empty());
    }

    #[test]
    fn handle_search_key_appends_chars_and_resets_cursor() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.search_active = true;
        app.selected = 1;
        app.handle_search_key(k(KeyCode::Char('f')));
        app.handle_search_key(k(KeyCode::Char('o')));
        assert_eq!(app.filter, "fo");
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn handle_search_key_enter_keeps_filter_and_exits_mode() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.search_active = true;
        app.filter = "foo".to_string();
        app.handle_search_key(k(KeyCode::Enter));
        assert!(!app.search_active);
        assert_eq!(app.filter, "foo");
    }

    #[test]
    fn handle_search_key_esc_clears_filter_and_exits_mode() {
        let mut app = app_with(vec![br("main", true)]);
        app.search_active = true;
        app.filter = "x".to_string();
        app.handle_search_key(k(KeyCode::Esc));
        assert!(!app.search_active);
        assert!(app.filter.is_empty());
    }

    #[test]
    fn handle_search_key_backspace_pops_char() {
        let mut app = app_with(vec![br("main", true)]);
        app.search_active = true;
        app.filter = "foo".to_string();
        app.handle_search_key(k(KeyCode::Backspace));
        assert_eq!(app.filter, "fo");
    }

    #[test]
    fn request_delete_blocks_current_branch() {
        let mut app = app_with(vec![br("main", true)]);
        app.selected = 0;
        app.request_delete(false);
        assert!(app.confirm.is_none());
        assert!(app.status.contains("cannot delete"));
    }

    #[test]
    fn request_delete_opens_confirm_focused_on_no() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(false);
        let state = app.confirm.as_ref().expect("confirm should be set");
        assert_eq!(state.focus, ConfirmChoice::No);
        match &state.action {
            ConfirmAction::DeleteBranch { name, force } => {
                assert_eq!(name, "foo");
                assert!(!force);
            }
        }
    }

    #[test]
    fn request_delete_force_flag_is_propagated() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(true);
        let state = app.confirm.as_ref().unwrap();
        match &state.action {
            ConfirmAction::DeleteBranch { force, .. } => assert!(force),
        }
    }

    #[test]
    fn handle_confirm_key_arrow_keys_move_focus() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(false);
        app.handle_confirm_key(k(KeyCode::Left));
        assert_eq!(app.confirm.as_ref().unwrap().focus, ConfirmChoice::Yes);
        app.handle_confirm_key(k(KeyCode::Right));
        assert_eq!(app.confirm.as_ref().unwrap().focus, ConfirmChoice::No);
    }

    #[test]
    fn handle_confirm_key_tab_toggles_focus() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(false);
        app.handle_confirm_key(k(KeyCode::Tab));
        assert_eq!(app.confirm.as_ref().unwrap().focus, ConfirmChoice::Yes);
        app.handle_confirm_key(k(KeyCode::Tab));
        assert_eq!(app.confirm.as_ref().unwrap().focus, ConfirmChoice::No);
    }

    #[test]
    fn handle_confirm_key_enter_with_no_focus_cancels() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(false);
        app.handle_confirm_key(k(KeyCode::Enter));
        assert!(app.confirm.is_none());
        assert_eq!(app.status, "cancelled");
    }

    #[test]
    fn handle_confirm_key_enter_with_yes_focus_executes() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(false);
        app.handle_confirm_key(k(KeyCode::Left));
        app.handle_confirm_key(k(KeyCode::Enter));
        assert!(app.confirm.is_none());
        assert!(app.status.starts_with("deleted "));
    }

    #[test]
    fn handle_confirm_key_y_shortcut_accepts_immediately() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(false);
        app.handle_confirm_key(k(KeyCode::Char('y')));
        assert!(app.confirm.is_none());
        assert!(app.status.starts_with("deleted "));
    }

    #[test]
    fn handle_confirm_key_n_shortcut_cancels() {
        let mut app = app_with(vec![br("main", true), br("foo", false)]);
        app.selected = 1;
        app.request_delete(false);
        app.handle_confirm_key(k(KeyCode::Char('n')));
        assert!(app.confirm.is_none());
        assert_eq!(app.status, "cancelled");
    }

    #[test]
    fn pressing_f_dispatches_fetch_with_monotonic_id() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = App::new(Arc::new(NoopRepo), tx);
        app.local_branches = vec![br("main", true)];
        app.on_key(k(KeyCode::Char('f')));

        let pending = app.pending_task.as_ref().expect("pending should be set");
        assert_eq!(pending.id, 1);
        assert_eq!(pending.desc, "fetching");

        let (id, action) = rx.try_recv().expect("dispatched action");
        assert_eq!(id, 1);
        assert!(matches!(action, Action::Fetch { remote: None }));
    }

    #[test]
    fn pressing_f_is_ignored_when_already_pending() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = App::new(Arc::new(NoopRepo), tx);
        app.local_branches = vec![br("main", true)];
        app.on_key(k(KeyCode::Char('f')));
        let first_id = app.pending_task.as_ref().unwrap().id;
        app.on_key(k(KeyCode::Char('f')));
        assert_eq!(app.pending_task.as_ref().unwrap().id, first_id);
        // Only one dispatch should have reached the worker channel.
        let (_, _) = rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn on_task_result_clears_pending_on_success() {
        let mut app = app_with(vec![br("main", true)]);
        app.pending_task = Some(PendingTask {
            id: 7,
            desc: "fetching".to_string(),
        });
        app.on_task_result(7, Ok(Outcome::Fetched));
        assert!(app.pending_task.is_none());
        assert_eq!(app.status, "fetched");
    }

    #[test]
    fn on_task_result_ignores_stale_ids() {
        let mut app = app_with(vec![br("main", true)]);
        app.pending_task = Some(PendingTask {
            id: 7,
            desc: "fetching".to_string(),
        });
        app.on_task_result(99, Ok(Outcome::Fetched));
        assert!(app.pending_task.is_some());
    }

    #[test]
    fn on_task_result_error_clears_pending_and_reports() {
        let mut app = app_with(vec![br("main", true)]);
        app.pending_task = Some(PendingTask {
            id: 7,
            desc: "fetching".to_string(),
        });
        app.on_task_result(7, Err("network down".to_string()));
        assert!(app.pending_task.is_none());
        assert!(app.status.contains("error"));
        assert!(app.status.contains("network down"));
    }

    #[test]
    fn on_tick_advances_spinner_only_when_pending() {
        let mut app = app_with(vec![br("main", true)]);
        let idle_before = app.spinner_frame;
        app.on_tick();
        assert_eq!(app.spinner_frame, idle_before);

        app.pending_task = Some(PendingTask {
            id: 1,
            desc: "fetching".to_string(),
        });
        let pending_before = app.spinner_frame;
        app.on_tick();
        assert_eq!(app.spinner_frame, pending_before.wrapping_add(1));
    }
}
