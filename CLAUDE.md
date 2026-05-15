# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

For project conventions, build commands, and architecture, see [AGENT.md](AGENT.md).
For the functional spec, key bindings, and roadmap, see [docs/spec.md](docs/spec.md).

## Repository status

**v0.1 + v0.2 feature-complete.** All 14 v0.1 features (`docs/spec.md`
§ 4.1) and the 12 v0.2 features (§ 6) are wired. `cargo test` runs 88
tests (70 unit + 18 integration). See `git log` for per-feature
history.
When asked to implement a feature, check `docs/spec.md` — if it
belongs to v0.3 / future, confirm with the user first.
