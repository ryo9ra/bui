use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

use crate::git::Worktree;

pub fn list(workdir: &Path) -> Result<Vec<Worktree>> {
    let cur = Command::new("git")
        .current_dir(workdir)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    let current_path = if cur.status.success() {
        String::from_utf8_lossy(&cur.stdout).trim().to_string()
    } else {
        String::new()
    };
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", first_useful_line(&out.stderr)));
    }
    Ok(parse_porcelain(
        &String::from_utf8_lossy(&out.stdout),
        &current_path,
    ))
}

pub fn add(workdir: &Path, path: &str, base: &str, new_branch: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.current_dir(workdir).args(["worktree", "add"]);
    if let Some(name) = new_branch {
        cmd.args(["-b", name]);
    }
    cmd.args([path, base]);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", first_useful_line(&out.stderr)));
    }
    Ok(())
}

pub fn remove(workdir: &Path, path: &str) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["worktree", "remove", path])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", first_useful_line(&out.stderr)));
    }
    Ok(())
}

pub(crate) fn parse_porcelain(stdout: &str, current_path: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    let mut path: Option<String> = None;
    let mut head: Option<String> = None;
    let mut branch: Option<String> = None;

    for line in stdout.lines() {
        let t = line.trim();
        if t.is_empty() {
            commit_record(&mut out, &mut path, &mut head, &mut branch, current_path);
            continue;
        }
        if let Some(p) = t.strip_prefix("worktree ") {
            path = Some(p.to_string());
        } else if let Some(h) = t.strip_prefix("HEAD ") {
            head = Some(h.to_string());
        } else if let Some(refn) = t.strip_prefix("branch ") {
            branch = Some(
                refn.strip_prefix("refs/heads/")
                    .unwrap_or(refn)
                    .to_string(),
            );
        }
        // `detached` lines and other markers are ignored — branch stays None.
    }
    commit_record(&mut out, &mut path, &mut head, &mut branch, current_path);
    out
}

fn commit_record(
    out: &mut Vec<Worktree>,
    path: &mut Option<String>,
    head: &mut Option<String>,
    branch: &mut Option<String>,
    current_path: &str,
) {
    if let Some(p) = path.take() {
        let is_current = p == current_path;
        out.push(Worktree {
            path: p,
            head: head.take().unwrap_or_default(),
            branch: branch.take(),
            is_current,
        });
    } else {
        head.take();
        branch.take();
    }
}

fn first_useful_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_worktrees_with_current_flag() {
        let porcelain = "\
worktree /repo/main
HEAD aaa
branch refs/heads/main

worktree /repo/wt-foo
HEAD bbb
branch refs/heads/feature/foo

worktree /repo/wt-detached
HEAD ccc
detached
";
        let wts = parse_porcelain(porcelain, "/repo/main");
        assert_eq!(wts.len(), 3);
        assert!(wts[0].is_current);
        assert_eq!(wts[0].branch.as_deref(), Some("main"));
        assert!(!wts[1].is_current);
        assert_eq!(wts[1].branch.as_deref(), Some("feature/foo"));
        assert_eq!(wts[1].path, "/repo/wt-foo");
        assert!(wts[2].branch.is_none());
        assert_eq!(wts[2].head, "ccc");
    }

    #[test]
    fn empty_porcelain_yields_empty_vec() {
        assert!(parse_porcelain("", "/repo/main").is_empty());
    }

    #[test]
    fn handles_trailing_record_without_blank_line() {
        let porcelain = "\
worktree /repo/main
HEAD aaa
branch refs/heads/main";
        let wts = parse_porcelain(porcelain, "/repo/main");
        assert_eq!(wts.len(), 1);
        assert!(wts[0].is_current);
    }
}
