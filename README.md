# bui

A terminal UI for git branch operations, written in Rust with
[`ratatui`](https://ratatui.rs).

`bui` stands for "**b**ranch **UI**". It focuses narrowly on the branch
domain: list, switch, create, rename, delete. It is **not** a full git TUI —
staging, committing, merging, and rebasing are deliberately out of scope.

## Status

Pre-v0.1, under active development. Not yet published. See
[`docs/spec.md`](docs/spec.md) for the formal scope and roadmap.

## v0.1 features (planned)

- Local branch list with the latest commit on each row
- Move with `j`/`k`/arrows; jump with `g`/`G`
- Switch (`Enter`), create (`c`), rename (`r`), delete (`d` / `D`)
- Incremental search with `/`
- Confirm dialog for destructive operations
- Help overlay (`?`); status bar for results, errors, and a spinner during
  async ops

Coming after v0.1:

- **v0.2** — remote branches, `fetch`/`pull`/`push`, right detail pane,
  filters/sorts, config file
- **v0.3** — worktrees, branch-to-branch diff in the right pane
- See [`docs/spec.md`](docs/spec.md) for the full roadmap

## Install

Not yet published. From source:

```sh
git clone https://github.com/RyotaSugawara/bui
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
