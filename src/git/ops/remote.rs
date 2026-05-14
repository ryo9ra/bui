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
        return Err(anyhow!(
            "git fetch failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}
