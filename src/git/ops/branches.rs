use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

use crate::git::{Branch, BranchDiff, Commit, DiffLine, RemoteBranch, UpstreamTrack};

// for-each-ref output is machine-readable. Field separator is \x1f (US).
const FIELD_SEP: char = '\x1f';
// Seven fields. Subject lives last so it can contain a stray `\x1f`
// without truncating subsequent fields (splitn caps the split count).
//   HEAD · name · sha · rel-date · upstream-short · upstream-track · subject
const FORMAT: &str =
    "%(HEAD)\x1f%(refname:short)\x1f%(objectname:short)\x1f%(committerdate:relative)\x1f%(upstream:short)\x1f%(upstream:track)\x1f%(contents:subject)";
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
        let parts: Vec<&str> = line.splitn(7, FIELD_SEP).collect();
        if parts.len() < 7 {
            continue;
        }
        let upstream_track = parse_upstream_track(parts[4], parts[5]);
        branches.push(Branch {
            is_current: parts[0].trim() == "*",
            name: parts[1].to_string(),
            short_sha: parts[2].to_string(),
            rel_date: parts[3].to_string(),
            subject: parts[6].to_string(),
            is_merged: false,
            worktree_path: None,
            upstream_track,
        });
    }
    branches
}

/// Convert the `%(upstream:track)` atom into structured data.
/// `upstream` is `%(upstream:short)`; an empty value means no upstream is
/// configured.
pub(crate) fn parse_upstream_track(upstream: &str, track: &str) -> Option<UpstreamTrack> {
    if upstream.is_empty() {
        return None;
    }
    let t = track.trim();
    if t.is_empty() {
        return Some(UpstreamTrack::default());
    }
    if t == "[gone]" {
        return Some(UpstreamTrack {
            gone: true,
            ..Default::default()
        });
    }
    let inner = t.trim_start_matches('[').trim_end_matches(']');
    let mut ahead = 0u32;
    let mut behind = 0u32;
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.parse().unwrap_or(0);
        }
    }
    Some(UpstreamTrack {
        ahead,
        behind,
        gone: false,
    })
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

pub fn checkout_tracking(workdir: &Path, local: &str, remote_ref: &str) -> Result<()> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["switch", "-c", local, "--track", remote_ref])
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

pub fn branch_diff(workdir: &Path, target: &str, base: &str) -> Result<BranchDiff> {
    let ahead = log_commits(workdir, &format!("{base}..{target}"))?;
    let behind = log_commits(workdir, &format!("{target}..{base}"))?;
    let patch = run_diff_patch(workdir, target, base)?;
    Ok(BranchDiff {
        target: target.to_string(),
        base: base.to_string(),
        ahead,
        behind,
        patch,
    })
}

fn run_diff_patch(workdir: &Path, target: &str, base: &str) -> Result<Vec<DiffLine>> {
    // `base...target` shows the changes target made since diverging from
    // base — same view as PR review.
    let range = format!("{base}...{target}");
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["diff", "--no-color", &range])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git diff {range} failed: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim()
        ));
    }
    Ok(parse_diff(&String::from_utf8_lossy(&out.stdout)))
}

pub(crate) fn parse_diff(stdout: &str) -> Vec<DiffLine> {
    stdout
        .lines()
        .map(|line| {
            if line.starts_with("diff --git ") {
                DiffLine::FileHeader(line.to_string())
            } else if line.starts_with("@@") {
                DiffLine::Hunk(line.to_string())
            } else if line.starts_with("+++") || line.starts_with("---") {
                // unified-diff file header lines — meta, not add/remove
                DiffLine::Meta(line.to_string())
            } else if line.starts_with("index ")
                || line.starts_with("new file mode")
                || line.starts_with("deleted file mode")
                || line.starts_with("similarity index ")
                || line.starts_with("rename from ")
                || line.starts_with("rename to ")
                || line.starts_with("old mode")
                || line.starts_with("new mode")
                || line.starts_with("\\ No newline at end of file")
            {
                DiffLine::Meta(line.to_string())
            } else if let Some(rest) = line.strip_prefix('+') {
                DiffLine::Add(format!("+{rest}"))
            } else if let Some(rest) = line.strip_prefix('-') {
                DiffLine::Remove(format!("-{rest}"))
            } else {
                DiffLine::Context(line.to_string())
            }
        })
        .collect()
}

fn log_commits(workdir: &Path, range: &str) -> Result<Vec<Commit>> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["log", "--pretty=format:%h\x1f%s", range])
        .output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git log {range} failed: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim()
        ));
    }
    Ok(parse_commits(&String::from_utf8_lossy(&out.stdout)))
}

pub(crate) fn parse_commits(stdout: &str) -> Vec<Commit> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\x1f');
            let sha = parts.next()?.to_string();
            let subject = parts.next()?.to_string();
            if sha.is_empty() {
                return None;
            }
            Some(Commit {
                short_sha: sha,
                subject,
            })
        })
        .collect()
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

    /// Helper to build a for-each-ref-shaped line with the current 7-field
    /// format: HEAD · name · sha · rel-date · upstream · track · subject.
    fn line(
        head: &str,
        name: &str,
        sha: &str,
        rel: &str,
        upstream: &str,
        track: &str,
        subject: &str,
    ) -> String {
        format!("{head}\u{1f}{name}\u{1f}{sha}\u{1f}{rel}\u{1f}{upstream}\u{1f}{track}\u{1f}{subject}\n")
    }

    #[test]
    fn parses_current_branch_marker() {
        let bs = parse_for_each_ref(&line("*", "main", "abc1234", "2 days ago", "", "", "fix oauth"));
        assert_eq!(bs.len(), 1);
        assert!(bs[0].is_current);
        assert_eq!(bs[0].name, "main");
        assert_eq!(bs[0].short_sha, "abc1234");
        assert_eq!(bs[0].rel_date, "2 days ago");
        assert_eq!(bs[0].subject, "fix oauth");
        assert!(bs[0].upstream_track.is_none());
    }

    #[test]
    fn parses_non_current_branch() {
        let bs = parse_for_each_ref(&line(" ", "feature/foo", "def5678", "1 hour ago", "", "", "wip"));
        assert_eq!(bs.len(), 1);
        assert!(!bs[0].is_current);
        assert_eq!(bs[0].name, "feature/foo");
    }

    #[test]
    fn parses_multiple_lines_in_order() {
        let stdout = line("*", "main", "aaa", "2d", "", "", "m")
            + &line(" ", "foo", "bbb", "1h", "", "", "f");
        let bs = parse_for_each_ref(&stdout);
        assert_eq!(bs.len(), 2);
        assert_eq!(bs[0].name, "main");
        assert_eq!(bs[1].name, "foo");
    }

    #[test]
    fn skips_blank_and_malformed_lines() {
        let stdout = format!(
            "{}\n{}",
            line("*", "main", "aaa", "2d", "", "", "m").trim_end(),
            "broken"
        );
        let bs = parse_for_each_ref(&stdout);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].name, "main");
    }

    #[test]
    fn subject_can_contain_the_field_separator() {
        // splitn(7) means the 7th split absorbs any remaining `\x1f` in
        // the subject — that's why we put subject last in the format.
        let bs = parse_for_each_ref(&line("*", "main", "abc", "2d", "", "", "has\x1fseparator"));
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].subject, "has\u{1f}separator");
    }

    #[test]
    fn parses_upstream_ahead_behind() {
        let bs = parse_for_each_ref(&line(
            "*",
            "main",
            "abc",
            "2d",
            "origin/main",
            "[ahead 3, behind 1]",
            "msg",
        ));
        let t = bs[0].upstream_track.as_ref().unwrap();
        assert_eq!(t.ahead, 3);
        assert_eq!(t.behind, 1);
        assert!(!t.gone);
    }

    #[test]
    fn parses_upstream_ahead_only() {
        let bs = parse_for_each_ref(&line(
            " ",
            "feature/foo",
            "abc",
            "2d",
            "origin/feature/foo",
            "[ahead 2]",
            "msg",
        ));
        let t = bs[0].upstream_track.as_ref().unwrap();
        assert_eq!(t.ahead, 2);
        assert_eq!(t.behind, 0);
    }

    #[test]
    fn parses_upstream_gone() {
        let bs = parse_for_each_ref(&line(
            " ",
            "old-branch",
            "abc",
            "2d",
            "origin/old-branch",
            "[gone]",
            "msg",
        ));
        let t = bs[0].upstream_track.as_ref().unwrap();
        assert!(t.gone);
    }

    #[test]
    fn parses_upstream_synced_zero_track() {
        let bs = parse_for_each_ref(&line(
            "*",
            "main",
            "abc",
            "2d",
            "origin/main",
            "",
            "msg",
        ));
        let t = bs[0].upstream_track.as_ref().unwrap();
        assert_eq!(t.ahead, 0);
        assert_eq!(t.behind, 0);
        assert!(!t.gone);
    }

    #[test]
    fn no_upstream_yields_none() {
        let bs = parse_for_each_ref(&line("*", "wip", "abc", "2d", "", "", "msg"));
        assert!(bs[0].upstream_track.is_none());
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
    fn parses_commit_log_lines() {
        let stdout = "abc1234\u{1f}fix oauth\ndef5678\u{1f}wip: refactor\n";
        let commits = parse_commits(stdout);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].short_sha, "abc1234");
        assert_eq!(commits[0].subject, "fix oauth");
        assert_eq!(commits[1].short_sha, "def5678");
        assert_eq!(commits[1].subject, "wip: refactor");
    }

    #[test]
    fn diff_parser_classifies_typical_lines() {
        let stdout = "\
diff --git a/foo.rs b/foo.rs
index 1234567..abcdef0 100644
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
+    println!(\"extra\");
 }
";
        let lines = parse_diff(stdout);
        assert!(matches!(lines[0], DiffLine::FileHeader(_)));
        assert!(matches!(lines[1], DiffLine::Meta(_)));
        assert!(matches!(lines[2], DiffLine::Meta(_)));
        assert!(matches!(lines[3], DiffLine::Meta(_)));
        assert!(matches!(lines[4], DiffLine::Hunk(_)));
        assert!(matches!(lines[5], DiffLine::Context(_)));
        match &lines[6] {
            DiffLine::Remove(s) => assert!(s.starts_with("-    println!")),
            _ => panic!("expected Remove at index 6"),
        }
        match &lines[7] {
            DiffLine::Add(s) => assert!(s.starts_with("+    println!")),
            _ => panic!("expected Add at index 7"),
        }
        assert!(matches!(lines[8], DiffLine::Add(_)));
        assert!(matches!(lines[9], DiffLine::Context(_)));
    }

    #[test]
    fn diff_parser_handles_no_newline_marker() {
        let stdout = "+last\n\\ No newline at end of file\n";
        let lines = parse_diff(stdout);
        assert_eq!(lines.len(), 2);
        assert!(matches!(lines[0], DiffLine::Add(_)));
        assert!(matches!(lines[1], DiffLine::Meta(_)));
    }

    #[test]
    fn diff_parser_does_not_misclassify_triple_dash() {
        // `---` and `+++` are file headers, not Remove/Add lines.
        let stdout = "--- a/foo\n+++ b/foo\n";
        let lines = parse_diff(stdout);
        assert!(matches!(lines[0], DiffLine::Meta(_)));
        assert!(matches!(lines[1], DiffLine::Meta(_)));
    }

    #[test]
    fn parser_skips_lines_missing_the_separator() {
        let stdout = "valid\u{1f}ok\nbroken\nanother\u{1f}fine\n";
        let commits = parse_commits(stdout);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].short_sha, "valid");
        assert_eq!(commits[1].short_sha, "another");
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
