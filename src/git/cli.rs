use anyhow::Result;

use crate::git::{Branch, Repo, ops};

pub struct CliRepo;

impl CliRepo {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CliRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl Repo for CliRepo {
    fn list_local_branches(&self) -> Result<Vec<Branch>> {
        ops::branches::list_local()
    }
    fn checkout(&self, name: &str) -> Result<()> {
        ops::branches::checkout(name)
    }
    fn create_branch(&self, name: &str, from: Option<&str>) -> Result<()> {
        ops::branches::create(name, from)
    }
    fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        ops::branches::delete(name, force)
    }
    fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        ops::branches::rename(old, new)
    }
}
