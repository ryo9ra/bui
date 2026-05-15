use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

use crate::git::{Branch, RemoteBranch};

// for-each-ref output is machine-readable. Field separator is \x1f (US).
const FIELD_SEP: char = '\x1f';
const FORMAT: &str =
    "%(HEAD)\x1f%(refname:short)\x1f%(objectname:short)\x1f%(committerdate:relative)\x1f%(contents:subject)";
const REMOTE_FORMAT: &str =
    "%(refname:short)\x1f%(objectname:short)\x1f%(committerdate:relative)\x1f%(contents:subject)\x1f%(symref)";

pub fn list_local(workdir: &Path) -> Result<Vec<Branch>> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args([
            "for-each-ref",
            "--sort=-committerdate",
            &format!("--format={FORMAT}"),
            "refs/heads",
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git for-each-ref failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let mut branches = parse_for_each_ref(&String::from_utf8_lossy(&out.stdout));

    // Augment with merged + worktree info. Best-effort: a failure of either
    // side doesn't drop the whole listing.
    if let Ok(merged) = list_merged_into_head(workdir) {
        for b in branches.iter_mut() {
            b.is_merged = merged.contains(&b.name);
        }
    }
    if let Ok(wts) = list_worktree_branches(workdir) {
        for b in branches.iter_mut() {
            b.worktree_path = wts.get(&b.name).cloned();
        }
    }
    Ok(branches)
}

fn list_merged_into_head(workdir: &Path) -> Result<HashSet<String>> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args([
            "for-each-ref",
            "--merged=HEAD",
            "--format=%(refname:short)",
            "refs/heads",
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git for-each-ref --merged failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect())
}

fn list_worktree_branches(workdir: &Path) -> Result<HashMap<String, String>> {
    // Identify the worktree that's "ours" so we don't flag branches we're
    // currently sitting on as living in "another" worktree.
    let cur = Command::new("git")
        .current_dir(workdir)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    let cur_path = if cur.status.success() {
        String::from_utf8_lossy(&cur.stdout).trim().to_string()
    } else {
        String::new()
    };

    let out = Command::new("git")
        .current_dir(workdir)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git worktree list failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(parse_worktree_porcelain(
        &String::from_utf8_lossy(&out.stdout),
        &cur_path,
    ))
}

pub(crate) fn parse_worktree_porcelain(
    stdout: &str,
    current_path: &str,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut path: Option<String> = None;
    let mut branch: Option<String> = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            commit_record(&mut map, path.take(), branch.take(), current_path);
            continue;
        }
        if let Some(p) = trimmed.strip_prefix("worktree ") {
            path = Some(p.to_string());
        } else if let Some(refn) = trimmed.strip_prefix("branch ") {
            let name = refn.strip_prefix("refs/heads/").unwrap_or(refn);
            branch = Some(name.to_string());
        }
    }
    // Trailing record without a blank line.
    commit_record(&mut map, path, branch, current_path);
    map
}

fn commit_record(
    map: &mut HashMap<String, String>,
    path: Option<String>,
    branch: Option<String>,
    current_path: &str,
) {
    if let (Some(p), Some(b)) = (path, branch)
        && p != current_path
    {
        map.insert(b, p);
    }
}

pub(crate) fn parse_for_each_ref(stdout: &str) -> Vec<Branch> {
    let mut branches = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(5, FIELD_SEP).collect();
        if parts.len() < 5 {
            continue;
        }
        branches.push(Branch {
            is_current: parts[0].trim() == "*",
            name: parts[1].to_string(),
            short_sha: parts[2].to_string(),
            rel_date: parts[3].to_string(),
            subject: parts[4].to_string(),
            is_merged: false,
            worktree_path: None,
        });
    }
    branches
}

pub fn list_remote(workdir: &Path) -> Result<Vec<RemoteBranch>> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args([
            "for-each-ref",
            "--sort=-committerdate",
            &format!("--format={REMOTE_FORMAT}"),
            "refs/remotes",
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git for-each-ref (remote) failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(parse_remote_for_each_ref(&String::from_utf8_lossy(&out.stdout)))
}

pub(crate) fn parse_remote_for_each_ref(stdout: &str) -> Vec<RemoteBranch> {
    let mut branches = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(5, FIELD_SEP).collect();
        if parts.len() < 5 {
            continue;
        }
        // `origin/HEAD -> origin/main` style symbolic refs have a non-empty
        // %(symref); skip them so the list only carries actual branches.
        if !parts[4].is_empty() {
            continue;
        }
        let full = parts[0];
        let Some((remote, name)) = full.split_once('/') else {
            continue;
        };
        branches.push(RemoteBranch {
            remote: remote.to_string(),
            name: name.to_string(),
            full_name: full.to_string(),
            short_sha: parts[1].to_string(),
            rel_date: parts[2].to_string(),
            subject: parts[3].to_string(),
        });
    }
    branches
}

pub fn checkout(workdir: &Path, name: &str) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["switch", name])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git switch failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub fn create(workdir: &Path, name: &str, from: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.current_dir(workdir).args(["branch", name]);
    if let Some(start) = from {
        cmd.arg(start);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git branch failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub fn delete(workdir: &Path, name: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["branch", flag, name])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git branch {flag} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub fn rename(workdir: &Path, old: &str, new: &str) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["branch", "-m", old, new])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git branch -m failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub fn set_upstream(workdir: &Path, branch: &str, upstream: &str) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args([
            "branch",
            &format!("--set-upstream-to={upstream}"),
            branch,
        ])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_branch_marker() {
        let line = "*\u{1f}main\u{1f}abc1234\u{1f}2 days ago\u{1f}fix oauth\n";
        let bs = parse_for_each_ref(line);
        assert_eq!(bs.len(), 1);
        assert!(bs[0].is_current);
        assert_eq!(bs[0].name, "main");
        assert_eq!(bs[0].short_sha, "abc1234");
        assert_eq!(bs[0].rel_date, "2 days ago");
        assert_eq!(bs[0].subject, "fix oauth");
    }

    #[test]
    fn parses_non_current_branch() {
        let line = " \u{1f}feature/foo\u{1f}def5678\u{1f}1 hour ago\u{1f}wip\n";
        let bs = parse_for_each_ref(line);
        assert_eq!(bs.len(), 1);
        assert!(!bs[0].is_current);
        assert_eq!(bs[0].name, "feature/foo");
    }

    #[test]
    fn parses_multiple_lines_in_order() {
        let stdout = "*\u{1f}main\u{1f}aaa\u{1f}2d\u{1f}m\n \u{1f}foo\u{1f}bbb\u{1f}1h\u{1f}f\n";
        let bs = parse_for_each_ref(stdout);
        assert_eq!(bs.len(), 2);
        assert_eq!(bs[0].name, "main");
        assert_eq!(bs[1].name, "foo");
    }

    #[test]
    fn skips_blank_and_malformed_lines() {
        let stdout = "*\u{1f}main\u{1f}aaa\u{1f}2d\u{1f}m\n\n \u{1f}foo\u{1f}bbb\u{1f}\n";
        let bs = parse_for_each_ref(stdout);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].name, "main");
    }

    #[test]
    fn subject_can_contain_the_field_separator() {
        let line = "*\u{1f}main\u{1f}abc\u{1f}2d\u{1f}has\u{1f}separator\n";
        let bs = parse_for_each_ref(line);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].subject, "has\u{1f}separator");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(parse_for_each_ref("").is_empty());
    }

    #[test]
    fn parses_remote_branch_line() {
        let line = "origin/feature/foo\u{1f}abc1234\u{1f}1 hour ago\u{1f}wip\u{1f}\n";
        let bs = parse_remote_for_each_ref(line);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].remote, "origin");
        assert_eq!(bs[0].name, "feature/foo");
        assert_eq!(bs[0].full_name, "origin/feature/foo");
        assert_eq!(bs[0].short_sha, "abc1234");
        assert_eq!(bs[0].rel_date, "1 hour ago");
        assert_eq!(bs[0].subject, "wip");
    }

    #[test]
    fn parses_remote_branch_skips_symbolic_head() {
        // origin/HEAD has a non-empty %(symref) pointing at the target.
        let stdout = "\
origin/HEAD\u{1f}abc\u{1f}2d\u{1f}m\u{1f}refs/remotes/origin/main
origin/main\u{1f}abc\u{1f}2d\u{1f}m\u{1f}
";
        let bs = parse_remote_for_each_ref(stdout);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].full_name, "origin/main");
    }

    #[test]
    fn parses_remote_branch_with_nested_name() {
        let line = "upstream/release/2024-q4\u{1f}abc\u{1f}1d\u{1f}cut\u{1f}\n";
        let bs = parse_remote_for_each_ref(line);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].remote, "upstream");
        assert_eq!(bs[0].name, "release/2024-q4");
    }

    #[test]
    fn worktree_parser_extracts_branches_for_other_worktrees() {
        let porcelain = "\
worktree /repo/main
HEAD aaa
branch refs/heads/main

worktree /repo/wt-feature
HEAD bbb
branch refs/heads/feature/foo

worktree /repo/wt-detached
HEAD ccc
detached
";
        let map = parse_worktree_porcelain(porcelain, "/repo/main");
        // Current worktree filtered out, detached records have no branch.
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("feature/foo"),
            Some(&"/repo/wt-feature".to_string())
        );
        assert!(!map.contains_key("main"));
    }

    #[test]
    fn worktree_parser_handles_trailing_record_without_blank_line() {
        let porcelain = "\
worktree /repo/main
HEAD aaa
branch refs/heads/main
worktree /repo/wt
HEAD bbb
branch refs/heads/foo";
        let map = parse_worktree_porcelain(porcelain, "/repo/main");
        assert_eq!(map.get("foo"), Some(&"/repo/wt".to_string()));
    }

    #[test]
    fn worktree_parser_returns_empty_when_only_current_worktree() {
        let porcelain = "\
worktree /repo/main
HEAD aaa
branch refs/heads/main
";
        let map = parse_worktree_porcelain(porcelain, "/repo/main");
        assert!(map.is_empty());
    }
}
