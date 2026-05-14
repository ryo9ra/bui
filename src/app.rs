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
    pub selected: usize,
    pub status: String,
    pub active_tab: Tab,
    pub modal: Option<Modal>,
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

impl App {
    pub fn new(repo: Box<dyn Repo>) -> Self {
        Self {
            repo,
            local_branches: Vec::new(),
            selected: 0,
            status: "ready".to_string(),
            active_tab: Tab::Local,
            modal: None,
            layout: LayoutSpec::v0_1_default(),
            should_quit: false,
            dirty: true,
        }
    }

    pub fn initial_refresh(&mut self) {
        match self.repo.list_local_branches() {
            Ok(bs) => {
                self.status = format!("{} branches", bs.len());
                self.local_branches = bs;
            }
            Err(e) => self.status = format!("error: {e}"),
        }
        self.dirty = true;
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.selected = self.local_branches.len().saturating_sub(1);
            }
            KeyCode::Tab => self.cycle_tab(true),
            KeyCode::BackTab => self.cycle_tab(false),
            KeyCode::Char('R') => self.initial_refresh(),
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

    pub fn on_tick(&mut self) {
        // animations / spinner / time-driven redraws will live here.
    }

    fn move_down(&mut self) {
        if self.selected + 1 < self.local_branches.len() {
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
