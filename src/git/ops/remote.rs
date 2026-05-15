use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

pub fn fetch(workdir: &Path, remote: Option<&str>, prune_tags: bool) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.current_dir(workdir).args(["fetch", "--prune"]);
    if prune_tags {
        cmd.arg("--prune-tags");
    }
    match remote {
        Some(r) => {
            cmd.arg(r);
        }
        None => {
            cmd.arg("--all");
        }
    }
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", first_useful_line(&out.stderr)));
    }
    Ok(())
}

pub fn pull(workdir: &Path) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["pull"])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = if stderr.contains("no tracking information") {
            // The default git message is helpful but too long for the
            // status bar. Point the user at the upstream picker.
            "no upstream — press u to set one".to_string()
        } else {
            first_useful_line(&out.stderr)
        };
        return Err(anyhow!("{msg}"));
    }
    Ok(())
}

pub fn delete_branch(workdir: &Path, remote: &str, branch: &str) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["push", remote, "--delete", branch])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", first_useful_line(&out.stderr)));
    }
    Ok(())
}

pub fn push(workdir: &Path) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["push"])
        .output()?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    // First push of a fresh branch: auto-retry with --set-upstream so the
    // user doesn't have to think about it. Matches lazygit's behaviour.
    if stderr.contains("has no upstream branch") {
        let retry = Command::new("git")
            .current_dir(workdir)
            .args(["push", "-u", "origin", "HEAD"])
            .output()?;
        if retry.status.success() {
            return Ok(());
        }
        return Err(anyhow!("{}", first_useful_line(&retry.stderr)));
    }
    // Diverged history: bui can offer a force-with-lease retry, so emit a
    // stable marker the App can match against in on_task_result.
    if stderr.contains("non-fast-forward") || stderr.contains("[rejected]") {
        return Err(anyhow!(
            "non-fast-forward: remote has diverged"
        ));
    }
    Err(anyhow!("{}", first_useful_line(stderr.as_bytes())))
}

pub fn push_force_with_lease(workdir: &Path) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["push", "--force-with-lease"])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", first_useful_line(&out.stderr)));
    }
    Ok(())
}

/// First non-blank line of `stderr`. Status-bar friendly; the rest gets
/// truncated anyway.
fn first_useful_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}
