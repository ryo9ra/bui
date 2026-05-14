#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub is_current: bool,
    pub short_sha: String,
    pub subject: String,
    pub rel_date: String,
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
