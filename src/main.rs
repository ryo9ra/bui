use std::io;
use std::sync::Arc;

use anyhow::Result;
use bui::{app, event, git, task};
use bui::git::Repo;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};

fn main() -> Result<()> {
    install_panic_hook();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let repo: Arc<dyn Repo> = Arc::new(git::cli::CliRepo::new());
    let (events, _handles) = event::start_event_threads();
    let (task_tx, _worker) = task::spawn(Arc::clone(&repo), events.tx.clone());
    let mut app = app::App::new(repo, task_tx);
    app.refresh(None);
    app::run_loop(&mut app, &events, terminal)
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(info);
    }));
}
