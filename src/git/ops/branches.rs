use std::process::Command;

use anyhow::{Result, anyhow};

use crate::git::Branch;

// for-each-ref output is machine-readable. Field separator is \x1f (US).
const FIELD_SEP: char = '\x1f';
const FORMAT: &str =
    "%(HEAD)\x1f%(refname:short)\x1f%(objectname:short)\x1f%(committerdate:relative)\x1f%(contents:subject)";

pub fn list_local() -> Result<Vec<Branch>> {
    let out = Command::new("git")
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
    Ok(parse_for_each_ref(&String::from_utf8_lossy(&out.stdout)))
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
        });
    }
    branches
}

#[allow(dead_code)]
pub fn checkout(name: &str) -> Result<()> {
    let out = Command::new("git").args(["switch", name]).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git switch failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn create(name: &str, from: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(["branch", name]);
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

#[allow(dead_code)]
pub fn delete(name: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    let out = Command::new("git")
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

#[allow(dead_code)]
pub fn rename(old: &str, new: &str) -> Result<()> {
    let out = Command::new("git")
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
        // 4-field line is malformed; blank line is skipped.
        let stdout = "*\u{1f}main\u{1f}aaa\u{1f}2d\u{1f}m\n\n \u{1f}foo\u{1f}bbb\u{1f}\n";
        let bs = parse_for_each_ref(stdout);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].name, "main");
    }

    #[test]
    fn subject_can_contain_the_field_separator() {
        // splitn(5, …) means the last field captures any extra separators.
        let line = "*\u{1f}main\u{1f}abc\u{1f}2d\u{1f}has\u{1f}separator\n";
        let bs = parse_for_each_ref(line);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].subject, "has\u{1f}separator");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(parse_for_each_ref("").is_empty());
    }
}
