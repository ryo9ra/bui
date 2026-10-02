# bui

A terminal UI for git branch operations, written in Rust with
[`ratatui`](https://ratatui.rs).

`bui` stands for "**b**ranch **UI**". It focuses narrowly on the branch
domain: list, switch, create, rename, delete. It is **not** a full git TUI —
staging, committing, merging, and rebasing are deliberately out of scope.

## Status

v0.1 + v0.2 + v0.3 feature-complete; not yet published to crates.io. See
[`docs/spec.md`](docs/spec.md) for the formal scope and roadmap.

## Features

- **Local branches** — list with latest commit, ahead/behind upstream
  badge, merged / in-worktree markers; switch (`Enter`), create (`c`, or
  `C` from the selected ref), rename (`r`), delete (`d` / `D`), set
  upstream (`u`)
- **Remote branches** — browse, check out as a tracking branch
  (`Enter`), delete on the remote (`d`)
- **Sync** — `fetch` / `pull` / `push`, with force-with-lease on
  non-fast-forward (always confirmed)
- **Worktrees** — list, add, and remove worktrees from their own tab
- **Detail & diff pane** — branch details on the right; `v` toggles a
  branch-to-branch unified diff, scrollable with `Ctrl-D` / `Ctrl-U`
- **Navigation** — `j`/`k`/arrows, `g`/`G`, incremental search (`/`),
  sort (`s`), merged/unmerged filter (`F`)
- **Safety & feedback** — confirm dialog for destructive operations,
  status bar with a spinner during async ops, context key hints, help
  overlay (`?`)
- **Config** — TOML config file with a customizable theme

## Install

Not yet published. From source:

```sh
git clone https://github.com/ryo9ra/bui
cd bui
cargo install --path .
```

Requires `git` on `PATH` (bui shells out to it).

## Usage

Run inside any git repository:

```sh
bui
```

Press `?` for the full key map.

## Development

See [`AGENT.md`](AGENT.md) for build/test commands, conventions, and the
architecture overview.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
