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
