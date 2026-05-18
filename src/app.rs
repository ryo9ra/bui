use std::io;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::config::Config;
use crate::event::{Event, EventChannel, Outcome, TaskId};
use crate::git::{Branch, BranchDiff, RemoteBranch, Repo, Worktree};
use crate::task::Action;
use crate::ui;
use crate::ui::layout::{LayoutSpec, RightPane};

pub struct App {
    pub repo: Arc<dyn Repo>,
    pub config: Config,
    pub task_tx: Sender<(TaskId, Action)>,
    pub next_task_id: TaskId,
    pub pending_task: Option<PendingTask>,
    pub spinner_frame: usize,
    pub local_branches: Vec<Branch>,
    pub remote_branches: Vec<RemoteBranch>,
    pub worktrees: Vec<Worktree>,
    /// Index into `visible_branches()` (Local tab).
    pub selected: usize,
    /// Index into `visible_remote_branches()` (Remote tab).
    pub selected_remote: usize,
    /// Index into `worktrees` (Worktree tab).
    pub selected_worktree: usize,
    pub filter: String,
    pub search_active: bool,
    pub sort_mode: SortMode,
    pub filter_predicate: FilterPredicate,
    pub status: String,
    pub active_tab: Tab,
    pub modal: Option<Modal>,
    pub input: Option<InputState>,
    pub confirm: Option<ConfirmState>,
    pub upstream_picker: Option<UpstreamPickerState>,
    pub layout: LayoutSpec,
    pub right_pane: RightPane,
    /// Lazily computed and cached when `right_pane == Diff`. Cleared on
    /// refresh / selection change so it can't go stale.
    pub branch_diff: Option<BranchDiff>,
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SortMode {
    Recency,
    Name,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Recency => "recency",
            SortMode::Name => "name",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FilterPredicate {
    All,
    Merged,
    Unmerged,
}

impl FilterPredicate {
    pub fn label(self) -> &'static str {
        match self {
            FilterPredicate::All => "all",
            FilterPredicate::Merged => "merged",
            FilterPredicate::Unmerged => "unmerged",
        }
    }

    pub fn matches(self, b: &Branch) -> bool {
        match self {
            FilterPredicate::All => true,
            FilterPredicate::Merged => b.is_merged,
            FilterPredicate::Unmerged => !b.is_merged,
        }
    }
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
    CreateBranchFrom { source: String },
    RenameBranch { old: String },
    /// Step 1 of the worktree-add flow: pick the new branch name (or leave
    /// empty to reuse `base`).
    AddWorktreeName {
        base: String,
    },
    /// Step 2: the path. `new_branch` is `Some` if step 1 entered a name.
    AddWorktreePath {
        base: String,
        new_branch: Option<String>,
    },
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

    pub fn create_branch_from(source: String, default_name: String) -> Self {
        Self {
            prompt: format!("Create branch from '{source}'"),
            value: default_name,
            mode: InputMode::CreateBranchFrom { source },
        }
    }

    pub fn add_worktree_step_name(base: String) -> Self {
        Self {
            prompt: format!("New branch off '{base}' (empty = use existing):"),
            value: String::new(),
            mode: InputMode::AddWorktreeName { base },
        }
    }

    pub fn add_worktree_step_path(
        base: String,
        new_branch: Option<String>,
        worktree_root: Option<&str>,
    ) -> Self {
        let label = new_branch.as_deref().unwrap_or(&base).to_string();
        let default = default_worktree_path(worktree_root, &label);
        Self {
            prompt: format!("Worktree path for '{label}' (~ expands, ^U clears)"),
            value: default,
            mode: InputMode::AddWorktreePath { base, new_branch },
        }
    }
}

/// Prefill suggestion for the worktree path:
/// - When `[worktree] root` is set, place under it (preserves any slashes
///   in the branch name to mirror its hierarchy).
/// - Otherwise, drop a `wt-<leaf>` sibling beside the current worktree
///   (`../` parent) — leaf = last `/`-separated segment of the branch so
///   nested branch names don't accidentally create deep paths.
pub(crate) fn default_worktree_path(root: Option<&str>, branch: &str) -> String {
    match root {
        Some(r) => format!("{}/{branch}", r.trim_end_matches('/')),
        None => {
            let leaf = branch.rsplit('/').next().unwrap_or(branch);
            format!("../wt-{leaf}")
        }
    }
}

pub struct ConfirmState {
    pub prompt: String,
    pub action: ConfirmAction,
    pub focus: ConfirmChoice,
}

pub struct UpstreamPickerState {
    /// Local branch we're configuring tracking for.
    pub branch: String,
    /// Unique remote names (e.g. `origin`, `forked`). bui composes the
    /// full upstream ref as `<remote>/<branch>` on submit.
    pub candidates: Vec<String>,
    /// Cursor into `visible_candidates()`.
    pub selected: usize,
    /// Case-insensitive substring filter applied to candidates.
    pub filter: String,
}

impl UpstreamPickerState {
    pub fn new(branch: String, remote_branches: &[RemoteBranch]) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut candidates: Vec<String> = Vec::new();
        for r in remote_branches {
            if seen.insert(r.remote.clone()) {
                candidates.push(r.remote.clone());
            }
        }
        // Pre-select `origin` if it's one of the remotes (the overwhelming
        // common case); otherwise the first.
        let selected = candidates
            .iter()
            .position(|c| c == "origin")
            .unwrap_or(0);
        Self {
            branch,
            candidates,
            selected,
            filter: String::new(),
        }
    }

    pub fn visible_candidates(&self) -> Vec<&String> {
        if self.filter.is_empty() {
            return self.candidates.iter().collect();
        }
        let needle = self.filter.to_lowercase();
        self.candidates
            .iter()
            .filter(|c| c.to_lowercase().contains(&needle))
            .collect()
    }

    pub fn current(&self) -> Option<String> {
        self.visible_candidates()
            .get(self.selected)
            .map(|s| (*s).clone())
    }

    /// The full upstream ref bui will hand to `git branch --set-upstream-to=`.
    pub fn target(&self) -> Option<String> {
        self.current().map(|r| format!("{r}/{}", self.branch))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ConfirmChoice {
    Yes,
    No,
}

pub enum ConfirmAction {
    DeleteBranch { name: String, force: bool },
    DeleteRemoteBranch { remote: String, branch: String },
    ForceWithLeasePush,
    RemoveWorktree { path: String },
}

impl App {
    pub fn new(
        repo: Arc<dyn Repo>,
        task_tx: Sender<(TaskId, Action)>,
        config: Config,
    ) -> Self {
        Self {
            repo,
            config,
            task_tx,
            next_task_id: 1,
            pending_task: None,
            spinner_frame: 0,
            local_branches: Vec::new(),
            remote_branches: Vec::new(),
            worktrees: Vec::new(),
            selected: 0,
            selected_remote: 0,
            selected_worktree: 0,
            filter: String::new(),
            search_active: false,
            sort_mode: SortMode::Recency,
            filter_predicate: FilterPredicate::All,
            status: "ready".to_string(),
            active_tab: Tab::Local,
            modal: None,
            input: None,
            confirm: None,
            upstream_picker: None,
            layout: LayoutSpec::default_layout(),
            right_pane: RightPane::Detail,
            branch_diff: None,
            should_quit: false,
            dirty: true,
        }
    }

    fn toggle_right_pane(&mut self) {
        self.right_pane = match self.right_pane {
            RightPane::Detail => RightPane::Diff,
            RightPane::Diff => RightPane::Detail,
        };
        if self.right_pane == RightPane::Diff {
            self.recompute_branch_diff();
        } else {
            self.branch_diff = None;
        }
        self.status = match self.right_pane {
            RightPane::Detail => "right pane: detail".to_string(),
            RightPane::Diff => "right pane: diff".to_string(),
        };
    }

    fn recompute_branch_diff(&mut self) {
        let Some(target) = self.selected_branch().map(|b| b.name.clone()) else {
            self.branch_diff = None;
            return;
        };
        let Some(base) = self
            .local_branches
            .iter()
            .find(|b| b.is_current)
            .map(|b| b.name.clone())
        else {
            self.branch_diff = None;
            return;
        };
        if target == base {
            self.branch_diff = None;
            return;
        }
        match self.repo.branch_diff(&target, &base) {
            Ok(d) => self.branch_diff = Some(d),
            Err(_) => self.branch_diff = None,
        }
    }

    fn maybe_refresh_branch_diff(&mut self) {
        if self.right_pane != RightPane::Diff {
            return;
        }
        if self.active_tab != Tab::Local {
            return;
        }
        let target = self.selected_branch().map(|b| b.name.clone());
        let cur = self.branch_diff.as_ref().map(|d| d.target.clone());
        if target != cur {
            self.recompute_branch_diff();
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
            Ok(Outcome::RemoteBranchDeleted { full_name }) => {
                self.refresh(None);
                self.status = format!("deleted {full_name}");
            }
            Err(e) => self.handle_task_error(&e),
        }
        self.dirty = true;
    }

    fn handle_task_error(&mut self, msg: &str) {
        // Diverged history on a regular push: offer the safer
        // --force-with-lease retry instead of just surfacing the error.
        if msg.contains("non-fast-forward") {
            self.confirm = Some(ConfirmState {
                prompt: "Remote has diverged. Force-with-lease push?".to_string(),
                action: ConfirmAction::ForceWithLeasePush,
                focus: ConfirmChoice::No,
            });
            self.status = "diverged — confirm force-with-lease".to_string();
            return;
        }
        self.status = format!("error: {msg}");
    }

    pub fn visible_branches(&self) -> Vec<&Branch> {
        let mut filtered: Vec<&Branch> = if self.filter.is_empty() {
            self.local_branches.iter().collect()
        } else {
            let needle = self.filter.to_lowercase();
            self.local_branches
                .iter()
                .filter(|b| b.name.to_lowercase().contains(&needle))
                .collect()
        };
        filtered.retain(|b| self.filter_predicate.matches(b));
        if self.sort_mode == SortMode::Name {
            filtered.sort_by(|a, b| a.name.cmp(&b.name));
        }
        filtered
    }

    pub fn visible_remote_branches(&self) -> Vec<&RemoteBranch> {
        let mut filtered: Vec<&RemoteBranch> = if self.filter.is_empty() {
            self.remote_branches.iter().collect()
        } else {
            let needle = self.filter.to_lowercase();
            self.remote_branches
                .iter()
                .filter(|b| b.full_name.to_lowercase().contains(&needle))
                .collect()
        };
        if self.sort_mode == SortMode::Name {
            filtered.sort_by(|a, b| a.full_name.cmp(&b.full_name));
        }
        filtered
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
            Tab::Worktree => self.worktrees.len(),
        }
    }

    fn current_selected(&self) -> usize {
        match self.active_tab {
            Tab::Local => self.selected,
            Tab::Remote => self.selected_remote,
            Tab::Worktree => self.selected_worktree,
        }
    }

    fn set_current_selected(&mut self, i: usize) {
        match self.active_tab {
            Tab::Local => self.selected = i,
            Tab::Remote => self.selected_remote = i,
            Tab::Worktree => self.selected_worktree = i,
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
        if let Ok(ws) = self.repo.list_worktrees() {
            self.worktrees = ws;
            if self.selected_worktree >= self.worktrees.len() {
                self.selected_worktree = self.worktrees.len().saturating_sub(1);
            }
        }
        self.dirty = true;
    }

    pub fn selected_worktree_entry(&self) -> Option<&Worktree> {
        self.worktrees.get(self.selected_worktree)
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
        // Ctrl-C is the universal escape hatch — quits even when a modal,
        // input, picker, confirm, or search is open. Matches shell
        // expectations.
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            self.should_quit = true;
            return;
        }
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
        if self.upstream_picker.is_some() {
            self.handle_upstream_picker_key(key);
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
            KeyCode::Enter if self.active_tab == Tab::Remote => self.checkout_remote_selected(),
            KeyCode::Char('c') if self.active_tab == Tab::Local => {
                self.input = Some(InputState::create_branch());
            }
            KeyCode::Char('C') => self.open_create_from_selected(),
            KeyCode::Char('r') if self.active_tab == Tab::Local => {
                if let Some(old) = self.selected_name() {
                    self.input = Some(InputState::rename_branch(old));
                }
            }
            KeyCode::Char('/') if self.active_tab != Tab::Worktree => self.start_search(),
            KeyCode::Char('s') => self.cycle_sort_mode(),
            KeyCode::Char('F') if self.active_tab == Tab::Local => {
                self.cycle_filter_predicate();
            }
            // Esc with nothing to dismiss → quit. When a filter is applied
            // it's cleared first; the user has to press Esc twice to quit
            // from a filtered view.
            KeyCode::Esc if !self.filter.is_empty() => self.clear_filter(),
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('d') if self.active_tab == Tab::Local => self.request_delete(false),
            KeyCode::Char('D') if self.active_tab == Tab::Local => self.request_delete(true),
            KeyCode::Char('d') if self.active_tab == Tab::Remote => self.request_delete_remote(),
            KeyCode::Char('d') if self.active_tab == Tab::Worktree => {
                self.request_remove_worktree();
            }
            KeyCode::Char('W') if self.active_tab == Tab::Local => self.open_add_worktree_input(),
            KeyCode::Enter if self.active_tab == Tab::Worktree => self.show_worktree_cd_hint(),
            KeyCode::Char('v') => self.toggle_right_pane(),
            KeyCode::Char('f') if self.pending_task.is_none() => {
                let prune_tags = self.config.fetch.prune_tags;
                self.dispatch(
                    Action::Fetch {
                        remote: None,
                        prune_tags,
                    },
                    "fetching",
                );
            }
            KeyCode::Char('p') if self.pending_task.is_none() => {
                self.dispatch(Action::Pull, "pulling");
            }
            KeyCode::Char('P') if self.pending_task.is_none() => {
                self.dispatch(Action::Push, "pushing");
            }
            KeyCode::Char('u') if self.active_tab == Tab::Local => self.open_upstream_picker(),
            _ => {}
        }
        self.maybe_refresh_branch_diff();
        self.dirty = true;
    }

    fn checkout_remote_selected(&mut self) {
        let Some(rb) = self.selected_remote_branch() else {
            return;
        };
        let local_name = rb.name.clone();
        let remote_ref = rb.full_name.clone();
        let local_exists = self
            .local_branches
            .iter()
            .any(|b| b.name == local_name);

        let result = if local_exists {
            self.repo.checkout(&local_name)
        } else {
            self.repo
                .checkout_remote_tracking(&local_name, &remote_ref)
        };

        match result {
            Ok(()) => {
                self.refresh(Some(&local_name));
                self.active_tab = Tab::Local;
                if let Some(i) = self
                    .visible_branches()
                    .iter()
                    .position(|b| b.name == local_name)
                {
                    self.selected = i;
                }
                self.status = if local_exists {
                    format!("switched to {local_name}")
                } else {
                    format!("tracked {remote_ref} as {local_name}")
                };
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    fn open_add_worktree_input(&mut self) {
        let Some(b) = self.selected_branch() else {
            return;
        };
        self.input = Some(InputState::add_worktree_step_name(b.name.clone()));
    }

    fn show_worktree_cd_hint(&mut self) {
        let Some(w) = self.selected_worktree_entry() else {
            return;
        };
        self.status = format!("cd {}", w.path);
    }

    fn request_remove_worktree(&mut self) {
        let Some(w) = self.selected_worktree_entry() else {
            return;
        };
        if w.is_current {
            self.status = "cannot remove the current worktree".to_string();
            return;
        }
        let path = w.path.clone();
        self.confirm = Some(ConfirmState {
            prompt: format!("Remove worktree at '{path}'?"),
            action: ConfirmAction::RemoveWorktree { path },
            focus: ConfirmChoice::No,
        });
    }

    fn do_remove_worktree(&mut self, path: &str) {
        match self.repo.remove_worktree(path) {
            Ok(()) => {
                self.refresh(None);
                self.status = format!("removed worktree {path}");
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    fn do_add_worktree(&mut self, path: &str, base: &str, new_branch: Option<&str>) {
        let expanded = expand_tilde(path);
        match self.repo.add_worktree(&expanded, base, new_branch) {
            Ok(()) => {
                self.refresh(None);
                self.active_tab = Tab::Worktree;
                if let Some(i) = self.worktrees.iter().position(|w| w.path == expanded) {
                    self.selected_worktree = i;
                }
                self.status = match new_branch {
                    Some(name) => {
                        format!("added worktree {expanded} on new branch {name} (off {base})")
                    }
                    None => format!("added worktree {expanded} for {base}"),
                };
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    fn request_delete_remote(&mut self) {
        if self.pending_task.is_some() {
            self.status = "another op is in progress".to_string();
            return;
        }
        let Some(rb) = self.selected_remote_branch() else {
            return;
        };
        let remote = rb.remote.clone();
        let branch = rb.name.clone();
        let full = rb.full_name.clone();
        self.confirm = Some(ConfirmState {
            prompt: format!("Delete remote branch '{full}'? (push --delete)"),
            action: ConfirmAction::DeleteRemoteBranch { remote, branch },
            focus: ConfirmChoice::No,
        });
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

    fn open_create_from_selected(&mut self) {
        match self.active_tab {
            Tab::Local => {
                if let Some(b) = self.selected_branch() {
                    self.input = Some(InputState::create_branch_from(
                        b.name.clone(),
                        String::new(),
                    ));
                }
            }
            Tab::Remote => {
                if let Some(r) = self.selected_remote_branch() {
                    self.input = Some(InputState::create_branch_from(
                        r.full_name.clone(),
                        r.name.clone(),
                    ));
                }
            }
            Tab::Worktree => {}
        }
    }

    fn open_upstream_picker(&mut self) {
        let Some(branch) = self.selected_name() else {
            return;
        };
        if self.remote_branches.is_empty() {
            self.status = "no remote branches — run fetch (f) first".to_string();
            return;
        }
        self.upstream_picker = Some(UpstreamPickerState::new(branch, &self.remote_branches));
    }

    fn handle_upstream_picker_key(&mut self, key: KeyEvent) {
        let Some(picker) = self.upstream_picker.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.upstream_picker = None;
                self.status = "cancelled".to_string();
            }
            KeyCode::Enter => self.submit_upstream_picker(),
            KeyCode::Up if picker.selected > 0 => picker.selected -= 1,
            KeyCode::Down if picker.selected + 1 < picker.visible_candidates().len() => {
                picker.selected += 1;
            }
            KeyCode::Backspace => {
                picker.filter.pop();
                picker.selected = 0;
            }
            KeyCode::Char(c) => {
                picker.filter.push(c);
                picker.selected = 0;
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn submit_upstream_picker(&mut self) {
        let Some(picker) = self.upstream_picker.take() else {
            return;
        };
        let Some(upstream) = picker.target() else {
            self.status = "no remote selected".to_string();
            return;
        };
        let branch = picker.branch;
        match self.repo.set_upstream(&branch, &upstream) {
            Ok(()) => {
                self.refresh(Some(&branch));
                self.status = format!("set upstream {branch} -> {upstream}");
            }
            Err(e) => self.status = format!("error: {e}"),
        }
    }

    fn execute_confirm(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::DeleteBranch { name, force } => self.do_delete_branch(&name, force),
            ConfirmAction::DeleteRemoteBranch { remote, branch } => {
                let desc = format!("deleting {remote}/{branch}");
                self.dispatch(
                    Action::DeleteRemoteBranch {
                        remote,
                        branch,
                    },
                    desc,
                );
            }
            ConfirmAction::ForceWithLeasePush => {
                self.dispatch(Action::PushForceWithLease, "force-with-lease push");
            }
            ConfirmAction::RemoveWorktree { path } => self.do_remove_worktree(&path),
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
            // ^U: readline-style "clear the whole line", handy when the
            // step-2 prefill isn't what the user wants.
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.value.clear();
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                input.value.push(c);
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn submit_input(&mut self) {
        let Some(input) = self.input.take() else {
            return;
        };
        let value = input.value.trim().to_string();
        let empty_cancel = |s: &mut Self| {
            s.status = "input cancelled (empty)".to_string();
        };
        match input.mode {
            InputMode::CreateBranch => {
                if value.is_empty() {
                    empty_cancel(self);
                    return;
                }
                self.do_create_branch(&value, None);
            }
            InputMode::CreateBranchFrom { source } => {
                if value.is_empty() {
                    empty_cancel(self);
                    return;
                }
                self.do_create_branch(&value, Some(&source));
            }
            InputMode::RenameBranch { old } => {
                if value.is_empty() {
                    empty_cancel(self);
                    return;
                }
                self.do_rename_branch(&old, &value);
            }
            InputMode::AddWorktreeName { base } => {
                // Step 1 → step 2. Empty value is meaningful here: it means
                // "reuse the existing base branch, don't create a new one".
                let new_branch = if value.is_empty() { None } else { Some(value) };
                let root = self.config.worktree.root.clone();
                self.input = Some(InputState::add_worktree_step_path(
                    base,
                    new_branch,
                    root.as_deref(),
                ));
            }
            InputMode::AddWorktreePath { base, new_branch } => {
                if value.is_empty() {
                    empty_cancel(self);
                    return;
                }
                self.do_add_worktree(&value, &base, new_branch.as_deref());
            }
        }
    }

    fn do_create_branch(&mut self, name: &str, source: Option<&str>) {
        match self.repo.create_branch(name, source) {
            Ok(()) => {
                self.refresh(Some(name));
                self.status = match source {
                    Some(src) => format!("created {name} from {src}"),
                    None => format!("created {name}"),
                };
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

    fn cycle_filter_predicate(&mut self) {
        let prefer = self.selected_branch().map(|b| b.name.clone());
        self.filter_predicate = match self.filter_predicate {
            FilterPredicate::All => FilterPredicate::Merged,
            FilterPredicate::Merged => FilterPredicate::Unmerged,
            FilterPredicate::Unmerged => FilterPredicate::All,
        };
        let visible = self.visible_branches();
        if let Some(name) = prefer
            && let Some(i) = visible.iter().position(|b| b.name == name)
        {
            self.selected = i;
        } else if self.selected >= visible.len() {
            self.selected = visible.len().saturating_sub(1);
        }
        self.status = format!("filter: {}", self.filter_predicate.label());
    }

    fn cycle_sort_mode(&mut self) {
        let prefer = match self.active_tab {
            Tab::Local => self.selected_branch().map(|b| b.name.clone()),
            Tab::Remote => self.selected_remote_branch().map(|b| b.full_name.clone()),
            Tab::Worktree => None,
        };
        self.sort_mode = match self.sort_mode {
            SortMode::Recency => SortMode::Name,
            SortMode::Name => SortMode::Recency,
        };
        if let Some(name) = prefer {
            match self.active_tab {
                Tab::Local => {
                    if let Some(i) = self.visible_branches().iter().position(|b| b.name == name) {
                        self.selected = i;
                    }
                }
                Tab::Remote => {
                    if let Some(i) = self
                        .visible_remote_branches()
                        .iter()
                        .position(|b| b.full_name == name)
                    {
                        self.selected_remote = i;
                    }
                }
                Tab::Worktree => {}
            }
        }
        self.status = format!("sort: {}", self.sort_mode.label());
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

fn expand_tilde(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return format!("{}/{rest}", home.to_string_lossy());
    }
    p.to_string()
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
        fn fetch(&self, _: Option<&str>, _: bool) -> anyhow::Result<()> {
            Ok(())
        }
        fn pull(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn push(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn push_force_with_lease(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn set_upstream(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn delete_remote_branch(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn checkout_remote_tracking(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn list_worktrees(&self) -> anyhow::Result<Vec<Worktree>> {
            Ok(vec![])
        }
        fn add_worktree(&self, _: &str, _: &str, _: Option<&str>) -> anyhow::Result<()> {
            Ok(())
        }
        fn remove_worktree(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn branch_diff(&self, target: &str, base: &str) -> anyhow::Result<BranchDiff> {
            Ok(BranchDiff {
                target: target.to_string(),
                base: base.to_string(),
                ahead: vec![],
                behind: vec![],
            })
        }
    }

    fn br(name: &str, current: bool) -> Branch {
        Branch {
            name: name.to_string(),
            is_current: current,
            short_sha: "abc1234".to_string(),
            subject: "subject".to_string(),
            rel_date: "1 day ago".to_string(),
            is_merged: false,
            worktree_path: None,
        }
    }

    fn app_with(branches: Vec<Branch>) -> App {
        let (tx, rx) = std::sync::mpsc::channel();
        // Leak the receiver so `dispatch()` calls don't fail with a closed
        // channel; tests that inspect dispatched actions build their own
        // channel directly.
        std::mem::forget(rx);
        let mut a = App::new(Arc::new(NoopRepo), tx, Config::default());
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
            _ => panic!("expected DeleteBranch action"),
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
            _ => panic!("expected DeleteBranch action"),
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
        let mut app = App::new(Arc::new(NoopRepo), tx, Config::default());
        app.local_branches = vec![br("main", true)];
        app.on_key(k(KeyCode::Char('f')));

        let pending = app.pending_task.as_ref().expect("pending should be set");
        assert_eq!(pending.id, 1);
        assert_eq!(pending.desc, "fetching");

        let (id, action) = rx.try_recv().expect("dispatched action");
        assert_eq!(id, 1);
        assert!(matches!(
            action,
            Action::Fetch {
                remote: None,
                prune_tags: false
            }
        ));
    }

    #[test]
    fn pressing_f_with_prune_tags_config_passes_flag_to_worker() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut cfg = Config::default();
        cfg.fetch.prune_tags = true;
        let mut app = App::new(Arc::new(NoopRepo), tx, cfg);
        app.local_branches = vec![br("main", true)];
        app.on_key(k(KeyCode::Char('f')));
        let (_, action) = rx.try_recv().expect("dispatched action");
        assert!(matches!(
            action,
            Action::Fetch {
                remote: None,
                prune_tags: true
            }
        ));
    }

    #[test]
    fn pressing_f_is_ignored_when_already_pending() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = App::new(Arc::new(NoopRepo), tx, Config::default());
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

    fn remote(remote: &str, name: &str) -> RemoteBranch {
        RemoteBranch {
            remote: remote.to_string(),
            name: name.to_string(),
            full_name: format!("{remote}/{name}"),
            short_sha: "abc1234".to_string(),
            subject: "subj".to_string(),
            rel_date: "1 hour ago".to_string(),
        }
    }

    #[test]
    fn upstream_picker_dedupes_remotes() {
        let remotes = vec![
            remote("origin", "main"),
            remote("origin", "feature/foo"),
            remote("origin", "other"),
        ];
        let picker = UpstreamPickerState::new("foo".to_string(), &remotes);
        assert_eq!(picker.candidates, vec!["origin".to_string()]);
        assert_eq!(picker.selected, 0);
    }

    #[test]
    fn upstream_picker_preselects_origin_when_present() {
        let remotes = vec![
            remote("forked", "main"),
            remote("origin", "main"),
            remote("upstream", "main"),
        ];
        let picker = UpstreamPickerState::new("foo".to_string(), &remotes);
        assert!(picker.candidates.contains(&"origin".to_string()));
        assert_eq!(picker.candidates[picker.selected], "origin");
    }

    #[test]
    fn upstream_picker_falls_back_to_first_without_origin() {
        let remotes = vec![remote("forked", "main"), remote("upstream", "main")];
        let picker = UpstreamPickerState::new("foo".to_string(), &remotes);
        assert_eq!(picker.selected, 0);
    }

    #[test]
    fn upstream_picker_filter_case_insensitive_substring() {
        let remotes = vec![
            remote("origin", "main"),
            remote("Forked", "main"),
            remote("upstream", "main"),
        ];
        let mut picker = UpstreamPickerState::new("foo".to_string(), &remotes);
        picker.filter = "or".to_string();
        let visible: Vec<_> = picker
            .visible_candidates()
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(visible, ["origin", "Forked"]);
    }

    #[test]
    fn upstream_picker_target_composes_full_ref() {
        let remotes = vec![remote("origin", "main")];
        let picker = UpstreamPickerState::new("feature/foo".to_string(), &remotes);
        assert_eq!(
            picker.target(),
            Some("origin/feature/foo".to_string())
        );
    }

    #[test]
    fn pressing_u_opens_picker_with_remote_names() {
        let mut app = app_with(vec![br("feature/foo", true)]);
        app.remote_branches = vec![
            remote("origin", "feature/foo"),
            remote("forked", "main"),
        ];
        app.on_key(k(KeyCode::Char('u')));
        let picker = app.upstream_picker.as_ref().expect("picker should be open");
        assert_eq!(picker.branch, "feature/foo");
        assert!(picker.candidates.contains(&"origin".to_string()));
        assert!(picker.candidates.contains(&"forked".to_string()));
        assert_eq!(picker.candidates[picker.selected], "origin");
    }

    #[test]
    fn pressing_u_with_no_remotes_sets_status_and_skips() {
        let mut app = app_with(vec![br("main", true)]);
        app.on_key(k(KeyCode::Char('u')));
        assert!(app.upstream_picker.is_none());
        assert!(app.status.contains("no remote branches"));
    }

    #[test]
    fn upstream_picker_arrow_navigates_visible_list() {
        let mut app = app_with(vec![br("foo", true)]);
        app.remote_branches = vec![
            remote("forked", "main"),
            remote("origin", "main"),
            remote("upstream", "main"),
        ];
        app.on_key(k(KeyCode::Char('u')));
        let before = app.upstream_picker.as_ref().unwrap().selected;
        app.handle_upstream_picker_key(k(KeyCode::Down));
        let after_down = app.upstream_picker.as_ref().unwrap().selected;
        assert!(after_down == before + 1 || (after_down == before && before + 1 >= app.upstream_picker.as_ref().unwrap().candidates.len()));
        app.handle_upstream_picker_key(k(KeyCode::Up));
        assert!(app.upstream_picker.as_ref().unwrap().selected <= after_down);
    }

    #[test]
    fn upstream_picker_enter_sets_upstream_to_remote_slash_branch() {
        let mut app = app_with(vec![br("feature/foo", true)]);
        app.remote_branches = vec![remote("origin", "feature/foo")];
        app.on_key(k(KeyCode::Char('u')));
        app.handle_upstream_picker_key(k(KeyCode::Enter));
        assert!(app.upstream_picker.is_none());
        assert_eq!(
            app.status,
            "set upstream feature/foo -> origin/feature/foo"
        );
    }

    fn br_merged(name: &str, current: bool, merged: bool) -> Branch {
        let mut b = br(name, current);
        b.is_merged = merged;
        b
    }

    #[test]
    fn visible_branches_filter_predicate_merged() {
        let mut app = app_with(vec![
            br_merged("main", true, true),
            br_merged("feature/done", false, true),
            br_merged("feature/wip", false, false),
        ]);
        app.filter_predicate = FilterPredicate::Merged;
        let names: Vec<_> = app
            .visible_branches()
            .iter()
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(names, ["main", "feature/done"]);
    }

    #[test]
    fn visible_branches_filter_predicate_unmerged() {
        let mut app = app_with(vec![
            br_merged("main", true, true),
            br_merged("feature/done", false, true),
            br_merged("feature/wip", false, false),
        ]);
        app.filter_predicate = FilterPredicate::Unmerged;
        let names: Vec<_> = app
            .visible_branches()
            .iter()
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(names, ["feature/wip"]);
    }

    #[test]
    fn cycle_filter_predicate_walks_all_merged_unmerged() {
        let mut app = app_with(vec![br_merged("main", true, true)]);
        assert_eq!(app.filter_predicate, FilterPredicate::All);
        app.cycle_filter_predicate();
        assert_eq!(app.filter_predicate, FilterPredicate::Merged);
        app.cycle_filter_predicate();
        assert_eq!(app.filter_predicate, FilterPredicate::Unmerged);
        app.cycle_filter_predicate();
        assert_eq!(app.filter_predicate, FilterPredicate::All);
    }

    #[test]
    fn cycle_filter_predicate_clamps_cursor_when_branch_filtered_out() {
        let mut app = app_with(vec![
            br_merged("main", true, true),
            br_merged("wip", false, false),
        ]);
        app.selected = 1; // on "wip"
        app.cycle_filter_predicate(); // → Merged; "wip" disappears
        // visible is just ["main"], cursor clamps to 0.
        assert_eq!(app.selected, 0);
        assert_eq!(app.filter_predicate, FilterPredicate::Merged);
    }

    #[test]
    fn visible_branches_sorted_by_name_when_mode_is_name() {
        let mut app = app_with(vec![
            br("zeta", false),
            br("alpha", false),
            br("middle", true),
        ]);
        app.sort_mode = SortMode::Name;
        let names: Vec<_> = app
            .visible_branches()
            .iter()
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(names, ["alpha", "middle", "zeta"]);
    }

    #[test]
    fn visible_branches_preserves_recency_order_by_default() {
        let app = app_with(vec![
            br("zeta", false),
            br("alpha", false),
            br("middle", true),
        ]);
        // Recency is the order we put them in (committerdate-desc from git).
        let names: Vec<_> = app
            .visible_branches()
            .iter()
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(names, ["zeta", "alpha", "middle"]);
    }

    #[test]
    fn cycle_sort_mode_toggles_and_preserves_selected_branch() {
        let mut app = app_with(vec![
            br("zeta", false),
            br("alpha", false),
            br("middle", true),
        ]);
        // Cursor on "alpha" (index 1 in recency order).
        app.selected = 1;
        app.cycle_sort_mode();
        assert_eq!(app.sort_mode, SortMode::Name);
        // After name sort, "alpha" is at index 0.
        assert_eq!(app.selected, 0);
        assert!(app.status.contains("name"));

        app.cycle_sort_mode();
        assert_eq!(app.sort_mode, SortMode::Recency);
        // Cursor back on "alpha" which is index 1 in recency order.
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn capital_c_on_local_opens_create_from_input_with_selected_source() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        app.selected = 1;
        app.on_key(k(KeyCode::Char('C')));
        let input = app.input.as_ref().expect("input should be open");
        match &input.mode {
            InputMode::CreateBranchFrom { source } => {
                assert_eq!(source, "feature/foo");
            }
            _ => panic!("expected CreateBranchFrom, got something else"),
        }
        assert_eq!(input.value, ""); // local: empty default
    }

    #[test]
    fn capital_c_on_remote_prefills_default_name() {
        let mut app = app_with(vec![br("main", true)]);
        app.remote_branches = vec![remote("origin", "feature/foo")];
        app.active_tab = Tab::Remote;
        app.selected_remote = 0;
        app.on_key(k(KeyCode::Char('C')));
        let input = app.input.as_ref().expect("input should be open");
        match &input.mode {
            InputMode::CreateBranchFrom { source } => {
                assert_eq!(source, "origin/feature/foo");
            }
            _ => panic!("expected CreateBranchFrom"),
        }
        assert_eq!(input.value, "feature/foo");
    }

    #[test]
    fn submit_create_branch_from_calls_repo_with_source() {
        let mut app = app_with(vec![br("main", true)]);
        app.input = Some(InputState::create_branch_from(
            "origin/main".to_string(),
            "topic".to_string(),
        ));
        app.submit_input();
        assert!(app.input.is_none());
        assert!(app.status.contains("created topic from origin/main"));
    }

    fn wt(path: &str, branch: Option<&str>, is_current: bool) -> Worktree {
        Worktree {
            path: path.to_string(),
            head: "abc1234".to_string(),
            branch: branch.map(|s| s.to_string()),
            is_current,
        }
    }

    #[test]
    fn capital_w_on_local_opens_worktree_name_step() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        app.selected = 1;
        app.on_key(k(KeyCode::Char('W')));
        let input = app.input.as_ref().expect("input should be open");
        match &input.mode {
            InputMode::AddWorktreeName { base } => assert_eq!(base, "feature/foo"),
            _ => panic!("expected AddWorktreeName mode"),
        }
        assert_eq!(input.value, "");
    }

    #[test]
    fn empty_name_step_skips_branch_creation_and_advances_to_path() {
        let mut app = app_with(vec![br("main", true)]);
        app.selected = 0;
        app.on_key(k(KeyCode::Char('W')));
        // Submit step 1 with empty value.
        app.submit_input();
        let input = app.input.as_ref().expect("step 2 should be open");
        match &input.mode {
            InputMode::AddWorktreePath { base, new_branch } => {
                assert_eq!(base, "main");
                assert!(new_branch.is_none());
            }
            _ => panic!("expected AddWorktreePath after step 1"),
        }
    }

    #[test]
    fn named_step_carries_new_branch_into_path_step() {
        let mut app = app_with(vec![br("main", true)]);
        app.selected = 0;
        app.on_key(k(KeyCode::Char('W')));
        if let Some(input) = app.input.as_mut() {
            input.value = "oauth-wip".to_string();
        }
        app.submit_input();
        let input = app.input.as_ref().expect("step 2 should be open");
        match &input.mode {
            InputMode::AddWorktreePath { base, new_branch } => {
                assert_eq!(base, "main");
                assert_eq!(new_branch.as_deref(), Some("oauth-wip"));
            }
            _ => panic!("expected AddWorktreePath after step 1"),
        }
    }

    #[test]
    fn default_worktree_path_uses_root_when_set() {
        assert_eq!(
            default_worktree_path(Some("~/wt"), "feature/oauth"),
            "~/wt/feature/oauth"
        );
        assert_eq!(
            default_worktree_path(Some("/abs/dir/"), "main"),
            "/abs/dir/main"
        );
    }

    #[test]
    fn default_worktree_path_uses_leaf_fallback_without_root() {
        assert_eq!(
            default_worktree_path(None, "feature/oauth"),
            "../wt-oauth"
        );
        assert_eq!(default_worktree_path(None, "main"), "../wt-main");
    }

    #[test]
    fn path_step_prefills_default_from_config_root() {
        let mut app = app_with(vec![br("main", true)]);
        app.config.worktree.root = Some("~/wt".to_string());
        app.selected = 0;
        app.on_key(k(KeyCode::Char('W')));
        if let Some(input) = app.input.as_mut() {
            input.value = "oauth-wip".to_string();
        }
        app.submit_input(); // step 1 → step 2
        let input = app.input.as_ref().expect("step 2");
        assert_eq!(input.value, "~/wt/oauth-wip");
    }

    #[test]
    fn path_step_falls_back_to_leaf_default_without_root() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        app.selected = 1;
        app.on_key(k(KeyCode::Char('W')));
        // Empty step 1 → step 2 prefills against base.
        app.submit_input();
        let input = app.input.as_ref().expect("step 2");
        assert_eq!(input.value, "../wt-foo");
    }

    #[test]
    fn esc_with_no_filter_or_modal_quits() {
        let mut app = app_with(vec![br("main", true)]);
        app.on_key(k(KeyCode::Esc));
        assert!(app.should_quit);
    }

    #[test]
    fn esc_with_filter_clears_first_does_not_quit() {
        let mut app = app_with(vec![br("main", true)]);
        app.filter = "foo".to_string();
        app.on_key(k(KeyCode::Esc));
        assert!(!app.should_quit);
        assert!(app.filter.is_empty());
    }

    #[test]
    fn esc_inside_help_modal_closes_modal_does_not_quit() {
        let mut app = app_with(vec![br("main", true)]);
        app.modal = Some(Modal::Help);
        app.on_key(k(KeyCode::Esc));
        assert!(app.modal.is_none());
        assert!(!app.should_quit);
    }

    #[test]
    fn ctrl_c_quits_immediately_even_from_inside_a_modal() {
        let mut app = app_with(vec![br("main", true)]);
        app.modal = Some(Modal::Help);
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
        // Modal stays as-is — Ctrl-C doesn't pretend to gracefully close it,
        // it just exits.
    }

    #[test]
    fn ctrl_c_quits_from_input_state_too() {
        let mut app = app_with(vec![br("main", true)]);
        app.input = Some(InputState::create_branch());
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn v_toggles_right_pane_between_detail_and_diff() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        app.selected = 1;
        assert_eq!(app.right_pane, RightPane::Detail);
        app.on_key(k(KeyCode::Char('v')));
        assert_eq!(app.right_pane, RightPane::Diff);
        app.on_key(k(KeyCode::Char('v')));
        assert_eq!(app.right_pane, RightPane::Detail);
    }

    #[test]
    fn entering_diff_pane_triggers_branch_diff_computation() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        app.selected = 1;
        app.on_key(k(KeyCode::Char('v')));
        // NoopRepo returns an empty diff but with target/base populated.
        let diff = app.branch_diff.as_ref().expect("diff computed");
        assert_eq!(diff.target, "feature/foo");
        assert_eq!(diff.base, "main");
    }

    #[test]
    fn diff_against_current_branch_is_empty_no_op() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        // Cursor on current → diff suppressed.
        app.selected = 0;
        app.on_key(k(KeyCode::Char('v')));
        assert_eq!(app.right_pane, RightPane::Diff);
        assert!(app.branch_diff.is_none());
    }

    #[test]
    fn moving_cursor_in_diff_mode_recomputes() {
        let mut app = app_with(vec![
            br("main", true),
            br("feature/foo", false),
            br("feature/bar", false),
        ]);
        app.selected = 1;
        app.on_key(k(KeyCode::Char('v')));
        assert_eq!(app.branch_diff.as_ref().unwrap().target, "feature/foo");
        // Move down to feature/bar — diff should swap target.
        app.on_key(k(KeyCode::Char('j')));
        assert_eq!(app.branch_diff.as_ref().unwrap().target, "feature/bar");
    }

    #[test]
    fn leaving_diff_mode_clears_the_cached_diff() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        app.selected = 1;
        app.on_key(k(KeyCode::Char('v')));
        assert!(app.branch_diff.is_some());
        app.on_key(k(KeyCode::Char('v')));
        assert!(app.branch_diff.is_none());
    }

    #[test]
    fn ctrl_u_clears_input_value() {
        let mut app = app_with(vec![br("main", true)]);
        app.input = Some(InputState::create_branch());
        if let Some(input) = app.input.as_mut() {
            input.value = "pre-filled".to_string();
        }
        app.handle_input_key(KeyEvent::new(
            KeyCode::Char('u'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(app.input.as_ref().unwrap().value, "");
    }

    #[test]
    fn path_step_submission_completes_the_flow() {
        let mut app = app_with(vec![br("main", true)]);
        app.input = Some(InputState::add_worktree_step_path(
            "main".to_string(),
            Some("oauth-wip".to_string()),
            None,
        ));
        if let Some(input) = app.input.as_mut() {
            input.value = "/tmp/wt".to_string();
        }
        app.submit_input();
        assert!(app.input.is_none());
        assert_eq!(app.active_tab, Tab::Worktree);
        assert!(
            app.status.contains("on new branch oauth-wip"),
            "status: {}",
            app.status
        );
    }

    #[test]
    fn enter_on_worktree_sets_cd_hint_in_status() {
        let mut app = app_with(vec![br("main", true)]);
        app.worktrees = vec![
            wt("/repo/main", Some("main"), true),
            wt("/repo/wt-foo", Some("feature/foo"), false),
        ];
        app.active_tab = Tab::Worktree;
        app.selected_worktree = 1;
        app.on_key(k(KeyCode::Enter));
        assert_eq!(app.status, "cd /repo/wt-foo");
    }

    #[test]
    fn d_on_worktree_opens_confirm_except_for_current() {
        let mut app = app_with(vec![br("main", true)]);
        app.worktrees = vec![
            wt("/repo/main", Some("main"), true),
            wt("/repo/wt-foo", Some("feature/foo"), false),
        ];
        app.active_tab = Tab::Worktree;

        // current worktree -> no confirm, status hint
        app.selected_worktree = 0;
        app.on_key(k(KeyCode::Char('d')));
        assert!(app.confirm.is_none());
        assert!(app.status.contains("cannot remove the current worktree"));

        // non-current -> confirm opens
        app.selected_worktree = 1;
        app.on_key(k(KeyCode::Char('d')));
        let state = app.confirm.as_ref().expect("confirm should be open");
        match &state.action {
            ConfirmAction::RemoveWorktree { path } => {
                assert_eq!(path, "/repo/wt-foo");
            }
            _ => panic!("expected RemoveWorktree"),
        }
        assert_eq!(state.focus, ConfirmChoice::No);
    }

    #[test]
    fn tilde_expansion_resolves_against_home_env() {
        // Force HOME for determinism.
        unsafe {
            std::env::set_var("HOME", "/tmp/fake-home");
        }
        assert_eq!(expand_tilde("~/wt-foo"), "/tmp/fake-home/wt-foo");
        assert_eq!(expand_tilde("/abs/path"), "/abs/path");
        assert_eq!(expand_tilde("relative"), "relative");
    }

    #[test]
    fn enter_on_remote_with_no_matching_local_creates_tracking() {
        let mut app = app_with(vec![br("main", true)]);
        app.remote_branches = vec![remote("origin", "feature/foo")];
        app.active_tab = Tab::Remote;
        app.selected_remote = 0;
        app.on_key(k(KeyCode::Enter));
        assert_eq!(app.active_tab, Tab::Local);
        assert!(
            app.status.contains("tracked origin/feature/foo"),
            "status was: {}",
            app.status
        );
    }

    #[test]
    fn enter_on_remote_with_existing_local_switches() {
        let mut app = app_with(vec![br("main", true), br("feature/foo", false)]);
        app.remote_branches = vec![remote("origin", "feature/foo")];
        app.active_tab = Tab::Remote;
        app.selected_remote = 0;
        app.on_key(k(KeyCode::Enter));
        assert_eq!(app.active_tab, Tab::Local);
        assert_eq!(app.status, "switched to feature/foo");
    }

    #[test]
    fn on_task_result_non_fast_forward_opens_force_with_lease_confirm() {
        let mut app = app_with(vec![br("main", true)]);
        app.pending_task = Some(PendingTask {
            id: 9,
            desc: "pushing".to_string(),
        });
        app.on_task_result(9, Err("non-fast-forward: remote has diverged".to_string()));
        assert!(app.pending_task.is_none());
        let state = app
            .confirm
            .as_ref()
            .expect("confirm should be open after non-ff");
        assert_eq!(state.focus, ConfirmChoice::No);
        assert!(matches!(state.action, ConfirmAction::ForceWithLeasePush));
        assert!(app.status.contains("diverged"));
    }

    #[test]
    fn confirming_force_with_lease_dispatches_async_action() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = App::new(Arc::new(NoopRepo), tx, Config::default());
        app.pending_task = Some(PendingTask {
            id: 1,
            desc: "pushing".to_string(),
        });
        app.on_task_result(1, Err("non-fast-forward: foo".to_string()));
        app.handle_confirm_key(k(KeyCode::Left));
        app.handle_confirm_key(k(KeyCode::Enter));

        let (_, action) = rx.try_recv().expect("dispatched action");
        assert!(matches!(action, Action::PushForceWithLease));
        let pending = app.pending_task.as_ref().expect("re-pending");
        assert!(pending.desc.contains("force-with-lease"));
    }

    #[test]
    fn pressing_d_on_remote_tab_opens_confirm() {
        let mut app = app_with(vec![br("main", true)]);
        app.remote_branches = vec![remote("origin", "feature/foo")];
        app.active_tab = Tab::Remote;
        app.selected_remote = 0;
        app.on_key(k(KeyCode::Char('d')));
        let state = app.confirm.as_ref().expect("confirm should be set");
        assert_eq!(state.focus, ConfirmChoice::No);
        match &state.action {
            ConfirmAction::DeleteRemoteBranch { remote, branch } => {
                assert_eq!(remote, "origin");
                assert_eq!(branch, "feature/foo");
            }
            _ => panic!("expected DeleteRemoteBranch action"),
        }
    }

    #[test]
    fn confirming_remote_delete_dispatches_async_action() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = App::new(Arc::new(NoopRepo), tx, Config::default());
        app.remote_branches = vec![remote("origin", "feature/foo")];
        app.active_tab = Tab::Remote;
        app.selected_remote = 0;
        app.on_key(k(KeyCode::Char('d')));
        // Move focus to Yes and confirm.
        app.handle_confirm_key(k(KeyCode::Left));
        app.handle_confirm_key(k(KeyCode::Enter));

        assert!(app.confirm.is_none());
        // dispatch() should have queued an async task.
        let pending = app.pending_task.as_ref().expect("pending task");
        assert!(pending.desc.contains("origin/feature/foo"));
        let (_, action) = rx.try_recv().expect("dispatched action");
        match action {
            Action::DeleteRemoteBranch { remote, branch } => {
                assert_eq!(remote, "origin");
                assert_eq!(branch, "feature/foo");
            }
            _ => panic!("expected DeleteRemoteBranch action"),
        }
    }

    #[test]
    fn on_task_result_remote_delete_clears_pending_and_sets_status() {
        let mut app = app_with(vec![br("main", true)]);
        app.pending_task = Some(PendingTask {
            id: 4,
            desc: "deleting origin/foo".to_string(),
        });
        app.on_task_result(
            4,
            Ok(Outcome::RemoteBranchDeleted {
                full_name: "origin/foo".to_string(),
            }),
        );
        assert!(app.pending_task.is_none());
        assert_eq!(app.status, "deleted origin/foo");
    }

    #[test]
    fn upstream_picker_esc_cancels() {
        let mut app = app_with(vec![br("foo", true)]);
        app.remote_branches = vec![remote("origin", "feature/foo")];
        app.on_key(k(KeyCode::Char('u')));
        app.handle_upstream_picker_key(k(KeyCode::Esc));
        assert!(app.upstream_picker.is_none());
        assert_eq!(app.status, "cancelled");
    }
}
