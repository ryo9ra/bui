# AGENT.md

Guidance for AI coding agents (and humans) working on `bui`.

## What is bui

`bui` ("(git) branch UI") is a Rust TUI for managing git branches, built on
[`ratatui`](https://ratatui.rs). See `README.md` for the user-facing intro
and `docs/spec.md` for the full functional spec and roadmap.

## Status

**v0.1 + v0.2 + v0.3 feature-complete + UX polish round.** All roadmap
features in `docs/spec.md` are wired:

- v0.1: A1 / A3 / A4 / B1 / B2 / B4 / B5 / B6 / F1 / G1 / G2 / G4 / G5 / I1
- v0.2: A2 Remote tab · A6 markers · A7 Detail pane · B3 Create-from-ref
  · B7 Upstream picker · C1-C3 fetch/pull/push · F3 sort · F4 filter ·
  G6 theme · G7 config
- v0.3: A8 branch diff (`v` toggle) · C4 force-with-lease (auto-confirm
  on non-ff) · C5 Remote `Enter` → tracking · C6 remote-branch delete ·
  C7 opt-in `--prune-tags` · E1-E4 Worktree tab (list / add / remove /
  cd hint), with config-driven path prefill
- UX polish (2026-05-18): Esc / Ctrl-C quit · proportional detail pane
  · middle-truncated long names · context hint footer · ahead/behind
  upstream badge · row flash on create/rename/add-worktree
- A8 extension (2026-05-18): Diff pane renders the full unified
  patch with classified colour + `Ctrl-D` / `Ctrl-U` scroll; cache
  invalidation keys off the (target, base) tuple so checkout / pull
  stay in sync.

`cargo test` runs 135 unit + 30 integration = 165 tests. When asked to
add a feature, cross-reference `docs/spec.md` first — if it belongs
to v0.3 / future, confirm with the user before implementing.

## Commands

```bash
cargo run                                    # run the TUI in the current repo
cargo build --release
cargo test
cargo test <pattern>                         # filter tests by name
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

## Architecture

Unidirectional, event-driven:

```
[keys / ticks / task results]  ─▶  App  ─▶  ui::draw
       ▲                           │
       │                           ▼
       └────  Worker  ◀────  Action dispatch
                 │
                 ▼
            git::Repo trait
```

Key principles (rationale lives in `docs/spec.md` § Architecture):

1. **Single event channel.** Inputs, ticks, and task results merge into one
   `mpsc::Receiver<Event>`. App state mutates only in response to events.
2. **Backend abstraction (`Repo` trait).** v0.1 ships `CliRepo` (shells out
   to `git`). A future `Git2Repo` can drop in without UI changes.
3. **All git ops run on a worker thread.** Even fast local ops go through
   `task::Worker`, so the UI never blocks and v0.2 fetch/pull/push reuse the
   same infrastructure.
4. **Scoped refresh.** Mutations emit `RepoChanged(Scope)`. Views reload
   only the scopes they care about (`LocalBranches`, `RemoteBranches`,
   `Worktrees`).
5. **Pluggable layout (`LayoutSpec`).** Panes are composed declaratively;
   v0.1 ships a list-only spec, v0.2 adds a detail right pane, v0.3 adds a
   diff right pane — without touching the render path.

## Planned module map

```
src/main.rs       terminal lifecycle, panic hook
src/app.rs        App state, main loop
src/event.rs      Event / Action types, channels
src/task.rs       worker thread, mpsc bridge
src/error.rs
src/config.rs     stub (populated in v0.2)
src/ui/           rendering (tabs, branch_list, detail, statusbar,
                  help, confirm, input, layout)
src/git/          Repo trait + CliRepo backend
```

## Conventions

### Git invocations
- Always use machine-readable output: `for-each-ref --format=…`,
  `--porcelain`, `-z`. Never parse `git branch`'s human format.
- Use `git switch` for checkout in new code (not `git checkout`).
- Pass refs by full name (`refs/heads/foo`) when ambiguity is possible.

### Error handling
- Internal: `Result<T, BuiError>` (`thiserror`).
- CLI / top-level boundary: `anyhow::Result` for context chaining.
- No `unwrap()` outside `main` and tests. `expect("invariant: …")` is fine
  with a justification string.

### Async / threading
- **No `tokio`.** Use `std::thread` + `std::sync::mpsc`.
- The main thread renders; all blocking I/O (process spawn) runs in
  `task::Worker`.

### UI
- Key bindings: vim (`j` `k` `g` `G`) **and** arrow keys are both bound.
- Destructive ops route through `ui::confirm`.
- The status bar shows the last result; the right edge shows a spinner
  during async ops.

### Tests
- Unit tests for pure logic (parsers, state reducers).
- `CliRepo` integration tests build a temp repo with `tempfile::tempdir()`
  + real `git init`. Don't mock git — verify parsers against real output.

## Before opening a PR

```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Update `docs/spec.md` if user-visible behaviour changes.
