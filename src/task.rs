//! Background worker that runs slow git operations off the UI thread.
//!
//! The worker owns a clone of the `Repo` (`Arc<dyn Repo>`), drains an
//! `Action` channel one at a time, and pushes results back into the main
//! `Event` channel as `Event::TaskResult(id, outcome)`. The main thread is
//! responsible for assigning IDs and tracking pending state — the worker
//! itself is deliberately dumb.

use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::thread::{self, JoinHandle};

use crate::event::{Event, Outcome, TaskId};
use crate::git::Repo;

pub enum Action {
    Fetch { remote: Option<String> },
    Pull,
    Push,
}

pub fn spawn(
    repo: Arc<dyn Repo>,
    event_tx: Sender<Event>,
) -> (Sender<(TaskId, Action)>, JoinHandle<()>) {
    let (tx, rx) = channel::<(TaskId, Action)>();
    let thread = thread::spawn(move || {
        for (id, action) in rx.iter() {
            let outcome = execute(repo.as_ref(), action);
            if event_tx.send(Event::TaskResult(id, outcome)).is_err() {
                return;
            }
        }
    });
    (tx, thread)
}

fn execute(repo: &dyn Repo, action: Action) -> Result<Outcome, String> {
    match action {
        Action::Fetch { remote } => repo
            .fetch(remote.as_deref())
            .map(|_| Outcome::Fetched)
            .map_err(|e| e.to_string()),
        Action::Pull => repo.pull().map(|_| Outcome::Pulled).map_err(|e| e.to_string()),
        Action::Push => repo.push().map(|_| Outcome::Pushed).map_err(|e| e.to_string()),
    }
}
