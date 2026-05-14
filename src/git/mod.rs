pub mod cli;
pub mod ops;
pub mod types;

pub use types::{Branch, RemoteBranch};

use anyhow::Result;

pub trait Repo: Send + Sync {
    fn list_local_branches(&self) -> Result<Vec<Branch>>;
    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>>;
    fn checkout(&self, name: &str) -> Result<()>;
    fn create_branch(&self, name: &str, from: Option<&str>) -> Result<()>;
    fn delete_branch(&self, name: &str, force: bool) -> Result<()>;
    fn rename_branch(&self, old: &str, new: &str) -> Result<()>;
}
