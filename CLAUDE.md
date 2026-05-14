# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

For project conventions, build commands, and architecture, see [AGENT.md](AGENT.md).
For the functional spec, key bindings, and roadmap, see [docs/spec.md](docs/spec.md).

## Repository status

**v0.1 feature-complete with tests.** All 14 features in `docs/spec.md`
§ 4.1 are wired (see `git log` for per-feature history); `cargo test`
runs 38 tests (30 unit + 8 integration against real `git`). When asked
to implement a feature, check `docs/spec.md` — if it belongs to v0.2 /
v0.3 / future, confirm with the user first.
