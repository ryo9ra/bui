# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

For project conventions, build commands, and architecture, see [AGENT.md](AGENT.md).
For the functional spec, key bindings, and roadmap, see [docs/spec.md](docs/spec.md).

## Repository status

**v0.1 complete, v0.2 in progress.** All 14 v0.1 features (`docs/spec.md`
§ 4.1) are wired. v0.2 progress: A2 Remote tab, A7 Detail pane, B7 Set
upstream picker, C1 fetch / C2 pull / C3 push (async worker). `cargo
test` runs 63 tests (47 unit + 16 integration). See `git log` for
per-feature history.
When asked to implement a feature, check `docs/spec.md` — if it
belongs to v0.3 / future, confirm with the user first.
