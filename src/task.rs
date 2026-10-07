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
    Fetch {
        remote: Option<String>,
        prune_tags: bool,
    },
    /// `fetch --all --prune` as the first half of clean-gone (`X`). The
    /// App collects `[gone]` branches from the refreshed list afterwards.
    FetchForCleanGone {
        prune_tags: bool,
    },
    Pull,
    Push,
    PushForceWithLease,
    DeleteRemoteBranch {
        remote: String,
        branch: String,
    },
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
        Action::Fetch { remote, prune_tags } => repo
            .fetch(remote.as_deref(), prune_tags)
            .map(|_| Outcome::Fetched)
            .map_err(|e| e.to_string()),
        Action::FetchForCleanGone { prune_tags } => repo
            .fetch(None, prune_tags)
            .map(|_| Outcome::FetchedForCleanGone)
            .map_err(|e| e.to_string()),
        Action::Pull => repo
            .pull()
            .map(|_| Outcome::Pulled)
            .map_err(|e| e.to_string()),
        Action::Push => repo
            .push()
            .map(|_| Outcome::Pushed)
            .map_err(|e| e.to_string()),
        Action::PushForceWithLease => repo
            .push_force_with_lease()
            .map(|_| Outcome::Pushed)
            .map_err(|e| e.to_string()),
        Action::DeleteRemoteBranch { remote, branch } => repo
            .delete_remote_branch(&remote, &branch)
            .map(|_| Outcome::RemoteBranchDeleted {
                full_name: format!("{remote}/{branch}"),
            })
            .map_err(|e| e.to_string()),
    }
}
