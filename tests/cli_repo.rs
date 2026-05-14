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
    assert!(i_newer < i_older, "newer should sort before older: {names:?}");
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
    run_git(work.path(), &["config", "user.email", "bui-test@example.com"]);
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
        .args([
            "rev-parse",
            "--abbrev-ref",
            "feature/new@{upstream}",
        ])
        .output()
        .unwrap();
    assert!(upstream_cfg.status.success());
    let upstream_name = String::from_utf8(upstream_cfg.stdout).unwrap().trim().to_string();
    assert_eq!(upstream_name, "origin/feature/new");
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
    run_git(work.path(), &["push", "-q", "-u", "origin", "feature/track-me"]);
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
    run_git(work.path(), &["checkout", "-q", "-b", "feature/no-upstream"]);
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
    assert!(!msg.contains("git pull failed"), "unexpected redundant prefix in: {msg}");
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
    run_git(other.path(), &["config", "user.email", "bui-test@example.com"]);
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
    repo.fetch(None).unwrap();
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

    repo.create_branch("from-initial", Some(&initial_sha)).unwrap();
    let from_initial = repo
        .list_local_branches()
        .unwrap()
        .into_iter()
        .find(|b| b.name == "from-initial")
        .unwrap();
    assert!(initial_sha.starts_with(&from_initial.short_sha));
}
