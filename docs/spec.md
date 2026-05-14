# bui — Specification

## 1. Overview

`bui` is a terminal UI for **git branch operations**. It runs inside a git
repository and presents an interactive view of branches together with the
most common branch-management actions.

The defining constraint: **bui is about branches**, not the whole of git.
Staging, committing, merging, rebasing, and cherry-picking are out of
scope (or, at best, delegated to external tools).

## 2. Goals

- Fast keyboard-driven branch CRUD that beats raw `git` CLI for repos with
  more than ~10 branches.
- Predictable behaviour: every action maps to one or two visible `git`
  commands. No hidden state.
- Trustworthy: destructive operations always confirm; errors from `git`
  are surfaced verbatim.

## 3. Non-goals (explicit, decided 2026-05-14)

- Commit / stage management
- Merge, rebase, cherry-pick, interactive rebase (category D)
- Mouse support (G8)
- Command palette (G3)
- PR / forge integration (H5)
- Upstream ahead/behind display (A5)
- ASCII commit graph (A9)

Out-of-scope items are revisited only if real demand emerges.

## 4. v0.1 scope

### 4.1 Feature list

| Code | Feature                                              |
|------|------------------------------------------------------|
| A1   | List local branches                                  |
| A3   | Highlight the current branch                         |
| A4   | Show latest commit (sha7 + subject + relative date)  |
| B1   | Checkout branch (`git switch`)                       |
| B2   | Create branch from HEAD                              |
| B4   | Delete branch (`-d`)                                 |
| B5   | Force-delete branch (`-D`)                           |
| B6   | Rename branch                                        |
| F1   | Incremental name search                              |
| G1   | Vim and arrow key bindings                           |
| G2   | Help overlay                                         |
| G4   | Confirm dialog for destructive ops                   |
| G5   | Status bar (result, error, async spinner)            |
| I1   | `cargo install bui` distribution                     |

### 4.2 Key bindings

| Key                   | Action                                  |
|-----------------------|-----------------------------------------|
| `j` / `↓`             | Move selection down                     |
| `k` / `↑`             | Move selection up                       |
| `g` / `Home`          | Jump to top                             |
| `G` / `End`           | Jump to bottom                          |
| `Ctrl-d` / `Ctrl-u`   | Half-page down / up                     |
| `Enter`               | Checkout selected branch                |
| `c`                   | Create new branch (opens input)         |
| `r`                   | Rename selected branch (opens input)    |
| `d`                   | Delete (`-d`, confirm)                  |
| `D`                   | Force-delete (`-D`, confirm)            |
| `/`                   | Start incremental search                |
| `Esc`                 | Cancel input / modal / search           |
| `R`                   | Refresh                                 |
| `?`                   | Toggle help overlay                     |
| `Tab` / `Shift-Tab`   | Switch tab (Local active in v0.1)       |
| `q`                   | Quit                                    |

### 4.3 Behaviour notes

- The current branch cannot be deleted; pressing `d`/`D` on it surfaces
  an error in the status bar (verbatim from `git`).
- Renaming the current branch is allowed (`git branch -m <new>`).
- After every successful mutation, the affected list refreshes and the
  cursor stays on the same branch (or moves to the new name on rename).
- Errors from `git` are surfaced verbatim in the status bar; full stderr
  is available in the help/log overlay (mechanism TBD in v0.1).
- The Remote and Worktree tabs render in v0.1 as **placeholders**: they
  can be focused (so the tab affordance is visible from day one) but the
  body says "Coming in v0.2 / v0.3".

### 4.4 Default UI layout (v0.1)

```
┌────────────────────────────────────────────────────────────────┐
│ [Local]  Remote   Worktree                          bui v0.1   │  Tab bar
├────────────────────────────────────────────────────────────────┤
│ * main                       abc1234  fix: oauth      2d ago   │
│   feature/foo                def5678  wip: ui         1h ago   │  Main pane
│   feature/bar                ghi9abc  refactor: io    3d ago   │  (v0.1: full
│   chore/deps                 jkl0def  bump tokio      5d ago   │   width)
│                                                                │
├────────────────────────────────────────────────────────────────┤
│ /search                                       ⟳ ready          │  Status bar
└────────────────────────────────────────────────────────────────┘
```

## 5. Architecture

### 5.1 Event loop

A single `mpsc::Receiver<Event>` drives the app. Producers:

- **Input thread** — crossterm key events → `Event::Input(KeyEvent)`
- **Tick thread** — ~30 Hz → `Event::Tick` (drives spinners and any
  time-based redraws)
- **Task worker** → `Event::TaskResult(TaskId, Result<Outcome>)`

Sketch:

```rust
loop {
    match rx.recv()? {
        Event::Input(k)         => app.on_key(k),
        Event::Tick             => app.on_tick(),
        Event::TaskResult(id,r) => app.on_task_result(id, r),
    }
    if app.dirty() { terminal.draw(|f| ui::draw(f, &app))?; }
    if app.should_quit() { break; }
}
```

App state mutates only inside the `on_*` handlers. UI is a pure projection
of state.

### 5.2 Backend abstraction

```rust
trait Repo: Send + Sync {
    fn list_local_branches(&self) -> Result<Vec<Branch>>;
    fn checkout(&self, name: &str) -> Result<()>;
    fn create_branch(&self, name: &str, from: Option<&str>) -> Result<()>;
    fn delete_branch(&self, name: &str, force: bool) -> Result<()>;
    fn rename_branch(&self, old: &str, new: &str) -> Result<()>;

    // v0.2:
    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>>;
    fn fetch(&self, remote: Option<&str>) -> Result<()>;
    fn pull(&self, mode: PullMode) -> Result<()>;
    fn push(&self, opts: PushOpts) -> Result<()>;

    // v0.3:
    fn list_worktrees(&self) -> Result<Vec<Worktree>>;
    fn add_worktree(&self, path: &Path, branch: &str) -> Result<()>;
    fn remove_worktree(&self, path: &Path) -> Result<()>;
    fn diff_branches(&self, a: &str, b: &str) -> Result<Diff>;
}
```

v0.1 implementation: `CliRepo`. Each method spawns `git` via
`std::process::Command` and parses machine-readable output
(`for-each-ref --format=…`, `--porcelain`, `-z`).

### 5.3 Pluggable layout

The render path queries a `LayoutSpec`:

```rust
struct LayoutSpec {
    tabs: bool,           // top tab bar
    main: MainSpec,       // see below
    statusbar: bool,
}

enum MainSpec {
    BranchList,                       // v0.1: full-width list
    Split(BranchList, RightPane),     // v0.2+: list | right pane
}

enum RightPane {
    Detail,   // v0.2: latest commit body + file stats
    Diff,     // v0.3: A...B diff against current branch
}
```

- **v0.1** uses `MainSpec::BranchList`.
- **v0.2** flips to `MainSpec::Split(BranchList, RightPane::Detail)`.
- **v0.3** lets the user toggle the right pane between `Detail` and
  `Diff`.

This is the *only* reason we abstract the layout from day one: the
diff-vs-detail decision is a v0.3 feature, but we don't want to rewrite
the render path when it lands. Locking the right pane behind an enum
makes it a localized change.

### 5.4 Refresh model

State mutations emit `Event::RepoChanged(Scope)`:

```rust
enum Scope { LocalBranches, RemoteBranches, Worktrees }
```

Each view observes the scopes it depends on and dispatches a re-fetch
through the worker. This decouples "what changed" from "what to redraw"
and lets v0.2 add remote-side mutations without touching the local view.

### 5.5 Module map

```
src/main.rs       terminal lifecycle, panic hook
src/app.rs        App state, main loop, key dispatch
src/event.rs      Event / Action types, mpsc channels
src/task.rs       worker thread, mpsc bridge
src/error.rs      BuiError
src/config.rs     stub (populated in v0.2)
src/ui/
  mod.rs          top-level layout
  layout.rs       LayoutSpec, RightPane
  tabs.rs         Local / Remote / Worktree tab strip
  branch_list.rs  the v0.1 main view
  detail.rs       v0.2 right pane (Detail)
  diff.rs         v0.3 right pane (Diff)
  statusbar.rs    result / error / spinner
  help.rs         `?` overlay
  confirm.rs      destructive-op confirmation
  input.rs        single-line input (create / rename / search)
src/git/
  mod.rs          Repo trait, facade
  cli.rs          CliRepo (v0.1)
  types.rs        Branch, Commit, RemoteBranch, Worktree, Diff
  ops/
    branches.rs   v0.1
    remote.rs     v0.2
    worktree.rs   v0.3
```

## 6. Roadmap

### v0.2 — Remote operations

| Code | Feature                                            |
|------|----------------------------------------------------|
| A2   | List remote branches (Remote tab becomes active)   |
| A6   | "merged" / "in worktree" markers                   |
| A7   | Right detail pane (latest commit body + file stats)|
| B3   | Create branch from arbitrary ref                   |
| B7   | Set / change upstream                              |
| C1   | fetch (all / single remote)                        |
| C2   | pull (ff-only / rebase modes)                      |
| C3   | push (current / selected)                          |
| F3   | Sort toggles (recency / name)                      |
| F4   | Filters (merged / unmerged / stale / author=me)    |
| G6   | Colour themes                                      |
| G7   | Config file (`~/.config/bui/config.toml`)          |
| —    | Layout default flips to `Split(_, Detail)`         |

### v0.3 — Worktrees & diff

| Code | Feature                                              |
|------|------------------------------------------------------|
| E1   | Worktree list (Worktree tab becomes active)          |
| E2   | Add worktree for a branch                            |
| E3   | Remove worktree                                      |
| E4   | "switch to worktree" — emit `cd` hint for a shell wrapper |
| A8   | Right pane diff (`A...B` vs current branch)          |
| C4   | force-with-lease push                                |
| C5   | Create local tracking branch from remote             |
| C6   | Delete remote branch (`push --delete`)               |
| C7   | Prune (`fetch --prune`)                              |

### v0.4+ — Polish & power features

- F2 fuzzy search (e.g. [`nucleo`](https://github.com/helix-editor/nucleo))
- H1 tags, H2 stash list, H3 reflog viewer
- H4 per-branch notes / favourites (local persistence)
- H6 conventional-commit branch naming helper
- I2 Homebrew tap, I3 man page, I4 shell completions

## 7. Decision log

- **2026-05-14 — Git backend: CLI shell-out.**
  Chose `git` CLI over `git2`/libgit2 because (a) branch operations are
  simple and well served by machine-readable output, (b) the user's git
  config, hooks, and credential helpers Just Work, (c) zero native build
  deps. Trade-off accepted: fork/exec per op. The `Repo` trait leaves the
  door open to swap in `git2` if specific ops show measurable cost in
  large repos.

- **2026-05-14 — No tokio.**
  `std::thread` + `mpsc` is sufficient for shell-out workloads. tokio's
  value (massive concurrent I/O, async/await ergonomics) does not apply
  to fork/exec of a few git commands.

- **2026-05-14 — Pluggable layout from v0.1.**
  A `LayoutSpec` abstraction is overkill for the v0.1 single-pane UI in
  isolation, but it is cheap to add now and removes the v0.2/v0.3 risk
  of rewriting the render path. Right pane is reserved by design, not
  rendered in v0.1.

- **2026-05-14 — Out of scope.**
  merge/rebase/cherry-pick (D), mouse (G8), command palette (G3), PR
  integration (H5), ahead/behind display (A5), commit graph (A9). Decided
  during initial scoping; revisit if real demand emerges.
