# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

For project conventions, build commands, and architecture, see [AGENT.md](AGENT.md).
For the functional spec, key bindings, and roadmap, see [docs/spec.md](docs/spec.md).

## Repository status

**v0.1 + v0.2 + v0.3 feature-complete, UX polished + A8 patch view
(2026-05-18).** All roadmap features in `docs/spec.md` (§ 4 / § 6)
are wired — worktree management, branch-to-branch diff,
force-with-lease push, remote-tracking checkout, remote-branch
delete — plus the polish round (Esc/Ctrl-C quit, ahead/behind
badge, hint footer, middle-truncated long names, proportional
detail pane, row flash on create/rename/add-worktree) and the A8
extension (full patch in the Diff pane with `Ctrl-D`/`Ctrl-U`
scroll, cache keyed on the (target, base) tuple so checkout / pull
stay in sync). `cargo test` runs 172 tests (140 unit + 32
integration). Clean gone (`X`, C8) added 2026-10-07. See `git log` for per-feature history.
When asked to implement a feature, check `docs/spec.md` — if it
belongs to v0.3 / future, confirm with the user first.
