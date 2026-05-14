use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

pub fn fetch(workdir: &Path, remote: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.current_dir(workdir).args(["fetch", "--prune"]);
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
    Err(anyhow!("{}", first_useful_line(stderr.as_bytes())))
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
