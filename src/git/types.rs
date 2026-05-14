#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub is_current: bool,
    pub short_sha: String,
    pub subject: String,
    pub rel_date: String,
}
