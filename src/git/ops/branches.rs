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
    let mut branches = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
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
    Ok(branches)
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
