pub mod cli;
pub mod ops;
pub mod types;

pub use types::{Branch, BranchDiff, Commit, RemoteBranch, UpstreamTrack, Worktree};

use anyhow::Result;

pub trait Repo: Send + Sync {
    fn list_local_branches(&self) -> Result<Vec<Branch>>;
    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>>;
    fn checkout(&self, name: &str) -> Result<()>;
    fn create_branch(&self, name: &str, from: Option<&str>) -> Result<()>;
    fn delete_branch(&self, name: &str, force: bool) -> Result<()>;
    fn rename_branch(&self, old: &str, new: &str) -> Result<()>;
    fn fetch(&self, remote: Option<&str>, prune_tags: bool) -> Result<()>;
    fn pull(&self) -> Result<()>;
    fn push(&self) -> Result<()>;
    fn push_force_with_lease(&self) -> Result<()>;
    fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()>;
    fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<()>;
    /// Create local branch `local` tracking `remote_ref` (e.g.
    /// `origin/feature/foo`) and switch to it atomically.
    fn checkout_remote_tracking(&self, local: &str, remote_ref: &str) -> Result<()>;
    fn list_worktrees(&self) -> Result<Vec<Worktree>>;
    /// `base` is the existing ref the worktree should start from. If
    /// `new_branch` is `Some(name)`, a new branch with that name is created
    /// off `base` (atomically, via `git worktree add -b`); otherwise `base`
    /// itself is checked out into the new worktree.
    fn add_worktree(&self, path: &str, base: &str, new_branch: Option<&str>) -> Result<()>;
    fn remove_worktree(&self, path: &str) -> Result<()>;
    /// Commits unique to each side of `target` vs `base`.
    fn branch_diff(&self, target: &str, base: &str) -> Result<BranchDiff>;
}
