use std::path::PathBuf;

use anyhow::Result;

use crate::git::{Branch, BranchDiff, RemoteBranch, Repo, Worktree, ops};

pub struct CliRepo {
    pub workdir: PathBuf,
}

impl CliRepo {
    pub fn new() -> Self {
        Self {
            workdir: PathBuf::from("."),
        }
    }

    pub fn at(workdir: PathBuf) -> Self {
        Self { workdir }
    }
}

impl Default for CliRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl Repo for CliRepo {
    fn list_local_branches(&self) -> Result<Vec<Branch>> {
        ops::branches::list_local(&self.workdir)
    }
    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>> {
        ops::branches::list_remote(&self.workdir)
    }
    fn checkout(&self, name: &str) -> Result<()> {
        ops::branches::checkout(&self.workdir, name)
    }
    fn create_branch(&self, name: &str, from: Option<&str>) -> Result<()> {
        ops::branches::create(&self.workdir, name, from)
    }
    fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        ops::branches::delete(&self.workdir, name, force)
    }
    fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        ops::branches::rename(&self.workdir, old, new)
    }
    fn fetch(&self, remote: Option<&str>, prune_tags: bool) -> Result<()> {
        ops::remote::fetch(&self.workdir, remote, prune_tags)
    }
    fn pull(&self) -> Result<()> {
        ops::remote::pull(&self.workdir)
    }
    fn push(&self) -> Result<()> {
        ops::remote::push(&self.workdir)
    }
    fn push_force_with_lease(&self) -> Result<()> {
        ops::remote::push_force_with_lease(&self.workdir)
    }
    fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()> {
        ops::branches::set_upstream(&self.workdir, branch, upstream)
    }
    fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<()> {
        ops::remote::delete_branch(&self.workdir, remote, branch)
    }
    fn checkout_remote_tracking(&self, local: &str, remote_ref: &str) -> Result<()> {
        ops::branches::checkout_tracking(&self.workdir, local, remote_ref)
    }
    fn list_worktrees(&self) -> Result<Vec<Worktree>> {
        ops::worktree::list(&self.workdir)
    }
    fn add_worktree(&self, path: &str, base: &str, new_branch: Option<&str>) -> Result<()> {
        ops::worktree::add(&self.workdir, path, base, new_branch)
    }
    fn remove_worktree(&self, path: &str) -> Result<()> {
        ops::worktree::remove(&self.workdir, path)
    }
    fn branch_diff(&self, target: &str, base: &str) -> Result<BranchDiff> {
        ops::branches::branch_diff(&self.workdir, target, base)
    }
}
