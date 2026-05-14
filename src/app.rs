use std::io;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::event::{Event, EventChannel};
use crate::git::{Branch, Repo};
use crate::ui;
use crate::ui::layout::LayoutSpec;

pub struct App {
    pub repo: Box<dyn Repo>,
    pub local_branches: Vec<Branch>,
    /// Index into `visible_branches()`, not `local_branches`.
    pub selected: usize,
    pub filter: String,
    pub search_active: bool,
    pub status: String,
    pub active_tab: Tab,
    pub modal: Option<Modal>,
    pub input: Option<InputState>,
    pub layout: LayoutSpec,
    pub should_quit: bool,
    pub dirty: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
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

impl App {
    pub fn new(repo: Box<dyn Repo>) -> Self {
        Self {
            repo,
            local_branches: Vec::new(),
            selected: 0,
            filter: String::new(),
            search_active: false,
            status: "ready".to_string(),
            active_tab: Tab::Local,
            modal: None,
            input: None,
            layout: LayoutSpec::v0_1_default(),
            should_quit: false,
            dirty: true,
        }
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

    fn selected_name(&self) -> Option<String> {
        self.visible_branches()
            .get(self.selected)
            .map(|b| b.name.clone())
    }

    fn selected_is_current(&self) -> bool {
        self.visible_branches()
            .get(self.selected)
            .is_some_and(|b| b.is_current)
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
        if self.search_active {
            self.handle_search_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.selected = self.visible_branches().len().saturating_sub(1);
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
            KeyCode::Char('/') if self.active_tab == Tab::Local => self.start_search(),
            KeyCode::Esc if !self.filter.is_empty() => self.clear_filter(),
            _ => {}
        }
        self.dirty = true;
    }

    fn start_search(&mut self) {
        self.filter.clear();
        self.search_active = true;
        self.selected = 0;
        self.status = "search".to_string();
    }

    fn clear_filter(&mut self) {
        self.filter.clear();
        self.selected = 0;
        self.status = "filter cleared".to_string();
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_active = false;
                self.filter.clear();
                self.selected = 0;
                self.status = "search cancelled".to_string();
            }
            KeyCode::Enter => {
                self.search_active = false;
                let total = self.local_branches.len();
                let shown = self.visible_branches().len();
                self.status = if self.filter.is_empty() {
                    format!("{total} branches")
                } else {
                    format!("{shown}/{total} matching '{}'", self.filter)
                };
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.selected = 0;
            }
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.selected = 0;
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
        // animations / spinner / time-driven redraws will live here.
    }

    fn move_down(&mut self) {
        if self.selected + 1 < self.visible_branches().len() {
            self.selected += 1;
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
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
            Event::TaskResult(_, _) => {}
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
