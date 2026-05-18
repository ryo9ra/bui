//! Integration tests for `bui::git::cli::CliRepo` against a real `git`
//! binary in a throwaway repository.

use std::path::Path;
use std::process::Command;

use bui::git::Repo;
use bui::git::cli::CliRepo;
use tempfile::TempDir;

fn run_git(workdir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Initialise a temp git repo with one commit on `main`.
fn init_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path();
    run_git(path, &["init", "-b", "main", "-q"]);
    run_git(path, &["config", "user.name", "bui-test"]);
    run_git(path, &["config", "user.email", "bui-test@example.com"]);
    run_git(path, &["config", "commit.gpgsign", "false"]);
    std::fs::write(path.join("README"), "test\n").unwrap();
    run_git(path, &["add", "."]);
    run_git(path, &["commit", "-m", "initial", "-q"]);
    dir
}

fn commit_on(dir: &Path, file: &str, msg: &str) {
    std::fs::write(dir.join(file), "data\n").unwrap();
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-m", msg, "-q"]);
}

fn open(dir: &TempDir) -> CliRepo {
    CliRepo::at(dir.path().to_path_buf())
}

#[test]
fn lists_only_the_initial_branch() {
    let dir = init_repo();
    let repo = open(&dir);
    let branches = repo.list_local_branches().unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "main");
    assert!(branches[0].is_current);
    assert!(!branches[0].short_sha.is_empty());
    assert_eq!(branches[0].subject, "initial");
    // HEAD is trivially reachable from HEAD so "main" is flagged merged.
    assert!(branches[0].is_merged);
    assert!(branches[0].worktree_path.is_none());
}

#[test]
fn marks_merged_branches_reachable_from_head() {
    let dir = init_repo();
    let repo = open(&dir);
    // `merged-topic` was created from main and never advanced — its tip
    // equals main, so it's trivially reachable from HEAD.
    repo.create_branch("merged-topic", None).unwrap();

    // `divergent` gets its own commit so its tip is no longer reachable
    // from main's HEAD.
    repo.create_branch("divergent", None).unwrap();
    repo.checkout("divergent").unwrap();
    commit_on(dir.path(), "extra", "diverge");
    repo.checkout("main").unwrap();

    let branches = repo.list_local_branches().unwrap();
    let merged: Vec<&str> = branches
        .iter()
        .filter(|b| b.is_merged)
        .map(|b| b.name.as_str())
        .collect();
    assert!(merged.contains(&"merged-topic"), "merged set: {merged:?}");
    assert!(!merged.contains(&"divergent"), "merged set: {merged:?}");
}

#[test]
fn marks_branches_checked_out_in_other_worktrees() {
    let dir = init_repo();
    let repo = open(&dir);
    repo.create_branch("feature/wt", None).unwrap();

    // Park `feature/wt` in a second worktree.
    let wt_dir = tempfile::tempdir().unwrap();
    let out = Command::new("git")
        .current_dir(dir.path())
        .args([
            "worktree",
            "add",
            wt_dir.path().to_str().unwrap(),
            "feature/wt",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let branches = repo.list_local_branches().unwrap();
    let wt_branch = branches
        .iter()
        .find(|b| b.name == "feature/wt")
        .expect("feature/wt should be listed");
    let path = wt_branch
        .worktree_path
        .as_ref()
        .expect("worktree path should be set");
    // tempfile paths can be symlinked (e.g. /var/folders/... → /private/var/...)
    // on macOS; checking the suffix avoids that resolution mismatch.
    let wt_basename = wt_dir.path().file_name().unwrap().to_str().unwrap();
    assert!(
        path.contains(wt_basename),
        "worktree path {path} should contain {wt_basename}"
    );

    // The current worktree's branch (main) should not be flagged.
    let main = branches
        .iter()
        .find(|b| b.name == "main")
        .expect("main should be listed");
    assert!(main.worktree_path.is_none());
}

#[test]
fn create_and_list_new_branch() {
    let dir = init_repo();
    let repo = open(&dir);
    repo.create_branch("feature/foo", None).unwrap();
    let names: Vec<_> = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(names.contains(&"main".to_string()));
    assert!(names.contains(&"feature/foo".to_string()));
}

#[test]
fn checkout_changes_the_current_branch() {
    let dir = init_repo();
    let repo = open(&dir);
    repo.create_branch("feature/foo", None).unwrap();
    repo.checkout("feature/foo").unwrap();
    let current = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .find(|b| b.is_current)
        .expect("a current branch");
    assert_eq!(current.name, "feature/foo");
}

#[test]
fn rename_moves_the_branch_name() {
    let dir = init_repo();
    let repo = open(&dir);
    repo.create_branch("feature/foo", None).unwrap();
    repo.rename_branch("feature/foo", "feature/bar").unwrap();
    let names: Vec<_> = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(names.contains(&"feature/bar".to_string()));
    assert!(!names.contains(&"feature/foo".to_string()));
}

#[test]
fn delete_removes_merged_branch() {
    let dir = init_repo();
    let repo = open(&dir);
    repo.create_branch("topic", None).unwrap();
    repo.delete_branch("topic", false).unwrap();
    let names: Vec<_> = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(!names.contains(&"topic".to_string()));
}

#[test]
fn delete_unmerged_branch_requires_force() {
    let dir = init_repo();
    let repo = open(&dir);
    repo.create_branch("topic", None).unwrap();
    repo.checkout("topic").unwrap();
    commit_on(dir.path(), "extra", "diverge");
    repo.checkout("main").unwrap();

    // -d should refuse the unmerged branch.
    let soft = repo.delete_branch("topic", false);
    assert!(soft.is_err(), "expected -d to refuse unmerged branch");

    // -D should succeed.
    repo.delete_branch("topic", true).unwrap();
    let names: Vec<_> = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(!names.contains(&"topic".to_string()));
}

#[test]
fn list_returns_committerdate_descending() {
    let dir = init_repo();
    let repo = open(&dir);
    // Two branches, each with a unique commit; the more recent should sort
    // first.
    repo.create_branch("older", None).unwrap();
    repo.create_branch("newer", None).unwrap();
    repo.checkout("older").unwrap();
    commit_on(dir.path(), "a", "first");
    repo.checkout("newer").unwrap();
    commit_on(dir.path(), "b", "second");

    let names: Vec<_> = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    let i_newer = names.iter().position(|n| n == "newer").unwrap();
    let i_older = names.iter().position(|n| n == "older").unwrap();
    assert!(
        i_newer < i_older,
        "newer should sort before older: {names:?}"
    );
}

#[test]
fn lists_remote_branches_after_clone() {
    let upstream = init_repo();
    // Add a second branch in the upstream so the clone sees more than just
    // origin/main.
    run_git(upstream.path(), &["branch", "feature"]);

    let clone_dir = tempfile::tempdir().expect("create clone tempdir");
    let out = Command::new("git")
        .args([
            "clone",
            "-q",
            upstream.path().to_str().unwrap(),
            clone_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("spawn git clone");
    assert!(
        out.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repo = CliRepo::at(clone_dir.path().to_path_buf());
    let remotes = repo.list_remote_branches().unwrap();
    let full_names: Vec<_> = remotes.iter().map(|b| b.full_name.clone()).collect();
    assert!(full_names.contains(&"origin/main".to_string()));
    assert!(full_names.contains(&"origin/feature".to_string()));
    // origin/HEAD is a symbolic ref and should be filtered out.
    assert!(!full_names.contains(&"origin/HEAD".to_string()));

    // Remote and name should be split correctly.
    let feature = remotes
        .iter()
        .find(|b| b.full_name == "origin/feature")
        .unwrap();
    assert_eq!(feature.remote, "origin");
    assert_eq!(feature.name, "feature");
}

#[test]
fn lists_remote_branches_empty_for_repo_without_remotes() {
    let dir = init_repo();
    let repo = open(&dir);
    let remotes = repo.list_remote_branches().unwrap();
    assert!(remotes.is_empty());
}

/// Set up a bare upstream + a working clone with an initial commit pushed
/// up. Returns (work, bare-upstream). Pull / push tests use this so the
/// remote actually accepts pushes.
fn init_work_and_bare_upstream() -> (TempDir, TempDir) {
    let upstream = tempfile::tempdir().expect("upstream tempdir");
    run_git(upstream.path(), &["init", "--bare", "-b", "main", "-q"]);

    let work = tempfile::tempdir().expect("work tempdir");
    let out = Command::new("git")
        .args([
            "clone",
            "-q",
            upstream.path().to_str().unwrap(),
            work.path().to_str().unwrap(),
        ])
        .output()
        .expect("spawn git clone");
    assert!(
        out.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    run_git(work.path(), &["config", "user.name", "bui-test"]);
    run_git(
        work.path(),
        &["config", "user.email", "bui-test@example.com"],
    );
    run_git(work.path(), &["config", "commit.gpgsign", "false"]);
    std::fs::write(work.path().join("README"), "test\n").unwrap();
    run_git(work.path(), &["add", "."]);
    run_git(work.path(), &["commit", "-m", "initial", "-q"]);
    run_git(work.path(), &["push", "-u", "origin", "main", "-q"]);
    (work, upstream)
}

fn rev_parse(workdir: &Path, refname: &str) -> String {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["rev-parse", refname])
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn push_reports_non_fast_forward_after_amend() {
    let (work, _upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    // Rewrite history so HEAD is no longer a fast-forward of origin/main.
    std::fs::write(work.path().join("README"), "amended\n").unwrap();
    run_git(work.path(), &["add", "."]);
    run_git(work.path(), &["commit", "--amend", "--no-edit", "-q"]);

    let err = repo
        .push()
        .expect_err("push should fail after history rewrite");
    let msg = format!("{err}");
    assert!(
        msg.contains("non-fast-forward"),
        "expected the non-fast-forward marker, got: {msg}"
    );
}

#[test]
fn force_with_lease_succeeds_after_local_history_rewrite() {
    let (work, upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    let original_sha = rev_parse(upstream.path(), "main");
    std::fs::write(work.path().join("README"), "amended\n").unwrap();
    run_git(work.path(), &["add", "."]);
    run_git(work.path(), &["commit", "--amend", "--no-edit", "-q"]);

    repo.push_force_with_lease().unwrap();

    let new_sha = rev_parse(upstream.path(), "main");
    assert_ne!(new_sha, original_sha);
    assert_eq!(new_sha, rev_parse(work.path(), "HEAD"));
}

#[test]
fn push_sets_upstream_automatically_for_new_branch() {
    let (work, upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    // A fresh branch with no upstream. Plain `git push` would fail with
    // "has no upstream branch"; bui should retry with --set-upstream.
    run_git(work.path(), &["checkout", "-q", "-b", "feature/new"]);
    commit_on(work.path(), "marker", "on feature");
    repo.push().unwrap();

    // The branch should now exist on the bare upstream.
    let upstream_branches: Vec<String> = String::from_utf8(
        Command::new("git")
            .current_dir(upstream.path())
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .lines()
    .map(|s| s.to_string())
    .collect();
    assert!(upstream_branches.contains(&"feature/new".to_string()));

    // Upstream tracking should be configured on the working copy.
    let upstream_cfg = Command::new("git")
        .current_dir(work.path())
        .args(["rev-parse", "--abbrev-ref", "feature/new@{upstream}"])
        .output()
        .unwrap();
    assert!(upstream_cfg.status.success());
    let upstream_name = String::from_utf8(upstream_cfg.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(upstream_name, "origin/feature/new");
}

#[test]
fn push_rebinds_tracking_when_upstream_name_mismatches() {
    let (work, upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    // Create a new local branch but leave its upstream pointing at
    // origin/main (this is what `git checkout -b temp1` inherits when the
    // local tracking config is set up that way). Plain `git push` would
    // fail here with "The upstream branch of your current branch does not
    // match the name of your current branch."
    run_git(work.path(), &["checkout", "-q", "-b", "temp1"]);
    run_git(
        work.path(),
        &["branch", "--set-upstream-to=origin/main", "temp1"],
    );
    commit_on(work.path(), "marker", "on temp1");

    repo.push().unwrap();

    let upstream_branches: Vec<String> = String::from_utf8(
        Command::new("git")
            .current_dir(upstream.path())
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .lines()
    .map(|s| s.to_string())
    .collect();
    assert!(
        upstream_branches.contains(&"temp1".to_string()),
        "expected origin/temp1 to exist after push, got: {upstream_branches:?}"
    );

    let upstream_cfg = Command::new("git")
        .current_dir(work.path())
        .args(["rev-parse", "--abbrev-ref", "temp1@{upstream}"])
        .output()
        .unwrap();
    assert!(upstream_cfg.status.success());
    let upstream_name = String::from_utf8(upstream_cfg.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(upstream_name, "origin/temp1");
}

#[test]
fn list_local_reports_ahead_behind_against_upstream() {
    let (work, _upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    // Make local main diverge: 2 commits locally, no push.
    commit_on(work.path(), "a", "ahead-1");
    commit_on(work.path(), "b", "ahead-2");

    let bs = repo.list_local_branches().unwrap();
    let main = bs.iter().find(|b| b.name == "main").unwrap();
    let track = main
        .upstream_track
        .as_ref()
        .expect("main should have upstream from clone");
    assert_eq!(track.ahead, 2);
    assert_eq!(track.behind, 0);
    assert!(!track.gone);
}

#[test]
fn list_local_reports_no_upstream_for_brand_new_branch() {
    let dir = init_repo();
    let repo = open(&dir);
    repo.create_branch("local-only", None).unwrap();
    let bs = repo.list_local_branches().unwrap();
    let local_only = bs.iter().find(|b| b.name == "local-only").unwrap();
    assert!(local_only.upstream_track.is_none());
}

#[test]
fn branch_diff_reports_ahead_and_behind_commits() {
    let dir = init_repo();
    let repo = open(&dir);

    // Build two branches that diverge from the initial commit.
    repo.create_branch("feature/foo", None).unwrap();
    repo.checkout("feature/foo").unwrap();
    commit_on(dir.path(), "feature1", "feature commit one");
    commit_on(dir.path(), "feature2", "feature commit two");
    repo.checkout("main").unwrap();
    commit_on(dir.path(), "main-only", "main moved on");

    let diff = repo.branch_diff("feature/foo", "main").unwrap();
    assert_eq!(diff.target, "feature/foo");
    assert_eq!(diff.base, "main");
    assert_eq!(diff.ahead.len(), 2);
    assert_eq!(diff.ahead[0].subject, "feature commit two");
    assert_eq!(diff.ahead[1].subject, "feature commit one");
    assert_eq!(diff.behind.len(), 1);
    assert_eq!(diff.behind[0].subject, "main moved on");
}

#[test]
fn branch_diff_populates_patch_for_diverged_branches() {
    use bui::git::DiffLine;
    let dir = init_repo();
    let repo = open(&dir);

    repo.create_branch("feature/foo", None).unwrap();
    repo.checkout("feature/foo").unwrap();
    std::fs::write(dir.path().join("foo.txt"), "added by feature\n").unwrap();
    run_git(dir.path(), &["add", "."]);
    run_git(dir.path(), &["commit", "-m", "feature change", "-q"]);
    repo.checkout("main").unwrap();

    let diff = repo.branch_diff("feature/foo", "main").unwrap();
    assert!(!diff.patch.is_empty(), "patch should have content");
    let kinds: Vec<&str> = diff
        .patch
        .iter()
        .map(|l| match l {
            DiffLine::FileHeader(_) => "file",
            DiffLine::Hunk(_) => "hunk",
            DiffLine::Add(_) => "add",
            DiffLine::Remove(_) => "remove",
            DiffLine::Context(_) => "ctx",
            DiffLine::Meta(_) => "meta",
        })
        .collect();
    assert!(kinds.contains(&"file"), "kinds: {kinds:?}");
    assert!(kinds.contains(&"hunk"), "kinds: {kinds:?}");
    assert!(kinds.contains(&"add"), "kinds: {kinds:?}");
}

#[test]
fn branch_diff_against_same_branch_is_empty() {
    let dir = init_repo();
    let repo = open(&dir);
    let diff = repo.branch_diff("main", "main").unwrap();
    assert!(diff.ahead.is_empty());
    assert!(diff.behind.is_empty());
    assert!(diff.patch.is_empty());
}

#[test]
fn worktree_add_creates_new_branch_off_base() {
    let dir = init_repo();
    let repo = open(&dir);
    let wt_dir = tempfile::tempdir().unwrap();

    // Step 1 of the bui flow: a new branch off main. The trait call
    // mirrors what App::do_add_worktree dispatches.
    repo.add_worktree(
        wt_dir.path().to_str().unwrap(),
        "main",
        Some("feature/from-base"),
    )
    .unwrap();

    // Both the branch and the worktree should exist.
    let branches: Vec<_> = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(branches.contains(&"feature/from-base".to_string()));

    let wts = repo.list_worktrees().unwrap();
    let added = wts
        .iter()
        .find(|w| w.branch.as_deref() == Some("feature/from-base"))
        .expect("new-branch worktree should be listed");
    let wt_basename = wt_dir.path().file_name().unwrap().to_str().unwrap();
    assert!(added.path.contains(wt_basename));
}

#[test]
fn worktree_add_list_remove_roundtrip() {
    let dir = init_repo();
    let repo = open(&dir);

    // Initial state: only the main worktree (current).
    let before = repo.list_worktrees().unwrap();
    assert_eq!(before.len(), 1);
    assert!(before[0].is_current);
    assert_eq!(before[0].branch.as_deref(), Some("main"));

    // Add a branch + worktree pointing at it.
    repo.create_branch("feature/wt", None).unwrap();
    let wt_dir = tempfile::tempdir().unwrap();
    repo.add_worktree(wt_dir.path().to_str().unwrap(), "feature/wt", None)
        .unwrap();

    let after_add = repo.list_worktrees().unwrap();
    assert_eq!(after_add.len(), 2);
    let added = after_add
        .iter()
        .find(|w| w.branch.as_deref() == Some("feature/wt"))
        .expect("added worktree should be listed");
    assert!(!added.is_current);
    // tempfile paths can be symlinked on macOS; check basename.
    let wt_basename = wt_dir.path().file_name().unwrap().to_str().unwrap();
    assert!(added.path.contains(wt_basename));

    // Remove and verify it's gone.
    repo.remove_worktree(&added.path).unwrap();
    let after_remove = repo.list_worktrees().unwrap();
    assert_eq!(after_remove.len(), 1);
    assert!(
        !after_remove
            .iter()
            .any(|w| w.branch.as_deref() == Some("feature/wt"))
    );
}

#[test]
fn checkout_remote_tracking_creates_local_branch_with_upstream() {
    let (work, upstream) = init_work_and_bare_upstream();

    // A second worker pushes a brand new branch to the bare upstream.
    let other = tempfile::tempdir().expect("other tempdir");
    let out = Command::new("git")
        .args([
            "clone",
            "-q",
            upstream.path().to_str().unwrap(),
            other.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    run_git(other.path(), &["config", "user.name", "bui-test"]);
    run_git(
        other.path(),
        &["config", "user.email", "bui-test@example.com"],
    );
    run_git(other.path(), &["config", "commit.gpgsign", "false"]);
    run_git(
        other.path(),
        &["checkout", "-q", "-b", "feature/from-elsewhere"],
    );
    commit_on(other.path(), "f", "f");
    run_git(
        other.path(),
        &["push", "-q", "-u", "origin", "feature/from-elsewhere"],
    );

    // The bui-side clone fetches and gains the remote-tracking ref.
    let repo = CliRepo::at(work.path().to_path_buf());
    repo.fetch(None, false).unwrap();
    let names_before: Vec<_> = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(!names_before.contains(&"feature/from-elsewhere".to_string()));

    // Create + checkout a tracking branch via the new API.
    repo.checkout_remote_tracking("feature/from-elsewhere", "origin/feature/from-elsewhere")
        .unwrap();

    // Local branch exists and is current.
    let branches = repo.list_local_branches().unwrap();
    let new_local = branches
        .iter()
        .find(|b| b.name == "feature/from-elsewhere")
        .expect("new local should be listed");
    assert!(new_local.is_current);

    // Upstream is set.
    let upstream_name = String::from_utf8(
        Command::new("git")
            .current_dir(work.path())
            .args([
                "rev-parse",
                "--abbrev-ref",
                "feature/from-elsewhere@{upstream}",
            ])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    assert_eq!(upstream_name, "origin/feature/from-elsewhere");
}

#[test]
fn set_upstream_configures_tracking_for_local_branch() {
    let (work, _upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    // Push a fresh branch so its remote-tracking counterpart exists, then
    // explicitly drop the tracking config so set_upstream() actually has
    // work to do.
    run_git(work.path(), &["checkout", "-q", "-b", "feature/track-me"]);
    commit_on(work.path(), "m", "marker");
    run_git(
        work.path(),
        &["push", "-q", "-u", "origin", "feature/track-me"],
    );
    run_git(work.path(), &["branch", "--unset-upstream"]);

    repo.set_upstream("feature/track-me", "origin/feature/track-me")
        .unwrap();

    let upstream_name = String::from_utf8(
        Command::new("git")
            .current_dir(work.path())
            .args(["rev-parse", "--abbrev-ref", "feature/track-me@{upstream}"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    assert_eq!(upstream_name, "origin/feature/track-me");
}

#[test]
fn pull_without_upstream_returns_actionable_error() {
    let (work, _upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    // A fresh branch with no upstream.
    run_git(
        work.path(),
        &["checkout", "-q", "-b", "feature/no-upstream"],
    );
    let err = repo.pull().expect_err("pull without upstream should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("no upstream"),
        "expected a hint about missing upstream, got: {msg}"
    );
    assert!(
        msg.contains("press u"),
        "expected message to point at the upstream picker, got: {msg}"
    );
    // The redundant "git pull failed:" prefix is gone.
    assert!(
        !msg.contains("git pull failed"),
        "unexpected redundant prefix in: {msg}"
    );
}

#[test]
fn delete_remote_branch_removes_it_from_upstream() {
    let (work, upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    // Create + push a fresh branch so the bare upstream has it.
    run_git(work.path(), &["checkout", "-q", "-b", "feature/remove-me"]);
    commit_on(work.path(), "x", "x");
    run_git(
        work.path(),
        &["push", "-q", "-u", "origin", "feature/remove-me"],
    );

    // Sanity check: it exists on upstream.
    let before: Vec<String> = String::from_utf8(
        Command::new("git")
            .current_dir(upstream.path())
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .lines()
    .map(|s| s.to_string())
    .collect();
    assert!(before.contains(&"feature/remove-me".to_string()));

    // Delete via bui.
    repo.delete_remote_branch("origin", "feature/remove-me")
        .unwrap();

    let after: Vec<String> = String::from_utf8(
        Command::new("git")
            .current_dir(upstream.path())
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .lines()
    .map(|s| s.to_string())
    .collect();
    assert!(!after.contains(&"feature/remove-me".to_string()));
}

#[test]
fn push_propagates_local_commits_to_bare_upstream() {
    let (work, upstream) = init_work_and_bare_upstream();
    let repo = CliRepo::at(work.path().to_path_buf());

    commit_on(work.path(), "extra", "more work");
    let head_after_local_commit = rev_parse(work.path(), "HEAD");

    repo.push().unwrap();
    let upstream_head = rev_parse(upstream.path(), "main");
    assert_eq!(upstream_head, head_after_local_commit);
}

#[test]
fn pull_brings_in_commits_pushed_elsewhere() {
    let (work, upstream) = init_work_and_bare_upstream();

    // A second worker commits and pushes through the bare upstream.
    let other = tempfile::tempdir().expect("other tempdir");
    let out = Command::new("git")
        .args([
            "clone",
            "-q",
            upstream.path().to_str().unwrap(),
            other.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    run_git(other.path(), &["config", "user.name", "bui-test"]);
    run_git(
        other.path(),
        &["config", "user.email", "bui-test@example.com"],
    );
    run_git(other.path(), &["config", "commit.gpgsign", "false"]);
    commit_on(other.path(), "outside", "from elsewhere");
    run_git(other.path(), &["push", "-q"]);

    let work_head_before = rev_parse(work.path(), "HEAD");
    let repo = CliRepo::at(work.path().to_path_buf());
    repo.pull().unwrap();
    let work_head_after = rev_parse(work.path(), "HEAD");
    assert_ne!(work_head_before, work_head_after);
    assert_eq!(work_head_after, rev_parse(upstream.path(), "main"));
}

#[test]
fn fetch_with_prune_tags_removes_local_tags_missing_from_remote() {
    let upstream = init_repo();
    let clone_dir = tempfile::tempdir().expect("create clone tempdir");
    let out = Command::new("git")
        .args([
            "clone",
            "-q",
            upstream.path().to_str().unwrap(),
            clone_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("spawn git clone");
    assert!(out.status.success());

    // Plant a tag in the clone that the upstream never had.
    run_git(clone_dir.path(), &["tag", "local-only"]);
    let tags_before: Vec<String> = String::from_utf8(
        Command::new("git")
            .current_dir(clone_dir.path())
            .args(["tag", "--list"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .lines()
    .map(|s| s.to_string())
    .collect();
    assert!(tags_before.contains(&"local-only".to_string()));

    let repo = CliRepo::at(clone_dir.path().to_path_buf());
    // Plain fetch leaves the local-only tag alone.
    repo.fetch(None, false).unwrap();
    let tags_mid: Vec<String> = String::from_utf8(
        Command::new("git")
            .current_dir(clone_dir.path())
            .args(["tag", "--list"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .lines()
    .map(|s| s.to_string())
    .collect();
    assert!(tags_mid.contains(&"local-only".to_string()));

    // With prune_tags, the local-only tag is removed.
    repo.fetch(None, true).unwrap();
    let tags_after: Vec<String> = String::from_utf8(
        Command::new("git")
            .current_dir(clone_dir.path())
            .args(["tag", "--list"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .lines()
    .map(|s| s.to_string())
    .collect();
    assert!(!tags_after.contains(&"local-only".to_string()));
}

#[test]
fn fetch_picks_up_new_upstream_branches() {
    let upstream = init_repo();
    let clone_dir = tempfile::tempdir().expect("create clone tempdir");
    let out = Command::new("git")
        .args([
            "clone",
            "-q",
            upstream.path().to_str().unwrap(),
            clone_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("spawn git clone");
    assert!(
        out.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repo = CliRepo::at(clone_dir.path().to_path_buf());

    // A branch added after clone shouldn't be in the local remote-tracking
    // view yet.
    run_git(upstream.path(), &["branch", "newly-added"]);
    let before: Vec<_> = repo
        .list_remote_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(!before.contains(&"newly-added".to_string()));

    // After fetch, it should appear.
    repo.fetch(None, false).unwrap();
    let after: Vec<_> = repo
        .list_remote_branches()
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(after.contains(&"newly-added".to_string()));
}

#[test]
fn create_from_explicit_start_point() {
    let dir = init_repo();
    let repo = open(&dir);
    // Make HEAD advance on main, then branch from the initial commit by sha.
    let initial_sha = {
        let out = Command::new("git")
            .current_dir(dir.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    commit_on(dir.path(), "advance", "advance main");

    repo.create_branch("from-initial", Some(&initial_sha))
        .unwrap();
    let from_initial = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .find(|b| b.name == "from-initial")
        .unwrap();
    assert!(initial_sha.starts_with(&from_initial.short_sha));
}
