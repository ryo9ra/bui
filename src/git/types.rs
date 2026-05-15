#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub is_current: bool,
    pub short_sha: String,
    pub subject: String,
    pub rel_date: String,
    /// True if the branch tip is reachable from HEAD.
    pub is_merged: bool,
    /// If checked out in another worktree, that worktree's path.
    pub worktree_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RemoteBranch {
    /// Remote name, e.g. "origin".
    pub remote: String,
    /// Branch name without the remote prefix, e.g. "feature/foo".
    pub name: String,
    /// Full short refname, e.g. "origin/feature/foo".
    pub full_name: String,
    pub short_sha: String,
    pub subject: String,
    pub rel_date: String,
}
