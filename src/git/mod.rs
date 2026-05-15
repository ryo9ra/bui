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
    fn fetch(&self, remote: Option<&str>) -> Result<()>;
    fn pull(&self) -> Result<()>;
    fn push(&self) -> Result<()>;
    fn push_force_with_lease(&self) -> Result<()>;
    fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()>;
    fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<()>;
}
