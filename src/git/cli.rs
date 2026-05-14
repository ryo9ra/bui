use std::path::PathBuf;

use anyhow::Result;

use crate::git::{Branch, Repo, ops};

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
}
