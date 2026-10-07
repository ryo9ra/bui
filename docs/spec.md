# bui — Specification

## 1. Overview

`bui` is a terminal UI for **git branch operations**. It runs inside a git
repository and presents an interactive view of branches together with the
most common branch-management actions.

The defining constraint: **bui is about branches**, not the whole of git.
Staging, committing, merging, rebasing, and cherry-picking are out of
scope (or, at best, delegated to external tools).

## Status (2026-05-18)

**v0.1 + v0.2 + v0.3 are feature-complete** and a round of UX polish
landed on 2026-05-18 (see §8). The diff pane (A8) was also extended
the same day to render the full unified-diff patch with scroll,
replacing the earlier commit-list-only view. Every code in §4.1 and
§6 is wired. `cargo test` runs 165 tests (135 unit + 30 integration).
See the per-feature checkmarks in §6 below.

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
- Upstream ahead/behind display (A5) — superseded by A8 (diff pane),
  which shows the same information in a richer form
- ASCII commit graph (A9)

Out-of-scope items are revisited only if real demand emerges.

## 4. v0.1 scope (baseline)

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

### 4.2 Behaviour notes (cumulative)

- The current branch cannot be deleted; pressing `d`/`D` on it surfaces
  an error in the status bar (verbatim from `git`).
- Renaming the current branch is allowed (`git branch -m <new>`).
- After every successful mutation, the affected list refreshes and the
  cursor stays on the same branch (or moves to the new name on rename).
- Errors from `git` are surfaced verbatim in the status bar; only the
  first useful stderr line is rendered (status bar is one row).
- The Remote and Worktree tabs are fully active as of v0.2 / v0.3
  respectively.

## 5. Full key map (current)

Bindings labelled `(Local)` / `(Remote)` / `(Worktree)` only fire when
that tab is active. Unlabelled keys work from anywhere.

### Navigation & view

| Key                 | Action                                          |
|---------------------|-------------------------------------------------|
| `j` / `↓`           | Move selection down                             |
| `k` / `↑`           | Move selection up                               |
| `g` / `Home`        | Jump to top                                     |
| `G` / `End`         | Jump to bottom                                  |
| `Tab` / `Shift-Tab` | Cycle tab (Local → Remote → Worktree)           |
| `R`                 | Refresh local + remote + worktrees              |
| `?`                 | Toggle help overlay                             |
| `q`                 | Quit                                            |
| `Esc`               | Cancel modal / input / clear filter; with nothing to dismiss, quit |
| `Ctrl-C`            | Quit immediately (bypasses every modal/popup)   |

### Branch operations

| Key       | Action                                                  |
|-----------|---------------------------------------------------------|
| `Enter`   | (Local) checkout                                        |
| `Enter`   | (Remote) create local tracking branch & switch          |
| `Enter`   | (Worktree) show `cd <path>` hint in status              |
| `c`       | (Local) create branch from HEAD                         |
| `C`       | Create branch from selected ref (Local or Remote)       |
| `r`       | (Local) rename selected branch                          |
| `d`       | (Local) delete `-d` · (Remote) `push --delete` · (Worktree) remove |
| `D`       | (Local) force-delete `-D`                               |
| `u`       | (Local) set upstream — pick a remote                    |
| `W`       | (Local) add worktree (2-step prompt)                    |

### Remote sync

| Key   | Action                                                              |
|-------|---------------------------------------------------------------------|
| `f`   | `git fetch --all --prune [--prune-tags]` (async)                    |
| `X`   | Clean gone: `fetch --prune` (async), then one confirm listing every |
|       | local branch whose upstream is `[gone]` → `git branch -D` each.     |
|       | Current / worktree-checked-out branches are skipped.                |
| `p`   | `git pull` (async)                                                  |
| `P`   | `git push` (async). On non-fast-forward, bui offers a              |
|       | confirmable `--force-with-lease` retry.                             |

### Filter / sort / view

| Key   | Action                                                          |
|-------|-----------------------------------------------------------------|
| `/`   | Incremental name search                                         |
| `s`   | Toggle sort (recency ↔ name)                                    |
| `F`   | (Local) cycle filter (all → merged → unmerged → all)            |
| `v`   | Toggle right pane (detail ↔ diff vs current branch)             |
| `Ctrl-D` / `Ctrl-U` | Scroll the Diff pane down / up half a page          |

### Input editing

| Key      | Action                                          |
|----------|-------------------------------------------------|
| `Ctrl-U` | Clear the input value (readline-style)          |

### Visual feedback

- Local branches with an upstream show their relationship inline:
  cyan `↑3↓1` / `↑2` / `↓1`, yellow `(gone)` when the upstream ref no
  longer exists. Synced upstreams stay quiet. The detail pane also
  carries an `Upstream:` summary line.
- Branch names and worktree paths longer than the column get
  **middle-truncated** with `…`; the leaf stays visible.
- After a successful create / rename / add-worktree, the new row
  flashes green for ~1.25 s so the change is obviously visible.
- The row beneath the main pane is a **context hint footer**: dim
  per-tab quick-reference of the most useful keys. Long lines clip
  on narrow terminals; the highest-value keys come first.
- The Diff pane (`v`) renders the full `git diff base...target`
  unified-diff output with classified colour: green for `+`, red
  for `-`, cyan for `@@` hunks, bold-white for `diff --git` file
  headers, dim for metadata. Above the patch a 3-line band shows
  the target/base pair, ahead/behind counts, and the total patch
  line count. `Ctrl-D` / `Ctrl-U` scroll the patch half a page at
  a time; the scroll position resets when the cursor lands on a
  different target.

### Confirm dialog

| Key             | Action                                       |
|-----------------|----------------------------------------------|
| `←` / `→` / `h` / `l` | Move Yes/No focus                       |
| `Tab` / `Shift-Tab`   | Toggle Yes/No focus                     |
| `Enter`         | Accept whichever is focused                  |
| `y` / `Y`       | Direct accept                                |
| `n` / `N` / `Esc` | Direct cancel                              |

Default focus is **No** for destructive operations.

## 6. Architecture

### 6.1 Event loop

A single `mpsc::Receiver<Event>` drives the app. Producers:

- **Input thread** — crossterm key events → `Event::Input(KeyEvent)`
- **Tick thread** — ~4 Hz → `Event::Tick` (advances the spinner only
  while a task is pending)
- **Task worker** → `Event::TaskResult(TaskId, Result<Outcome>)`

```rust
loop {
    match rx.recv()? {
        Event::Input(k)         => app.on_key(k),
        Event::Tick             => app.on_tick(),
        Event::TaskResult(id,r) => app.on_task_result(id, r),
    }
    if app.dirty { terminal.draw(|f| ui::draw(f, &app))?; }
    if app.should_quit { break; }
}
```

App state mutates only inside the `on_*` handlers. UI is a pure projection
of state.

### 6.2 Backend abstraction

The trait actually shipped:

```rust
trait Repo: Send + Sync {
    fn list_local_branches(&self) -> Result<Vec<Branch>>;
    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>>;
    fn checkout(&self, name: &str) -> Result<()>;
    fn create_branch(&self, name: &str, from: Option<&str>) -> Result<()>;
    fn delete_branch(&self, name: &str, force: bool) -> Result<()>;
    fn rename_branch(&self, old: &str, new: &str) -> Result<()>;
    fn fetch(&self, remote: Option<&str>, prune_tags: bool) -> Result<()>;
    fn pull(&self) -> Result<()>;
    fn push(&self) -> Result<()>;
    fn push_force_with_lease(&self) -> Result<()>;
    fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()>;
    fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<()>;
    fn checkout_remote_tracking(&self, local: &str, remote_ref: &str) -> Result<()>;
    fn list_worktrees(&self) -> Result<Vec<Worktree>>;
    fn add_worktree(&self, path: &str, base: &str, new_branch: Option<&str>) -> Result<()>;
    fn remove_worktree(&self, path: &str) -> Result<()>;
    fn branch_diff(&self, target: &str, base: &str) -> Result<BranchDiff>;
}
```

Sole implementation: `CliRepo`. Each method spawns `git` via
`std::process::Command` and parses machine-readable output
(`for-each-ref --format=…`, `worktree list --porcelain`, `--pretty=format:%h\x1f%s`).

The `for-each-ref` format is 7 fields (HEAD · name · sha · rel-date ·
upstream-short · upstream-track · subject). Subject lives last so a
stray `\x1f` in a commit message can't truncate later fields. The
upstream-short / upstream-track pair feeds `Branch.upstream_track`
(`Option<UpstreamTrack>`) and powers the ahead/behind badge.

`branch_diff` returns a `BranchDiff { target, base, ahead, behind,
patch }`. The first two come from `git log base..target` and
`git log target..base`; `patch` is the parsed output of `git diff
--no-color base...target` (three dots — same view as PR review).
`DiffLine` classifies each line as FileHeader / Hunk / Add /
Remove / Context / Meta so the renderer can colour without
re-parsing.

### 6.3 Layout

```rust
struct LayoutSpec {
    tabs: bool,
    main: MainSpec,
    statusbar: bool,
}

enum MainSpec {
    BranchList,   // (unused at runtime; reserved for "no detail" mode)
    Split,        // list | right pane
}

enum RightPane {
    Detail,       // selected branch metadata
    Diff,         // commits ahead/behind current branch
}
```

The `RightPane` variant lives on `App` (not on `MainSpec`) so the user
can toggle Detail ↔ Diff at runtime via `v` without re-shaping the
layout.

### 6.4 Async task model

Long-running git ops (fetch / pull / push / force-with-lease push /
remote-branch delete) flow through a dedicated worker thread:

```rust
pub enum Action {
    Fetch { remote: Option<String>, prune_tags: bool },
    Pull,
    Push,
    PushForceWithLease,
    DeleteRemoteBranch { remote: String, branch: String },
}

pub enum Outcome {
    Fetched,
    Pulled,
    Pushed,
    RemoteBranchDeleted { full_name: String },
}
```

App carries `pending_task: Option<PendingTask>` and a monotonic
`next_task_id`. While a task is in flight, the status bar shows a
braille spinner; a second `f`/`p`/`P` is ignored. The corresponding
`on_task_result` handler may chain a follow-up confirm dialog (e.g.
non-fast-forward push offers a `--force-with-lease` retry).

### 6.5 Module map (current)

```
src/main.rs            terminal lifecycle, panic hook
src/app.rs             App state, main loop, key dispatch, picker/confirm/
                       input/spinner state machines
src/event.rs           Event / Outcome types, mpsc channels
src/task.rs            worker thread that drains Action → Outcome
src/error.rs           BuiError
src/config.rs          TOML config loader (theme, fetch.prune_tags,
                       worktree.root)
src/lib.rs             re-export modules for integration tests
src/ui/
  mod.rs               top-level layout dispatch + middle_truncate
  layout.rs            LayoutSpec / MainSpec / RightPane
  tabs.rs              Local / Remote / Worktree tab strip + status meta
  branch_list.rs       Local, Remote, Worktree list renderers
  detail.rs            right-pane detail for branches and worktrees
  diff.rs              right-pane branch-to-branch diff
  hint_bar.rs          per-tab context key footer above status
  statusbar.rs         result / error / spinner / search prompt
  help.rs              `?` overlay
  confirm.rs           yes/no confirm with focused-button UI
  input.rs             single-line input popup (with Ctrl-U clear)
  upstream_picker.rs   remote picker for `u`
src/git/
  mod.rs               Repo trait + facade
  cli.rs               CliRepo
  types.rs             Branch / RemoteBranch / Worktree / Commit /
                       BranchDiff
  ops/
    branches.rs        list / checkout / create / delete / rename /
                       set_upstream / checkout_tracking / branch_diff /
                       merged & worktree augmentation
    remote.rs          fetch / pull / push / force-with-lease /
                       delete_branch (push --delete). push always uses
                       `-u origin HEAD` so a missing or mis-named
                       upstream is reconciled in a single call.
    worktree.rs        list / add (with optional -b) / remove
```

## 7. Roadmap (status)

### v0.2 — Remote operations ✓ shipped

| Code | Feature                                            | Status |
|------|----------------------------------------------------|--------|
| A2   | List remote branches (Remote tab becomes active)   | ✓      |
| A6   | "merged" / "in worktree" markers                   | ✓      |
| A7   | Right detail pane (selected branch metadata)       | ✓      |
| B3   | Create branch from arbitrary ref (`C`)             | ✓      |
| B7   | Set / change upstream (`u`)                        | ✓      |
| C1   | fetch (all / single remote)                        | ✓      |
| C2   | pull                                               | ✓      |
| C3   | push                                               | ✓      |
| F3   | Sort toggle (recency ↔ name) (`s`)                 | ✓      |
| F4   | Filters (all / merged / unmerged) (`F`)            | ✓ partial — stale & author=me deferred |
| G6   | Colour themes                                      | ✓      |
| G7   | Config file (`~/.config/bui/config.toml`)          | ✓      |
| —    | Layout default flips to `Split(_, Detail)`         | ✓      |

### v0.3 — Worktrees, diff, advanced remote ✓ shipped

| Code | Feature                                              | Status |
|------|------------------------------------------------------|--------|
| E1   | Worktree list (Worktree tab becomes active)          | ✓      |
| E2   | Add worktree, optionally creating a new branch off a base (`W`, 2-step) | ✓ |
| E3   | Remove worktree (`d`)                                | ✓      |
| E4   | "switch to worktree" — `cd <path>` hint in status    | ✓ (no shell wrapper yet) |
| A8   | Right-pane diff (`A...B` vs current branch) (`v`)    | ✓      |
| C4   | force-with-lease push (auto-offered on non-ff)       | ✓      |
| C5   | Create local tracking branch from remote (`Enter` on Remote) | ✓ |
| C6   | Delete remote branch (`d` on Remote, push --delete)  | ✓      |
| C7   | `--prune-tags` (opt-in via config)                   | ✓ — `--prune` itself is always on |
| C8   | Clean gone (`X`): fetch --prune + bulk-delete `[gone]` branches | ✓ (added 2026-10-07) |

### v0.4+ — Polish & power features

- F2 fuzzy search (e.g. [`nucleo`](https://github.com/helix-editor/nucleo))
- F4 stale / author=me predicates (need committerdate-unix + authorname)
- H1 tags, H2 stash list, H3 reflog viewer
- H4 per-branch notes / favourites (local persistence)
- H6 conventional-commit branch naming helper
- I2 Homebrew tap, I3 man page, I4 shell completions
- Shell wrapper that turns the Worktree-Enter `cd` hint into a real
  `cd` (design pre-decided as Plan B — see §8)

## 8. Decision log

- **2026-05-14 — Git backend: CLI shell-out.**
  Chose `git` CLI over `git2`/libgit2 because (a) branch operations are
  simple and well served by machine-readable output, (b) the user's git
  config, hooks, and credential helpers Just Work, (c) zero native build
  deps.

- **2026-05-14 — No tokio.**
  `std::thread` + `mpsc` is sufficient for shell-out workloads. tokio's
  value (massive concurrent I/O, async/await ergonomics) does not apply
  to fork/exec of a few git commands.

- **2026-05-14 — Pluggable layout from v0.1.**
  A `LayoutSpec` abstraction is overkill for the v0.1 single-pane UI in
  isolation, but it is cheap to add now and removes the v0.2/v0.3 risk
  of rewriting the render path.

- **2026-05-14 — Out of scope.**
  merge/rebase/cherry-pick (D), mouse (G8), command palette (G3), PR
  integration (H5), ahead/behind display (A5), commit graph (A9).

- **2026-05-15 — Async worker is "one task at a time".**
  The worker drains `Action`s serially; while `pending_task` is `Some`,
  another `f`/`p`/`P` is silently ignored rather than queued. Simpler
  state, predictable order, no head-of-line surprises. If parallelism
  ever matters (e.g. simultaneous fetches against multiple remotes),
  this is the place to revisit.

- **2026-05-15 — Push UX: auto-set-upstream + force-with-lease confirm.**
  First push of a fresh branch silently retries with
  `--set-upstream origin HEAD` so users don't need to think about it.
  Non-fast-forward failures open a confirm dialog offering a
  `--force-with-lease` retry; plain `--force` is intentionally not
  exposed.

- **2026-05-15 — Upstream picker: remote names only.**
  Variant B of the picker — bui composes `<remote>/<current-branch>` on
  submit. Loses the "track a differently-named upstream" case (`local
  wip/oauth` → `origin/feature/oauth`); that flow remains a `git
  branch --set-upstream-to=` shell call. Acceptable corner case.

- **2026-05-15 — Worktree path prefill from config.**
  `[worktree] root = "~/wt"` causes step 2 of `W` to prefill
  `<root>/<branch>`. Without the config, the fallback is
  `../wt-<leaf>` (last `/`-segment of the branch). Reduces the common
  flow to 4 keystrokes.

- **2026-05-15 — Diff pane is a runtime toggle, not a layout variant.**
  `App.right_pane` (Detail / Diff) is the source of truth; `LayoutSpec`
  only knows whether the layout is split or single. Lets `v` switch
  Detail ↔ Diff without re-shaping the layout tree.

- **2026-05-15 — Worktree-Enter is a `cd` hint, not a real cd.**
  A TUI can't `cd` its parent shell. v0.3 ships the status-bar hint;
  v0.4+ may ship a shell wrapper (e.g. `bui-cd` zsh function) that
  reads the last hint and performs the `cd`.

- **2026-05-18 — UX polish round 1.**
  Eight UX gaps closed in one session. Decisions worth keeping:

  - `Esc` with nothing to dismiss is now a quit fallback; `Ctrl-C`
    is the always-quits universal escape hatch (bypasses every
    modal). Inside an overlay, `Esc` still closes the overlay first.
  - Detail pane is `Percentage(40)` instead of fixed `Length(40)`
    so it scales with the terminal. Branch list keeps a `Min(30)`
    floor.
  - Long branch names / worktree paths use **middle truncation**
    (`feature/…r-1234`) rather than ratatui's default end-trunc,
    so the identifying leaf stays visible.
  - A dim **hint footer** sits between the main pane and status
    bar, showing the most useful keys for the active tab. The
    startup status line ("? help · Tab tabs · …") doubles as a
    first-launch nudge.
  - Local branches show their upstream relationship inline (`↑3↓1`,
    `(gone)`, hidden when synced) via `%(upstream:short)` +
    `%(upstream:track)` atoms. No extra git invocation per branch.
  - Success on create / rename / add-worktree flashes the row
    green for ~1.25 s (`FlashState` + `FlashKind` on App, ttl
    decremented in `on_tick`). Checkout / delete / fetch /
    pull / push do not flash — only "new visible row" verbs.
  - Cursor after delete uses the existing clamp behaviour
    ("next adjacent, fallback to new last") — verified by tests,
    not changed.

- **2026-05-18 — A8 diff pane shows the full patch, not just
  commits.**
  v0.3's initial A8 showed two compact commit lists (ahead /
  behind). The pane now renders the unified-diff patch from
  `git diff --no-color base...target` (three dots), classified
  into `DiffLine::{FileHeader, Hunk, Add, Remove, Context,
  Meta}` and coloured accordingly. Header band keeps the
  target/base/ahead/behind summary above the scrollable patch.
  `Ctrl-D` / `Ctrl-U` scroll by 10 lines; chosen over j/k so
  the list-side cursor remains the j/k owner. Plain `d` for
  delete is preserved because the Ctrl-D match arm comes first
  and uses a `KeyModifiers::CONTROL` guard.

- **2026-05-18 — Diff cache invalidation keys off the
  (target, base) tuple, not just target.**
  Earlier the cache compared only the target name. After a
  checkout, the cached `{target: feature/foo, base: main}`
  stayed live even though `feature/foo` had become the current
  branch (so target should equal base and the pane should
  show no-diff). Now `maybe_refresh_branch_diff` computes the
  desired `Option<(target, base)>` and recomputes whenever it
  differs from the cached pair. `on_task_result` also forces
  a recompute when Diff mode is active, so pull / fetch /
  push that move HEAD don't leave stale lines visible.

- **2026-05-18 — Shell wrapper design locked in as Plan B (explicit
  Enter, no auto-cd from `W`).**
  Decision is logged now to keep future work resumable. v0.3 ships
  without it; whoever picks this up implements as follows:

  - **Contract.** bui reads `BUI_CD_TARGET_FILE` at startup. If set,
    `Enter` on a Worktree row writes the worktree's absolute path to
    that file (overwriting any prior value) **in addition to** the
    existing status-bar hint. On clean quit (`q` / `Esc`), the file
    is left as-is for the wrapper to consume.
  - **Plan B explicitly, not Plan A.** `W` (add worktree) does **not**
    auto-set the cd target even when the new worktree lands cleanly.
    The user must press `Enter` on the new row to confirm intent. This
    avoids the "I just ran W to look around and accidentally got
    teleported" failure mode and keeps `Enter` as the single, explicit
    "I want to go here" verb.
  - **Per-press latching.** Each `Enter` overwrites the target file.
    If the user picks A, then B, then quits, B wins. If they pick A
    and then navigate away without re-pressing Enter, A still wins.
    Suitable for the typical "open bui → pick or create wt → quit
    there" flow.
  - **Suggested wrapper** (zsh / bash equivalent):
    ```sh
    bui() {
        local target_file="$(mktemp)"
        BUI_CD_TARGET_FILE="$target_file" command bui "$@"
        local target=$(cat "$target_file" 2>/dev/null)
        rm -f "$target_file"
        [ -n "$target" ] && [ -d "$target" ] && cd "$target"
    }
    ```
  - **Out of scope for the wrapper feature.** Multi-step "cd to bui's
    suggestion then back" flows; integration with `direnv`; explicit
    `:cd <path>` command palette. Revisit if demand emerges.

- **2026-10-07 — Clean gone (`X`) is fetch + one confirm + `-D`.**
  The common post-merge chore (`git fetch --prune` → find `[gone]`
  branches → delete them) collapses into one key. Decisions:

  - **Fetch first, then confirm.** The candidate list is only known
    after the prune-fetch, so `X` dispatches
    `Action::FetchForCleanGone` and the confirm opens when the
    worker reports back. The confirm lists the branches (first 8,
    then "…and N more") and marks unmerged ones; default focus is
    No like every destructive op.
  - **`-D`, not `-d`.** Squash / rebase merges leave the local tip
    unreachable from HEAD, so `-d` would refuse the main use case.
    The upstream being gone plus the explicit list in the confirm
    is the safety net.
  - **Skipped, not failed.** The current branch and branches
    checked out in another worktree are left alone and named in
    the confirm / status line; bui does not remove worktrees here.
  - Deletes run synchronously on the main thread like `d` / `D`
    (local ref ops, fast). Per-branch failures are collected into
    the status line instead of aborting the batch.
